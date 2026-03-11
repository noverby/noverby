#![allow(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::only_used_in_recursion)]
//! mojo-gui XR shim — C FFI exposing Blitz + OpenXR for multi-panel XR rendering.
//!
//! This crate provides a flat C ABI (defined in `mojo_xr.h`) that the Mojo FFI
//! bindings (`xr/native/src/xr/xr_blitz.mojo`) call via `DLHandle`. It extends
//! the desktop Blitz renderer pattern with:
//!
//!   - **Multi-document support** — one `blitz-dom` document per XR panel
//!   - **Offscreen Vello rendering** — each panel renders to a GPU texture
//!   - **OpenXR integration** — session lifecycle, frame loop, quad layer compositing
//!   - **Controller raycasting** — pointer ray → panel quad intersection → DOM events
//!
//! # Architecture
//!
//! ```text
//! Mojo (xr_blitz.mojo)
//!   │ DLHandle FFI calls
//!   ▼
//! libmojo_xr.so (this crate)
//!   ├── XrSessionContext — owns panels, events, session state
//!   │   ├── Panel 0: blitz-dom document + ID map + event handlers + texture
//!   │   ├── Panel 1: blitz-dom document + ...
//!   │   └── Panel N: ...
//!   ├── OpenXR session (when not headless)
//!   ├── wgpu device + Vello renderer (when not headless)
//!   └── Event ring buffer (polled by Mojo)
//! ```
//!
//! # Build modes
//!
//! - **Normal**: Links against the OpenXR loader for real XR rendering.
//! - **Headless** (`mxr_create_headless`): No OpenXR, no GPU — DOM operations
//!   only. Used for integration tests and CI.
//!
//! # Thread safety
//!
//! All functions must be called from the thread that created the session.
//! This matches OpenXR's single-thread requirement for session calls.

use std::collections::HashMap;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU32, Ordering};

// ---------------------------------------------------------------------------
// Constants — mirror the #defines in mojo_xr.h
// ---------------------------------------------------------------------------

pub const MXR_EVT_CLICK: u8 = 1;
pub const MXR_EVT_INPUT: u8 = 2;
pub const MXR_EVT_CHANGE: u8 = 3;
pub const MXR_EVT_KEYDOWN: u8 = 4;
pub const MXR_EVT_KEYUP: u8 = 5;
pub const MXR_EVT_FOCUS: u8 = 6;
pub const MXR_EVT_BLUR: u8 = 7;
pub const MXR_EVT_SUBMIT: u8 = 8;
pub const MXR_EVT_MOUSEDOWN: u8 = 9;
pub const MXR_EVT_MOUSEUP: u8 = 10;
pub const MXR_EVT_MOUSEMOVE: u8 = 11;

pub const MXR_EVT_XR_SELECT: u8 = 0x80;
pub const MXR_EVT_XR_SQUEEZE: u8 = 0x81;
pub const MXR_EVT_XR_HOVER_ENTER: u8 = 0x82;
pub const MXR_EVT_XR_HOVER_EXIT: u8 = 0x83;

pub const MXR_HAND_LEFT: u8 = 0;
pub const MXR_HAND_RIGHT: u8 = 1;
pub const MXR_HAND_HEAD: u8 = 2;

pub const MXR_SPACE_LOCAL: u8 = 0;
pub const MXR_SPACE_STAGE: u8 = 1;
pub const MXR_SPACE_VIEW: u8 = 2;
pub const MXR_SPACE_UNBOUNDED: u8 = 3;

pub const MXR_STATE_IDLE: i32 = 0;
pub const MXR_STATE_READY: i32 = 1;
pub const MXR_STATE_FOCUSED: i32 = 2;
pub const MXR_STATE_VISIBLE: i32 = 3;
pub const MXR_STATE_STOPPING: i32 = 4;
pub const MXR_STATE_EXITING: i32 = 5;

const VERSION: &str = "0.1.0";

/// Maximum event ring buffer capacity.
const EVENT_RING_CAPACITY: usize = 256;

/// Global panel ID counter (monotonically increasing across all sessions).
static NEXT_PANEL_ID: AtomicU32 = AtomicU32::new(1);

// ---------------------------------------------------------------------------
// C-compatible structs (repr(C)) — must match mojo_xr.h exactly
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MxrEvent {
    pub valid: i32,
    pub panel_id: u32,
    pub handler_id: u32,
    pub event_type: u8,
    pub value_ptr: *const c_char,
    pub value_len: u32,
    pub hit_u: f32,
    pub hit_v: f32,
    pub hand: u8,
}

impl Default for MxrEvent {
    fn default() -> Self {
        Self {
            valid: 0,
            panel_id: 0,
            handler_id: 0,
            event_type: 0,
            value_ptr: std::ptr::null(),
            value_len: 0,
            hit_u: -1.0,
            hit_v: -1.0,
            hand: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MxrPose {
    pub px: f32,
    pub py: f32,
    pub pz: f32,
    pub qx: f32,
    pub qy: f32,
    pub qz: f32,
    pub qw: f32,
    pub valid: i32,
}

impl Default for MxrPose {
    fn default() -> Self {
        Self {
            px: 0.0,
            py: 0.0,
            pz: 0.0,
            qx: 0.0,
            qy: 0.0,
            qz: 0.0,
            qw: 1.0,
            valid: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MxrRaycastHit {
    pub hit: i32,
    pub panel_id: u32,
    pub u: f32,
    pub v: f32,
    pub distance: f32,
}

impl Default for MxrRaycastHit {
    fn default() -> Self {
        Self {
            hit: 0,
            panel_id: 0,
            u: 0.0,
            v: 0.0,
            distance: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Panel transform — position + orientation in 3D space
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct PanelTransform {
    /// Position (meters) in the reference space.
    position: [f32; 3],
    /// Orientation as a unit quaternion [x, y, z, w].
    rotation: [f32; 4],
    /// Physical size in meters [width, height].
    size_m: [f32; 2],
    /// Whether the panel is curved.
    curved: bool,
    /// Curvature radius in meters.
    curvature_radius: f32,
}

impl Default for PanelTransform {
    fn default() -> Self {
        Self {
            position: [0.0, 1.4, -1.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            size_m: [0.8, 0.6],
            curved: false,
            curvature_radius: 1.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Event handler registration (per node, per event type)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct EventListener {
    node_id: u32,
    handler_id: u32,
    event_type: u8,
}

// ---------------------------------------------------------------------------
// Panel — one Blitz DOM document + ID mapping + event handlers
// ---------------------------------------------------------------------------

/// Mojo element ID ↔ internal node ID mapping (same pattern as desktop shim).
#[derive(Debug, Default)]
struct IdMap {
    /// mojo_id → internal node_id
    to_internal: HashMap<u32, u32>,
    /// internal node_id → mojo_id
    to_mojo: HashMap<u32, u32>,
}

impl IdMap {
    fn assign(&mut self, mojo_id: u32, internal_id: u32) {
        self.to_internal.insert(mojo_id, internal_id);
        self.to_mojo.insert(internal_id, mojo_id);
    }

    fn remove_mojo(&mut self, mojo_id: u32) {
        if let Some(internal_id) = self.to_internal.remove(&mojo_id) {
            self.to_mojo.remove(&internal_id);
        }
    }

    #[allow(dead_code)]
    fn get_internal(&self, mojo_id: u32) -> Option<u32> {
        self.to_internal.get(&mojo_id).copied()
    }

    #[allow(dead_code)]
    fn get_mojo(&self, internal_id: u32) -> Option<u32> {
        self.to_mojo.get(&internal_id).copied()
    }
}

/// Interpreter stack — mirrors the desktop shim's stack for mutation processing.
#[derive(Debug, Default)]
struct InterpreterStack {
    stack: Vec<u32>,
}

impl InterpreterStack {
    fn push(&mut self, id: u32) {
        self.stack.push(id);
    }

    fn pop(&mut self) -> Option<u32> {
        self.stack.pop()
    }

    fn pop_n(&mut self, n: usize) -> Vec<u32> {
        let start = self.stack.len().saturating_sub(n);
        self.stack.drain(start..).collect()
    }

    #[allow(dead_code)]
    fn top(&self) -> Option<u32> {
        self.stack.last().copied()
    }

    fn clear(&mut self) {
        self.stack.clear();
    }
}

/// Simulated DOM node for headless mode.
///
/// In headless mode we don't have a real Blitz document, so we maintain a
/// lightweight DOM tree that supports the same operations (create element,
/// set attribute, append child, etc.) for testing purposes.
#[derive(Clone, Debug)]
struct HeadlessNode {
    /// Internal node ID (unique within a panel).
    id: u32,
    /// Tag name (empty for text/placeholder nodes).
    tag: String,
    /// Node kind.
    kind: HeadlessNodeKind,
    /// Attributes (name → value).
    attributes: HashMap<String, String>,
    /// Children (internal IDs, ordered).
    children: Vec<u32>,
    /// Parent (internal ID), or 0 for root.
    parent: u32,
}

#[derive(Clone, Debug, PartialEq)]
enum HeadlessNodeKind {
    Element,
    Text(String),
    Placeholder,
}

impl HeadlessNode {
    fn element(id: u32, tag: &str) -> Self {
        Self {
            id,
            tag: tag.to_string(),
            kind: HeadlessNodeKind::Element,
            attributes: HashMap::new(),
            children: Vec::new(),
            parent: 0,
        }
    }

    fn text(id: u32, content: &str) -> Self {
        Self {
            id,
            tag: String::new(),
            kind: HeadlessNodeKind::Text(content.to_string()),
            attributes: HashMap::new(),
            children: Vec::new(),
            parent: 0,
        }
    }

    fn placeholder(id: u32) -> Self {
        Self {
            id,
            tag: String::new(),
            kind: HeadlessNodeKind::Placeholder,
            attributes: HashMap::new(),
            children: Vec::new(),
            parent: 0,
        }
    }
}

/// A registered template (headless mode) — a tree of nodes that can be cloned.
#[derive(Clone, Debug)]
struct HeadlessTemplate {
    /// The serialized template buffer (kept for re-parsing if needed).
    #[allow(dead_code)]
    data: Vec<u8>,
    /// Root nodes of the template (as HeadlessNode trees).
    roots: Vec<HeadlessNode>,
}

/// Panel state — one per XR panel.
struct Panel {
    /// Unique panel ID (assigned globally).
    panel_id: u32,
    /// Texture dimensions in pixels.
    texture_width: u32,
    texture_height: u32,
    /// 3D transform.
    transform: PanelTransform,
    /// Whether the panel is visible in the scene.
    visible: bool,
    /// Whether the panel accepts pointer input.
    interactive: bool,
    /// Whether the panel's texture needs re-rendering.
    dirty: bool,
    /// Whether mutations are currently being batched.
    in_mutation_batch: bool,
    /// ID mapping (mojo element IDs ↔ internal node IDs).
    id_map: IdMap,
    /// Interpreter stack for mutation processing.
    stack: InterpreterStack,
    /// Event listeners registered on DOM nodes.
    listeners: Vec<EventListener>,
    /// Registered templates (template_id → template).
    templates: HashMap<u32, HeadlessTemplate>,
    /// Next template ID.
    next_template_id: u32,
    /// Headless DOM tree (id → node). Only populated in headless mode.
    nodes: HashMap<u32, HeadlessNode>,
    /// Next internal node ID.
    next_node_id: u32,
    /// Mount point node ID.
    mount_point_id: u32,
    /// User-agent stylesheets (stored for potential future use).
    ua_stylesheets: Vec<String>,
}

impl Panel {
    fn new(panel_id: u32, width_px: u32, height_px: u32) -> Self {
        // Create a minimal document structure: root → html → body → mount_point
        let mut nodes = HashMap::new();

        let root_id = 1_u32;
        let html_id = 2_u32;
        let body_id = 3_u32;
        let mount_id = 4_u32;

        let mut root = HeadlessNode::element(root_id, "document");
        root.children.push(html_id);

        let mut html = HeadlessNode::element(html_id, "html");
        html.parent = root_id;
        html.children.push(body_id);

        let mut body = HeadlessNode::element(body_id, "body");
        body.parent = html_id;
        body.children.push(mount_id);

        let mut mount = HeadlessNode::element(mount_id, "div");
        mount.parent = body_id;

        nodes.insert(root_id, root);
        nodes.insert(html_id, html);
        nodes.insert(body_id, body);
        nodes.insert(mount_id, mount);

        Self {
            panel_id,
            texture_width: width_px,
            texture_height: height_px,
            transform: PanelTransform::default(),
            visible: true,
            interactive: true,
            dirty: false,
            in_mutation_batch: false,
            id_map: IdMap::default(),
            stack: InterpreterStack::default(),
            listeners: Vec::new(),
            templates: HashMap::new(),
            next_template_id: 0,
            nodes,
            next_node_id: 5, // start after the document scaffolding
            mount_point_id: mount_id,
            ua_stylesheets: Vec::new(),
        }
    }

    /// Allocate a new internal node ID.
    fn alloc_node_id(&mut self) -> u32 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    /// Create an element node and add it to the DOM.
    fn create_element(&mut self, tag: &str) -> u32 {
        let id = self.alloc_node_id();
        self.nodes.insert(id, HeadlessNode::element(id, tag));
        id
    }

    /// Create a text node and add it to the DOM.
    fn create_text_node(&mut self, text: &str) -> u32 {
        let id = self.alloc_node_id();
        self.nodes.insert(id, HeadlessNode::text(id, text));
        id
    }

    /// Create a placeholder (comment) node.
    fn create_placeholder(&mut self) -> u32 {
        let id = self.alloc_node_id();
        self.nodes.insert(id, HeadlessNode::placeholder(id));
        id
    }

    /// Set an attribute on a node.
    fn set_attribute(&mut self, node_id: u32, name: &str, value: &str) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.attributes.insert(name.to_string(), value.to_string());
        }
    }

    /// Remove an attribute from a node.
    fn remove_attribute(&mut self, node_id: u32, name: &str) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.attributes.remove(name);
        }
    }

    /// Set the text content of a text node.
    fn set_text_content(&mut self, node_id: u32, text: &str) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.kind = HeadlessNodeKind::Text(text.to_string());
        }
    }

    /// Append children to a parent node.
    fn append_children(&mut self, parent_id: u32, child_ids: &[u32]) {
        // First detach children from any previous parent
        for &child_id in child_ids {
            if let Some(child) = self.nodes.get(&child_id) {
                let old_parent = child.parent;
                if old_parent != 0 {
                    if let Some(old_p) = self.nodes.get_mut(&old_parent) {
                        old_p.children.retain(|&c| c != child_id);
                    }
                }
            }
        }

        // Set new parent on children
        for &child_id in child_ids {
            if let Some(child) = self.nodes.get_mut(&child_id) {
                child.parent = parent_id;
            }
        }

        // Add to parent's children list
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.extend_from_slice(child_ids);
        }
    }

    /// Insert nodes before a reference node.
    fn insert_before(&mut self, reference_id: u32, node_ids: &[u32]) {
        let parent_id = self.nodes.get(&reference_id).map(|n| n.parent).unwrap_or(0);
        if parent_id == 0 {
            return;
        }

        // Detach nodes from old parents
        for &nid in node_ids {
            if let Some(node) = self.nodes.get(&nid) {
                let old_parent = node.parent;
                if old_parent != 0 {
                    if let Some(old_p) = self.nodes.get_mut(&old_parent) {
                        old_p.children.retain(|&c| c != nid);
                    }
                }
            }
            if let Some(node) = self.nodes.get_mut(&nid) {
                node.parent = parent_id;
            }
        }

        // Find the reference index in the parent's children and insert before it
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if let Some(pos) = parent.children.iter().position(|&c| c == reference_id) {
                for (i, &nid) in node_ids.iter().enumerate() {
                    parent.children.insert(pos + i, nid);
                }
            }
        }
    }

    /// Insert nodes after a reference node.
    fn insert_after(&mut self, reference_id: u32, node_ids: &[u32]) {
        let parent_id = self.nodes.get(&reference_id).map(|n| n.parent).unwrap_or(0);
        if parent_id == 0 {
            return;
        }

        // Detach nodes from old parents
        for &nid in node_ids {
            if let Some(node) = self.nodes.get(&nid) {
                let old_parent = node.parent;
                if old_parent != 0 {
                    if let Some(old_p) = self.nodes.get_mut(&old_parent) {
                        old_p.children.retain(|&c| c != nid);
                    }
                }
            }
            if let Some(node) = self.nodes.get_mut(&nid) {
                node.parent = parent_id;
            }
        }

        // Find the reference index and insert after it
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if let Some(pos) = parent.children.iter().position(|&c| c == reference_id) {
                for (i, &nid) in node_ids.iter().enumerate() {
                    parent.children.insert(pos + 1 + i, nid);
                }
            }
        }
    }

    /// Replace a node with one or more new nodes.
    fn replace_with(&mut self, old_id: u32, new_ids: &[u32]) {
        let parent_id = self.nodes.get(&old_id).map(|n| n.parent).unwrap_or(0);
        if parent_id == 0 {
            return;
        }

        // Detach new nodes from old parents
        for &nid in new_ids {
            if let Some(node) = self.nodes.get(&nid) {
                let old_parent = node.parent;
                if old_parent != 0 {
                    if let Some(old_p) = self.nodes.get_mut(&old_parent) {
                        old_p.children.retain(|&c| c != nid);
                    }
                }
            }
            if let Some(node) = self.nodes.get_mut(&nid) {
                node.parent = parent_id;
            }
        }

        // Replace old_id in parent's children list
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if let Some(pos) = parent.children.iter().position(|&c| c == old_id) {
                parent.children.remove(pos);
                for (i, &nid) in new_ids.iter().enumerate() {
                    parent.children.insert(pos + i, nid);
                }
            }
        }

        // Remove old node from DOM
        if let Some(old_node) = self.nodes.get_mut(&old_id) {
            old_node.parent = 0;
        }
    }

    /// Remove a node from the DOM tree.
    fn remove_node(&mut self, node_id: u32) {
        if let Some(node) = self.nodes.get(&node_id) {
            let parent_id = node.parent;
            if parent_id != 0 {
                if let Some(parent) = self.nodes.get_mut(&parent_id) {
                    parent.children.retain(|&c| c != node_id);
                }
            }
        }
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.parent = 0;
        }
        // Note: we don't recursively delete the subtree — the node remains
        // in the map for potential re-insertion. The mojo-side ID mapping
        // cleanup handles logical deletion.
    }

    /// Get the child node at a given index.
    fn child_at(&self, parent_id: u32, index: u32) -> Option<u32> {
        self.nodes
            .get(&parent_id)
            .and_then(|n| n.children.get(index as usize).copied())
    }

    /// Get the number of children of a node.
    fn child_count(&self, parent_id: u32) -> u32 {
        self.nodes
            .get(&parent_id)
            .map(|n| n.children.len() as u32)
            .unwrap_or(0)
    }

    /// Navigate to a node by following a path of child indices from a root.
    fn node_at_path(&self, root_id: u32, path: &[u32]) -> Option<u32> {
        let mut current = root_id;
        for &index in path {
            current = self.child_at(current, index)?;
        }
        Some(current)
    }

    /// Serialize a subtree to an HTML-like string (for testing/debugging).
    fn serialize_subtree(&self, root_id: u32) -> String {
        let mut out = String::new();
        self.serialize_node(root_id, &mut out, 0);
        out
    }

    fn serialize_node(&self, node_id: u32, out: &mut String, depth: usize) {
        let Some(node) = self.nodes.get(&node_id) else {
            return;
        };

        match &node.kind {
            HeadlessNodeKind::Element => {
                out.push('<');
                out.push_str(&node.tag);

                // Sort attributes for deterministic output
                let mut attrs: Vec<_> = node.attributes.iter().collect();
                attrs.sort_by_key(|(k, _)| (*k).clone());
                for (name, value) in &attrs {
                    out.push(' ');
                    out.push_str(name);
                    out.push_str("=\"");
                    out.push_str(&value.replace('"', "&quot;"));
                    out.push('"');
                }

                if node.children.is_empty() {
                    out.push_str(" />");
                } else {
                    out.push('>');
                    for &child_id in &node.children {
                        self.serialize_node(child_id, out, depth + 1);
                    }
                    out.push_str("</");
                    out.push_str(&node.tag);
                    out.push('>');
                }
            }
            HeadlessNodeKind::Text(text) => {
                out.push_str(text);
            }
            HeadlessNodeKind::Placeholder => {
                out.push_str("<!--placeholder-->");
            }
        }
    }

    /// Add an event listener.
    fn add_event_listener(&mut self, node_id: u32, handler_id: u32, event_type: u8) {
        self.listeners.push(EventListener {
            node_id,
            handler_id,
            event_type,
        });
    }

    /// Remove an event listener.
    fn remove_event_listener(&mut self, node_id: u32, handler_id: u32, event_type: u8) {
        self.listeners.retain(|l| {
            !(l.node_id == node_id && l.handler_id == handler_id && l.event_type == event_type)
        });
    }

    /// Find the handler for an event on a node (walk up the tree for bubbling).
    fn find_handler(&self, node_id: u32, event_type: u8) -> Option<u32> {
        let mut current = Some(node_id);
        while let Some(nid) = current {
            for listener in &self.listeners {
                if listener.node_id == nid && listener.event_type == event_type {
                    return Some(listener.handler_id);
                }
            }
            current = self
                .nodes
                .get(&nid)
                .and_then(|n| if n.parent != 0 { Some(n.parent) } else { None });
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Event ring buffer
// ---------------------------------------------------------------------------

/// Internal event (owns the value string).
#[derive(Clone, Debug)]
struct InternalEvent {
    panel_id: u32,
    handler_id: u32,
    event_type: u8,
    value: String,
    hit_u: f32,
    hit_v: f32,
    hand: u8,
}

struct EventRing {
    events: Vec<InternalEvent>,
    /// Temporary storage for the value string of the last polled event,
    /// kept alive so the C pointer remains valid until the next poll.
    last_polled_value: String,
}

impl EventRing {
    fn new() -> Self {
        Self {
            events: Vec::with_capacity(EVENT_RING_CAPACITY),
            last_polled_value: String::new(),
        }
    }

    fn push(&mut self, event: InternalEvent) {
        if self.events.len() < EVENT_RING_CAPACITY {
            self.events.push(event);
        }
        // Silently drop if full — same as desktop shim.
    }

    fn poll(&mut self) -> MxrEvent {
        if self.events.is_empty() {
            return MxrEvent::default();
        }

        let event = self.events.remove(0);
        self.last_polled_value = event.value;

        MxrEvent {
            valid: 1,
            panel_id: event.panel_id,
            handler_id: event.handler_id,
            event_type: event.event_type,
            value_ptr: self.last_polled_value.as_ptr() as *const c_char,
            value_len: self.last_polled_value.len() as u32,
            hit_u: event.hit_u,
            hit_v: event.hit_v,
            hand: event.hand,
        }
    }

    fn count(&self) -> u32 {
        self.events.len() as u32
    }

    fn clear(&mut self) {
        self.events.clear();
    }
}

// ---------------------------------------------------------------------------
// XrSessionContext — the top-level session state
// ---------------------------------------------------------------------------

pub struct XrSessionContext {
    /// Whether this is a headless (no OpenXR/GPU) session.
    headless: bool,
    /// Session state (mirrors XrSessionState).
    state: i32,
    /// Whether the session has been destroyed.
    destroyed: bool,
    /// All panels, keyed by panel_id.
    panels: HashMap<u32, Panel>,
    /// Panel insertion order (for deterministic iteration).
    panel_order: Vec<u32>,
    /// Focused panel ID (0 = no focus).
    focused_panel_id: u32,
    /// Event ring buffer.
    events: EventRing,
    /// Active reference space type.
    reference_space: u8,
    /// Application name.
    #[allow(dead_code)]
    app_name: String,
}

impl XrSessionContext {
    fn new_headless() -> Self {
        Self {
            headless: true,
            state: MXR_STATE_FOCUSED, // Headless sessions start focused.
            destroyed: false,
            panels: HashMap::new(),
            panel_order: Vec::new(),
            focused_panel_id: 0,
            events: EventRing::new(),
            reference_space: MXR_SPACE_STAGE,
            app_name: String::from("headless"),
        }
    }

    fn new_with_name(name: &str) -> Self {
        Self {
            headless: false,
            state: MXR_STATE_IDLE,
            destroyed: false,
            panels: HashMap::new(),
            panel_order: Vec::new(),
            focused_panel_id: 0,
            events: EventRing::new(),
            reference_space: MXR_SPACE_STAGE,
            app_name: name.to_string(),
        }
    }

    fn is_alive(&self) -> bool {
        !self.destroyed && self.state < MXR_STATE_EXITING
    }

    fn create_panel(&mut self, width_px: u32, height_px: u32) -> u32 {
        let panel_id = NEXT_PANEL_ID.fetch_add(1, Ordering::Relaxed);
        let panel = Panel::new(panel_id, width_px, height_px);
        self.panels.insert(panel_id, panel);
        self.panel_order.push(panel_id);

        // Auto-focus the first panel
        if self.focused_panel_id == 0 {
            self.focused_panel_id = panel_id;
        }

        panel_id
    }

    fn destroy_panel(&mut self, panel_id: u32) {
        self.panels.remove(&panel_id);
        self.panel_order.retain(|&id| id != panel_id);
        if self.focused_panel_id == panel_id {
            self.focused_panel_id = self
                .panel_order
                .iter()
                .copied()
                .find(|&id| {
                    self.panels
                        .get(&id)
                        .map(|p| p.visible && p.interactive)
                        .unwrap_or(false)
                })
                .unwrap_or(0);
        }
    }

    fn get_panel(&self, panel_id: u32) -> Option<&Panel> {
        self.panels.get(&panel_id)
    }

    fn get_panel_mut(&mut self, panel_id: u32) -> Option<&mut Panel> {
        self.panels.get_mut(&panel_id)
    }

    /// Raycast against all visible, interactive panels. Returns the closest hit.
    fn raycast(&self, origin: [f32; 3], direction: [f32; 3]) -> MxrRaycastHit {
        let mut best = MxrRaycastHit::default();
        let mut best_distance = f32::MAX;

        for &panel_id in &self.panel_order {
            let Some(panel) = self.panels.get(&panel_id) else {
                continue;
            };
            if !panel.visible || !panel.interactive {
                continue;
            }

            let t = &panel.transform;
            let q = t.rotation; // [x, y, z, w]

            // Panel normal: rotate (0, 0, -1) by quaternion
            let nx = -2.0 * (q[0] * q[2] + q[3] * q[1]);
            let ny = -2.0 * (q[1] * q[2] - q[3] * q[0]);
            let nz = -(1.0 - 2.0 * (q[0] * q[0] + q[1] * q[1]));

            // Ray-plane intersection
            let denom = direction[0] * nx + direction[1] * ny + direction[2] * nz;
            if denom.abs() < 1e-6 {
                continue;
            }

            let diff = [
                t.position[0] - origin[0],
                t.position[1] - origin[1],
                t.position[2] - origin[2],
            ];
            let t_hit = (diff[0] * nx + diff[1] * ny + diff[2] * nz) / denom;

            if t_hit < 0.0 || t_hit >= best_distance {
                continue;
            }

            // Hit point in world space
            let hit = [
                origin[0] + direction[0] * t_hit,
                origin[1] + direction[1] * t_hit,
                origin[2] + direction[2] * t_hit,
            ];

            // Panel-local axes (right = rotated +X, up = rotated +Y)
            let right = [
                1.0 - 2.0 * (q[1] * q[1] + q[2] * q[2]),
                2.0 * (q[0] * q[1] + q[3] * q[2]),
                2.0 * (q[0] * q[2] - q[3] * q[1]),
            ];
            let up = [
                2.0 * (q[0] * q[1] - q[3] * q[2]),
                1.0 - 2.0 * (q[0] * q[0] + q[2] * q[2]),
                2.0 * (q[1] * q[2] + q[3] * q[0]),
            ];

            // Offset from panel center
            let local_offset = [
                hit[0] - t.position[0],
                hit[1] - t.position[1],
                hit[2] - t.position[2],
            ];
            let local_x = local_offset[0] * right[0]
                + local_offset[1] * right[1]
                + local_offset[2] * right[2];
            let local_y =
                local_offset[0] * up[0] + local_offset[1] * up[1] + local_offset[2] * up[2];

            let half_w = t.size_m[0] * 0.5;
            let half_h = t.size_m[1] * 0.5;

            if local_x < -half_w || local_x > half_w || local_y < -half_h || local_y > half_h {
                continue;
            }

            let u = (local_x + half_w) / t.size_m[0];
            let v = 1.0 - (local_y + half_h) / t.size_m[1];

            best = MxrRaycastHit {
                hit: 1,
                panel_id,
                u,
                v,
                distance: t_hit,
            };
            best_distance = t_hit;
        }

        best
    }
}

// ---------------------------------------------------------------------------
// Helper: read a UTF-8 string from a C pointer + length
// ---------------------------------------------------------------------------

unsafe fn read_str(ptr: *const c_char, len: u32) -> &'static str {
    if ptr.is_null() || len == 0 {
        return "";
    }
    let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
    std::str::from_utf8_unchecked(slice)
}

/// Write a string into a C buffer. Returns the number of bytes written
/// (or required size if buf is null / too small).
unsafe fn write_to_buf(s: &str, buf: *mut c_char, buf_len: u32) -> u32 {
    let needed = s.len() as u32;
    if buf.is_null() || buf_len == 0 {
        return needed;
    }
    let copy_len = std::cmp::min(needed, buf_len) as usize;
    std::ptr::copy_nonoverlapping(s.as_ptr(), buf as *mut u8, copy_len);
    copy_len as u32
}

// ---------------------------------------------------------------------------
// C FFI exports
// ---------------------------------------------------------------------------

// ── Session lifecycle ──────────────────────────────────────────────────────

/// Create an XR session with the specified application name.
///
/// In this initial scaffold (Step 5.1/5.2), this creates a session context
/// but does NOT initialize OpenXR or GPU resources. Real OpenXR integration
/// will be added in a subsequent step when the `openxr` crate integration
/// is wired up.
///
/// For now, this behaves like `mxr_create_headless` but records the app name
/// and sets the initial state to IDLE (rather than FOCUSED).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_create_session(
    app_name: *const c_char,
    app_name_len: u32,
) -> *mut XrSessionContext {
    let name = read_str(app_name, app_name_len);
    let ctx = Box::new(XrSessionContext::new_with_name(name));
    Box::into_raw(ctx)
}

/// Create a headless XR session for testing (no OpenXR, no GPU).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_create_headless() -> *mut XrSessionContext {
    let ctx = Box::new(XrSessionContext::new_headless());
    Box::into_raw(ctx)
}

/// Query the current session state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_session_state(session: *mut XrSessionContext) -> i32 {
    if session.is_null() {
        return MXR_STATE_EXITING;
    }
    (*session).state
}

/// Check if the session is alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_is_alive(session: *mut XrSessionContext) -> i32 {
    if session.is_null() {
        return 0;
    }
    (*session).is_alive() as i32
}

/// Destroy the session and free all resources.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_destroy_session(session: *mut XrSessionContext) {
    if session.is_null() {
        return;
    }
    let mut ctx = Box::from_raw(session);
    ctx.destroyed = true;
    ctx.state = MXR_STATE_EXITING;
    // Box drop runs here, freeing all panels and their DOM trees.
}

// ── Panel lifecycle ────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_create_panel(
    session: *mut XrSessionContext,
    width_px: u32,
    height_px: u32,
) -> u32 {
    if session.is_null() || width_px == 0 || height_px == 0 {
        return 0;
    }
    (*session).create_panel(width_px, height_px)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_destroy_panel(session: *mut XrSessionContext, panel_id: u32) {
    if session.is_null() {
        return;
    }
    (*session).destroy_panel(panel_id);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_set_transform(
    session: *mut XrSessionContext,
    panel_id: u32,
    px: f32,
    py: f32,
    pz: f32,
    qx: f32,
    qy: f32,
    qz: f32,
    qw: f32,
) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.transform.position = [px, py, pz];
        panel.transform.rotation = [qx, qy, qz, qw];
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_set_size(
    session: *mut XrSessionContext,
    panel_id: u32,
    width_m: f32,
    height_m: f32,
) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.transform.size_m = [width_m, height_m];
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_set_visible(
    session: *mut XrSessionContext,
    panel_id: u32,
    visible: i32,
) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.visible = visible != 0;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_is_visible(
    session: *mut XrSessionContext,
    panel_id: u32,
) -> i32 {
    if session.is_null() {
        return 0;
    }
    (*session)
        .get_panel(panel_id)
        .map(|p| p.visible as i32)
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_set_curved(
    session: *mut XrSessionContext,
    panel_id: u32,
    curved: i32,
    radius: f32,
) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.transform.curved = curved != 0;
        panel.transform.curvature_radius = radius;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_count(session: *mut XrSessionContext) -> u32 {
    if session.is_null() {
        return 0;
    }
    (*session).panels.len() as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_add_ua_stylesheet(
    session: *mut XrSessionContext,
    panel_id: u32,
    css_ptr: *const c_char,
    css_len: u32,
) {
    if session.is_null() {
        return;
    }
    let css = read_str(css_ptr, css_len);
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.ua_stylesheets.push(css.to_string());
    }
}

// ── Mutations ──────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_begin_mutations(session: *mut XrSessionContext, panel_id: u32) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.in_mutation_batch = true;
    }
}

/// Apply a binary mutation buffer to a panel's DOM.
///
/// In headless mode, this is a no-op placeholder. The Mojo-side
/// MutationInterpreter will call individual DOM operation FFI functions
/// (create_element, set_attribute, etc.) instead of passing a raw buffer.
///
/// When the full interpreter is implemented on the Rust side (matching the
/// desktop shim pattern), this function will decode the binary opcodes and
/// apply them to the panel's Blitz document directly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_apply_mutations(
    session: *mut XrSessionContext,
    panel_id: u32,
    _buf: *const u8,
    _len: u32,
) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.dirty = true;
        // TODO: Implement binary opcode interpreter (same protocol as desktop shim).
        // For now, the Mojo-side MutationInterpreter calls individual FFI functions.
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_end_mutations(session: *mut XrSessionContext, panel_id: u32) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.in_mutation_batch = false;
        panel.dirty = true;
    }
}

// ── Per-panel DOM operations ───────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_create_element(
    session: *mut XrSessionContext,
    panel_id: u32,
    tag_ptr: *const c_char,
    tag_len: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    let tag = read_str(tag_ptr, tag_len);
    (*session)
        .get_panel_mut(panel_id)
        .map(|p| p.create_element(tag))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_create_text_node(
    session: *mut XrSessionContext,
    panel_id: u32,
    text_ptr: *const c_char,
    text_len: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    let text = read_str(text_ptr, text_len);
    (*session)
        .get_panel_mut(panel_id)
        .map(|p| p.create_text_node(text))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_create_placeholder(
    session: *mut XrSessionContext,
    panel_id: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    (*session)
        .get_panel_mut(panel_id)
        .map(|p| p.create_placeholder())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_register_template(
    session: *mut XrSessionContext,
    panel_id: u32,
    buf: *const u8,
    len: u32,
) {
    if session.is_null() || buf.is_null() {
        return;
    }
    let data = std::slice::from_raw_parts(buf, len as usize).to_vec();
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        let tid = panel.next_template_id;
        panel.next_template_id += 1;
        panel.templates.insert(
            tid,
            HeadlessTemplate {
                data,
                roots: Vec::new(), // Template root parsing is a TODO
            },
        );
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_clone_template(
    session: *mut XrSessionContext,
    panel_id: u32,
    _template_id: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    // In headless mode, cloning a template creates a placeholder element.
    // Full template cloning requires parsing the template buffer (TODO).
    (*session)
        .get_panel_mut(panel_id)
        .map(|p| p.create_element("template-clone"))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_append_children(
    session: *mut XrSessionContext,
    panel_id: u32,
    parent_id: u32,
    child_ids: *const u32,
    count: u32,
) {
    if session.is_null() || child_ids.is_null() {
        return;
    }
    let ids = std::slice::from_raw_parts(child_ids, count as usize);
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.append_children(parent_id, ids);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_insert_before(
    session: *mut XrSessionContext,
    panel_id: u32,
    reference_id: u32,
    node_ids: *const u32,
    count: u32,
) {
    if session.is_null() || node_ids.is_null() {
        return;
    }
    let ids = std::slice::from_raw_parts(node_ids, count as usize);
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.insert_before(reference_id, ids);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_insert_after(
    session: *mut XrSessionContext,
    panel_id: u32,
    reference_id: u32,
    node_ids: *const u32,
    count: u32,
) {
    if session.is_null() || node_ids.is_null() {
        return;
    }
    let ids = std::slice::from_raw_parts(node_ids, count as usize);
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.insert_after(reference_id, ids);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_replace_with(
    session: *mut XrSessionContext,
    panel_id: u32,
    old_id: u32,
    new_ids: *const u32,
    count: u32,
) {
    if session.is_null() || new_ids.is_null() {
        return;
    }
    let ids = std::slice::from_raw_parts(new_ids, count as usize);
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.replace_with(old_id, ids);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_remove_node(
    session: *mut XrSessionContext,
    panel_id: u32,
    node_id: u32,
) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.remove_node(node_id);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_set_attribute(
    session: *mut XrSessionContext,
    panel_id: u32,
    node_id: u32,
    name_ptr: *const c_char,
    name_len: u32,
    value_ptr: *const c_char,
    value_len: u32,
) {
    if session.is_null() {
        return;
    }
    let name = read_str(name_ptr, name_len);
    let value = read_str(value_ptr, value_len);
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.set_attribute(node_id, name, value);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_remove_attribute(
    session: *mut XrSessionContext,
    panel_id: u32,
    node_id: u32,
    name_ptr: *const c_char,
    name_len: u32,
) {
    if session.is_null() {
        return;
    }
    let name = read_str(name_ptr, name_len);
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.remove_attribute(node_id, name);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_set_text_content(
    session: *mut XrSessionContext,
    panel_id: u32,
    node_id: u32,
    text_ptr: *const c_char,
    text_len: u32,
) {
    if session.is_null() {
        return;
    }
    let text = read_str(text_ptr, text_len);
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.set_text_content(node_id, text);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_node_at_path(
    session: *mut XrSessionContext,
    panel_id: u32,
    root_id: u32,
    path: *const u32,
    path_len: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    let path_slice = if path.is_null() || path_len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(path, path_len as usize)
    };
    (*session)
        .get_panel(panel_id)
        .and_then(|p| p.node_at_path(root_id, path_slice))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_child_at(
    session: *mut XrSessionContext,
    panel_id: u32,
    parent_id: u32,
    index: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    (*session)
        .get_panel(panel_id)
        .and_then(|p| p.child_at(parent_id, index))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_child_count(
    session: *mut XrSessionContext,
    panel_id: u32,
    parent_id: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    (*session)
        .get_panel(panel_id)
        .map(|p| p.child_count(parent_id))
        .unwrap_or(0)
}

// ── Events ─────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_add_event_listener(
    session: *mut XrSessionContext,
    panel_id: u32,
    node_id: u32,
    handler_id: u32,
    event_type: u8,
) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.add_event_listener(node_id, handler_id, event_type);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_remove_event_listener(
    session: *mut XrSessionContext,
    panel_id: u32,
    node_id: u32,
    handler_id: u32,
    event_type: u8,
) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel_mut(panel_id) {
        panel.remove_event_listener(node_id, handler_id, event_type);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_poll_event(session: *mut XrSessionContext) -> MxrEvent {
    if session.is_null() {
        return MxrEvent::default();
    }
    (*session).events.poll()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_event_count(session: *mut XrSessionContext) -> u32 {
    if session.is_null() {
        return 0;
    }
    (*session).events.count()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_event_clear(session: *mut XrSessionContext) {
    if session.is_null() {
        return;
    }
    (*session).events.clear();
}

// ── Raycasting ─────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_raycast_panels(
    session: *mut XrSessionContext,
    ox: f32,
    oy: f32,
    oz: f32,
    dx: f32,
    dy: f32,
    dz: f32,
) -> MxrRaycastHit {
    if session.is_null() {
        return MxrRaycastHit::default();
    }
    (*session).raycast([ox, oy, oz], [dx, dy, dz])
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_set_focused_panel(session: *mut XrSessionContext, panel_id: u32) {
    if session.is_null() {
        return;
    }
    (*session).focused_panel_id = panel_id;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_get_focused_panel(session: *mut XrSessionContext) -> u32 {
    if session.is_null() {
        return 0;
    }
    (*session).focused_panel_id
}

// ── Frame loop ─────────────────────────────────────────────────────────────

/// Wait for the next frame. In headless mode, returns a synthetic timestamp.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_wait_frame(session: *mut XrSessionContext) -> i64 {
    if session.is_null() {
        return 0;
    }
    if (*session).headless {
        // Return a monotonic timestamp in nanoseconds for testing.
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    } else {
        // TODO: Call xrWaitFrame when OpenXR is wired up.
        0
    }
}

/// Begin a new frame. In headless mode, always succeeds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_begin_frame(session: *mut XrSessionContext) -> i32 {
    if session.is_null() {
        return 0;
    }
    if (*session).headless {
        1 // Always succeeds in headless mode.
    } else {
        // TODO: Call xrBeginFrame when OpenXR is wired up.
        0
    }
}

/// Render dirty panel textures. In headless mode, just clears dirty flags.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_render_dirty_panels(session: *mut XrSessionContext) -> u32 {
    if session.is_null() {
        return 0;
    }
    let mut count = 0u32;
    let panel_ids: Vec<u32> = (*session).panel_order.clone();
    for panel_id in panel_ids {
        if let Some(panel) = (*session).get_panel_mut(panel_id) {
            if panel.dirty && panel.visible {
                // In headless mode, we just clear the dirty flag.
                // With a real GPU, this would run Vello to render the
                // panel's Blitz DOM to its offscreen texture.
                panel.dirty = false;
                count += 1;
            }
        }
    }
    count
}

/// End the frame. In headless mode, no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_end_frame(session: *mut XrSessionContext) {
    let _ = session;
    // TODO: Call xrEndFrame with quad layers when OpenXR is wired up.
}

// ── Input ──────────────────────────────────────────────────────────────────

/// Get a controller/head pose. In headless mode, returns an invalid pose.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_get_pose(session: *mut XrSessionContext, _hand: u8) -> MxrPose {
    if session.is_null() {
        return MxrPose::default();
    }
    // In headless mode, poses are not tracked.
    MxrPose::default()
}

/// Get the aim ray for a controller. In headless mode, returns invalid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_get_aim_ray(
    session: *mut XrSessionContext,
    _hand: u8,
    out_ox: *mut f32,
    out_oy: *mut f32,
    out_oz: *mut f32,
    out_dx: *mut f32,
    out_dy: *mut f32,
    out_dz: *mut f32,
) -> i32 {
    if session.is_null() {
        return 0;
    }
    // In headless mode, aim rays are not available.
    if !out_ox.is_null() {
        *out_ox = 0.0;
    }
    if !out_oy.is_null() {
        *out_oy = 0.0;
    }
    if !out_oz.is_null() {
        *out_oz = 0.0;
    }
    if !out_dx.is_null() {
        *out_dx = 0.0;
    }
    if !out_dy.is_null() {
        *out_dy = 0.0;
    }
    if !out_dz.is_null() {
        *out_dz = 0.0;
    }
    0 // Not valid
}

// ── Reference spaces ───────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_set_reference_space(
    session: *mut XrSessionContext,
    space_type: u8,
) -> i32 {
    if session.is_null() {
        return 0;
    }
    if space_type > MXR_SPACE_UNBOUNDED {
        return 0;
    }
    (*session).reference_space = space_type;
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_get_reference_space(session: *mut XrSessionContext) -> u8 {
    if session.is_null() {
        return MXR_SPACE_STAGE;
    }
    (*session).reference_space
}

// ── Capabilities ───────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_has_extension(
    session: *mut XrSessionContext,
    _ext_name_ptr: *const c_char,
    _ext_name_len: u32,
) -> i32 {
    if session.is_null() {
        return 0;
    }
    // In headless mode and the current scaffold, no extensions are available.
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_has_hand_tracking(session: *mut XrSessionContext) -> i32 {
    if session.is_null() {
        return 0;
    }
    0 // Not available in headless mode.
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_has_passthrough(session: *mut XrSessionContext) -> i32 {
    if session.is_null() {
        return 0;
    }
    0 // Not available in headless mode.
}

// ── Debug & inspection ─────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_print_tree(session: *mut XrSessionContext, panel_id: u32) {
    if session.is_null() {
        return;
    }
    if let Some(panel) = (*session).get_panel(panel_id) {
        let html = panel.serialize_subtree(panel.mount_point_id);
        eprintln!("[panel {}] {}", panel_id, html);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_serialize_subtree(
    session: *mut XrSessionContext,
    panel_id: u32,
    buf: *mut c_char,
    buf_len: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    let html = (*session)
        .get_panel(panel_id)
        .map(|p| p.serialize_subtree(p.mount_point_id))
        .unwrap_or_default();
    write_to_buf(&html, buf, buf_len)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_get_node_tag(
    session: *mut XrSessionContext,
    panel_id: u32,
    node_id: u32,
    buf: *mut c_char,
    buf_len: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    let tag = (*session)
        .get_panel(panel_id)
        .and_then(|p| p.nodes.get(&node_id))
        .map(|n| n.tag.as_str())
        .unwrap_or("");
    write_to_buf(tag, buf, buf_len)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_get_text_content(
    session: *mut XrSessionContext,
    panel_id: u32,
    node_id: u32,
    buf: *mut c_char,
    buf_len: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    let text = (*session)
        .get_panel(panel_id)
        .and_then(|p| p.nodes.get(&node_id))
        .and_then(|n| match &n.kind {
            HeadlessNodeKind::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .unwrap_or("");
    write_to_buf(text, buf, buf_len)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_get_attribute_value(
    session: *mut XrSessionContext,
    panel_id: u32,
    node_id: u32,
    name_ptr: *const c_char,
    name_len: u32,
    buf: *mut c_char,
    buf_len: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    let name = read_str(name_ptr, name_len);
    let value = (*session)
        .get_panel(panel_id)
        .and_then(|p| p.nodes.get(&node_id))
        .and_then(|n| n.attributes.get(name))
        .map(|v| v.as_str())
        .unwrap_or("");
    write_to_buf(value, buf, buf_len)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_inject_event(
    session: *mut XrSessionContext,
    panel_id: u32,
    handler_id: u32,
    event_type: u8,
    value_ptr: *const c_char,
    value_len: u32,
) {
    if session.is_null() {
        return;
    }
    let value = read_str(value_ptr, value_len).to_string();
    (*session).events.push(InternalEvent {
        panel_id,
        handler_id,
        event_type,
        value,
        hit_u: -1.0,
        hit_v: -1.0,
        hand: 0,
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_version(buf: *mut c_char, buf_len: u32) -> u32 {
    write_to_buf(VERSION, buf, buf_len)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_mount_point_id(
    session: *mut XrSessionContext,
    panel_id: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    (*session)
        .get_panel(panel_id)
        .map(|p| p.mount_point_id)
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mxr_panel_get_child_mojo_id(
    session: *mut XrSessionContext,
    panel_id: u32,
    parent_id: u32,
    index: u32,
) -> u32 {
    if session.is_null() {
        return 0;
    }
    (*session)
        .get_panel(panel_id)
        .and_then(|p| {
            let child_internal = p.child_at(parent_id, index)?;
            p.id_map.get_mojo(child_internal)
        })
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Session lifecycle ──────────────────────────────────────────────

    #[test]
    fn headless_session_lifecycle() {
        unsafe {
            let session = mxr_create_headless();
            assert!(!session.is_null());
            assert_eq!(mxr_is_alive(session), 1);
            assert_eq!(mxr_session_state(session), MXR_STATE_FOCUSED);
            mxr_destroy_session(session);
        }
    }

    #[test]
    fn named_session_lifecycle() {
        unsafe {
            let name = "Test App";
            let session = mxr_create_session(name.as_ptr() as *const c_char, name.len() as u32);
            assert!(!session.is_null());
            assert_eq!(mxr_is_alive(session), 1);
            assert_eq!(mxr_session_state(session), MXR_STATE_IDLE);
            mxr_destroy_session(session);
        }
    }

    #[test]
    fn null_session_safety() {
        unsafe {
            assert_eq!(mxr_is_alive(std::ptr::null_mut()), 0);
            assert_eq!(mxr_session_state(std::ptr::null_mut()), MXR_STATE_EXITING);
            assert_eq!(mxr_create_panel(std::ptr::null_mut(), 100, 100), 0);
            assert_eq!(mxr_panel_count(std::ptr::null_mut()), 0);
            mxr_destroy_session(std::ptr::null_mut()); // Should not crash.
        }
    }

    // ── Panel lifecycle ───────────────────────────────────────────────

    #[test]
    fn create_and_destroy_panel() {
        unsafe {
            let session = mxr_create_headless();
            assert_eq!(mxr_panel_count(session), 0);

            let p1 = mxr_create_panel(session, 960, 720);
            assert_ne!(p1, 0);
            assert_eq!(mxr_panel_count(session), 1);
            assert_eq!(mxr_panel_is_visible(session, p1), 1);

            let p2 = mxr_create_panel(session, 480, 360);
            assert_ne!(p2, 0);
            assert_ne!(p1, p2);
            assert_eq!(mxr_panel_count(session), 2);

            mxr_destroy_panel(session, p1);
            assert_eq!(mxr_panel_count(session), 1);
            assert_eq!(mxr_panel_is_visible(session, p1), 0); // Destroyed panel

            mxr_destroy_session(session);
        }
    }

    #[test]
    fn panel_visibility() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);

            assert_eq!(mxr_panel_is_visible(session, p), 1);
            mxr_panel_set_visible(session, p, 0);
            assert_eq!(mxr_panel_is_visible(session, p), 0);
            mxr_panel_set_visible(session, p, 1);
            assert_eq!(mxr_panel_is_visible(session, p), 1);

            mxr_destroy_session(session);
        }
    }

    #[test]
    fn zero_dimension_panel_rejected() {
        unsafe {
            let session = mxr_create_headless();
            assert_eq!(mxr_create_panel(session, 0, 100), 0);
            assert_eq!(mxr_create_panel(session, 100, 0), 0);
            assert_eq!(mxr_panel_count(session), 0);
            mxr_destroy_session(session);
        }
    }

    // ── DOM operations ────────────────────────────────────────────────

    #[test]
    fn create_elements_and_append() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);
            let mount = mxr_panel_mount_point_id(session, p);
            assert_ne!(mount, 0);

            let tag = "div";
            let div = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );
            assert_ne!(div, 0);

            let ids = [div];
            mxr_panel_append_children(session, p, mount, ids.as_ptr(), 1);
            assert_eq!(mxr_panel_child_count(session, p, mount), 1);
            assert_eq!(mxr_panel_child_at(session, p, mount, 0), div);

            mxr_destroy_session(session);
        }
    }

    #[test]
    fn text_node_operations() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);

            let text = "Hello, XR!";
            let node = mxr_panel_create_text_node(
                session,
                p,
                text.as_ptr() as *const c_char,
                text.len() as u32,
            );
            assert_ne!(node, 0);

            // Read back text content
            let mut buf = [0u8; 64];
            let len =
                mxr_panel_get_text_content(session, p, node, buf.as_mut_ptr() as *mut c_char, 64);
            let content = std::str::from_utf8(&buf[..len as usize]).unwrap();
            assert_eq!(content, "Hello, XR!");

            // Update text
            let new_text = "Updated!";
            mxr_panel_set_text_content(
                session,
                p,
                node,
                new_text.as_ptr() as *const c_char,
                new_text.len() as u32,
            );
            let len =
                mxr_panel_get_text_content(session, p, node, buf.as_mut_ptr() as *mut c_char, 64);
            let content = std::str::from_utf8(&buf[..len as usize]).unwrap();
            assert_eq!(content, "Updated!");

            mxr_destroy_session(session);
        }
    }

    #[test]
    fn set_and_get_attribute() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);

            let tag = "button";
            let btn = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );

            let name = "class";
            let value = "primary";
            mxr_panel_set_attribute(
                session,
                p,
                btn,
                name.as_ptr() as *const c_char,
                name.len() as u32,
                value.as_ptr() as *const c_char,
                value.len() as u32,
            );

            let mut buf = [0u8; 64];
            let len = mxr_panel_get_attribute_value(
                session,
                p,
                btn,
                name.as_ptr() as *const c_char,
                name.len() as u32,
                buf.as_mut_ptr() as *mut c_char,
                64,
            );
            let attr = std::str::from_utf8(&buf[..len as usize]).unwrap();
            assert_eq!(attr, "primary");

            // Remove attribute
            mxr_panel_remove_attribute(
                session,
                p,
                btn,
                name.as_ptr() as *const c_char,
                name.len() as u32,
            );
            let len = mxr_panel_get_attribute_value(
                session,
                p,
                btn,
                name.as_ptr() as *const c_char,
                name.len() as u32,
                buf.as_mut_ptr() as *mut c_char,
                64,
            );
            assert_eq!(len, 0);

            mxr_destroy_session(session);
        }
    }

    #[test]
    fn insert_before_and_after() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);
            let mount = mxr_panel_mount_point_id(session, p);

            let tag = "span";
            let a = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );
            let b = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );
            let c = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );

            // Append a to mount: [a]
            mxr_panel_append_children(session, p, mount, [a].as_ptr(), 1);
            assert_eq!(mxr_panel_child_count(session, p, mount), 1);

            // Insert b before a: [b, a]
            mxr_panel_insert_before(session, p, a, [b].as_ptr(), 1);
            assert_eq!(mxr_panel_child_count(session, p, mount), 2);
            assert_eq!(mxr_panel_child_at(session, p, mount, 0), b);
            assert_eq!(mxr_panel_child_at(session, p, mount, 1), a);

            // Insert c after b: [b, c, a]
            mxr_panel_insert_after(session, p, b, [c].as_ptr(), 1);
            assert_eq!(mxr_panel_child_count(session, p, mount), 3);
            assert_eq!(mxr_panel_child_at(session, p, mount, 0), b);
            assert_eq!(mxr_panel_child_at(session, p, mount, 1), c);
            assert_eq!(mxr_panel_child_at(session, p, mount, 2), a);

            mxr_destroy_session(session);
        }
    }

    #[test]
    fn replace_with_and_remove() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);
            let mount = mxr_panel_mount_point_id(session, p);

            let tag = "div";
            let a = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );
            let b = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );

            mxr_panel_append_children(session, p, mount, [a].as_ptr(), 1);
            assert_eq!(mxr_panel_child_count(session, p, mount), 1);

            // Replace a with b
            mxr_panel_replace_with(session, p, a, [b].as_ptr(), 1);
            assert_eq!(mxr_panel_child_count(session, p, mount), 1);
            assert_eq!(mxr_panel_child_at(session, p, mount, 0), b);

            // Remove b
            mxr_panel_remove_node(session, p, b);
            assert_eq!(mxr_panel_child_count(session, p, mount), 0);

            mxr_destroy_session(session);
        }
    }

    // ── Events ────────────────────────────────────────────────────────

    #[test]
    fn event_inject_and_poll() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);

            assert_eq!(mxr_event_count(session), 0);

            let value = "test value";
            mxr_panel_inject_event(
                session,
                p,
                42,
                MXR_EVT_INPUT,
                value.as_ptr() as *const c_char,
                value.len() as u32,
            );
            assert_eq!(mxr_event_count(session), 1);

            let evt = mxr_poll_event(session);
            assert_eq!(evt.valid, 1);
            assert_eq!(evt.panel_id, p);
            assert_eq!(evt.handler_id, 42);
            assert_eq!(evt.event_type, MXR_EVT_INPUT);
            assert_eq!(evt.value_len, value.len() as u32);

            let polled_value = std::str::from_utf8(std::slice::from_raw_parts(
                evt.value_ptr as *const u8,
                evt.value_len as usize,
            ))
            .unwrap();
            assert_eq!(polled_value, "test value");

            // Queue is empty now
            let evt2 = mxr_poll_event(session);
            assert_eq!(evt2.valid, 0);

            mxr_destroy_session(session);
        }
    }

    #[test]
    fn event_listener_registration() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);
            let mount = mxr_panel_mount_point_id(session, p);

            let tag = "button";
            let btn = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );
            mxr_panel_append_children(session, p, mount, [btn].as_ptr(), 1);

            mxr_panel_add_event_listener(session, p, btn, 7, MXR_EVT_CLICK);

            let panel = (*session).get_panel(p).unwrap();
            assert_eq!(panel.listeners.len(), 1);
            assert_eq!(panel.listeners[0].handler_id, 7);

            mxr_panel_remove_event_listener(session, p, btn, 7, MXR_EVT_CLICK);
            let panel = (*session).get_panel(p).unwrap();
            assert_eq!(panel.listeners.len(), 0);

            mxr_destroy_session(session);
        }
    }

    // ── Raycasting ────────────────────────────────────────────────────

    #[test]
    fn raycast_hits_panel() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 960, 720);

            // Place panel at (0, 1.4, -1.0), facing the user (identity rotation)
            mxr_panel_set_transform(session, p, 0.0, 1.4, -1.0, 0.0, 0.0, 0.0, 1.0);
            mxr_panel_set_size(session, p, 0.8, 0.6);

            // Cast a ray from origin toward the panel center
            let hit = mxr_raycast_panels(session, 0.0, 1.4, 0.0, 0.0, 0.0, -1.0);
            assert_eq!(hit.hit, 1);
            assert_eq!(hit.panel_id, p);
            assert!(
                (hit.u - 0.5).abs() < 0.01,
                "u should be ~0.5, got {}",
                hit.u
            );
            assert!(
                (hit.v - 0.5).abs() < 0.01,
                "v should be ~0.5, got {}",
                hit.v
            );
            assert!((hit.distance - 1.0).abs() < 0.01);

            mxr_destroy_session(session);
        }
    }

    #[test]
    fn raycast_misses_when_outside_bounds() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 960, 720);
            mxr_panel_set_transform(session, p, 0.0, 1.4, -1.0, 0.0, 0.0, 0.0, 1.0);
            mxr_panel_set_size(session, p, 0.8, 0.6);

            // Cast a ray that misses (far to the right)
            let hit = mxr_raycast_panels(session, 5.0, 1.4, 0.0, 0.0, 0.0, -1.0);
            assert_eq!(hit.hit, 0);

            mxr_destroy_session(session);
        }
    }

    #[test]
    fn raycast_skips_hidden_panels() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 960, 720);
            mxr_panel_set_transform(session, p, 0.0, 1.4, -1.0, 0.0, 0.0, 0.0, 1.0);
            mxr_panel_set_size(session, p, 0.8, 0.6);

            mxr_panel_set_visible(session, p, 0);
            let hit = mxr_raycast_panels(session, 0.0, 1.4, 0.0, 0.0, 0.0, -1.0);
            assert_eq!(hit.hit, 0);

            mxr_destroy_session(session);
        }
    }

    // ── Focus ─────────────────────────────────────────────────────────

    #[test]
    fn focus_management() {
        unsafe {
            let session = mxr_create_headless();
            let p1 = mxr_create_panel(session, 100, 100);
            let p2 = mxr_create_panel(session, 100, 100);

            // First panel gets auto-focus
            assert_eq!(mxr_get_focused_panel(session), p1);

            mxr_set_focused_panel(session, p2);
            assert_eq!(mxr_get_focused_panel(session), p2);

            mxr_set_focused_panel(session, 0);
            assert_eq!(mxr_get_focused_panel(session), 0);

            mxr_destroy_session(session);
        }
    }

    // ── Frame loop ────────────────────────────────────────────────────

    #[test]
    fn headless_frame_loop() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);

            let ts = mxr_wait_frame(session);
            assert!(ts > 0);

            assert_eq!(mxr_begin_frame(session), 1);

            // Mark panel dirty via end_mutations
            mxr_panel_begin_mutations(session, p);
            mxr_panel_end_mutations(session, p);

            let rendered = mxr_render_dirty_panels(session);
            assert_eq!(rendered, 1);

            // Second render should find nothing dirty
            let rendered = mxr_render_dirty_panels(session);
            assert_eq!(rendered, 0);

            mxr_end_frame(session);
            mxr_destroy_session(session);
        }
    }

    // ── Reference spaces ──────────────────────────────────────────────

    #[test]
    fn reference_space_management() {
        unsafe {
            let session = mxr_create_headless();

            assert_eq!(mxr_get_reference_space(session), MXR_SPACE_STAGE);
            assert_eq!(mxr_set_reference_space(session, MXR_SPACE_LOCAL), 1);
            assert_eq!(mxr_get_reference_space(session), MXR_SPACE_LOCAL);

            // Invalid space type
            assert_eq!(mxr_set_reference_space(session, 99), 0);
            assert_eq!(mxr_get_reference_space(session), MXR_SPACE_LOCAL);

            mxr_destroy_session(session);
        }
    }

    // ── Version ───────────────────────────────────────────────────────

    #[test]
    fn version_string() {
        unsafe {
            let mut buf = [0u8; 32];
            let len = mxr_version(buf.as_mut_ptr() as *mut c_char, 32);
            let version = std::str::from_utf8(&buf[..len as usize]).unwrap();
            assert_eq!(version, VERSION);
        }
    }

    // ── Serialization ─────────────────────────────────────────────────

    #[test]
    fn serialize_subtree() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);
            let mount = mxr_panel_mount_point_id(session, p);

            let tag = "p";
            let para = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );
            let text = "Hello, world!";
            let txt = mxr_panel_create_text_node(
                session,
                p,
                text.as_ptr() as *const c_char,
                text.len() as u32,
            );

            mxr_panel_append_children(session, p, mount, [para].as_ptr(), 1);
            mxr_panel_append_children(session, p, para, [txt].as_ptr(), 1);

            let mut buf = [0u8; 256];
            let len = mxr_panel_serialize_subtree(session, p, buf.as_mut_ptr() as *mut c_char, 256);
            let html = std::str::from_utf8(&buf[..len as usize]).unwrap();
            assert_eq!(html, "<div><p>Hello, world!</p></div>");

            mxr_destroy_session(session);
        }
    }

    // ── Placeholder nodes ─────────────────────────────────────────────

    #[test]
    fn placeholder_node() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);
            let mount = mxr_panel_mount_point_id(session, p);

            let ph = mxr_panel_create_placeholder(session, p);
            assert_ne!(ph, 0);

            mxr_panel_append_children(session, p, mount, [ph].as_ptr(), 1);

            let mut buf = [0u8; 256];
            let len = mxr_panel_serialize_subtree(session, p, buf.as_mut_ptr() as *mut c_char, 256);
            let html = std::str::from_utf8(&buf[..len as usize]).unwrap();
            assert_eq!(html, "<div><!--placeholder--></div>");

            mxr_destroy_session(session);
        }
    }

    // ── Node path navigation ──────────────────────────────────────────

    #[test]
    fn node_at_path() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);
            let mount = mxr_panel_mount_point_id(session, p);

            let tag = "div";
            let outer = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );
            let inner = mxr_panel_create_element(
                session,
                p,
                tag.as_ptr() as *const c_char,
                tag.len() as u32,
            );

            mxr_panel_append_children(session, p, mount, [outer].as_ptr(), 1);
            mxr_panel_append_children(session, p, outer, [inner].as_ptr(), 1);

            // Navigate: mount → child 0 → child 0
            let path = [0u32, 0u32];
            let found = mxr_panel_node_at_path(session, p, mount, path.as_ptr(), 2);
            assert_eq!(found, inner);

            mxr_destroy_session(session);
        }
    }

    // ── Panel UA stylesheet storage ───────────────────────────────────

    #[test]
    fn ua_stylesheet() {
        unsafe {
            let session = mxr_create_headless();
            let p = mxr_create_panel(session, 100, 100);

            let css = "body { margin: 0; }";
            mxr_panel_add_ua_stylesheet(
                session,
                p,
                css.as_ptr() as *const c_char,
                css.len() as u32,
            );

            let panel = (*session).get_panel(p).unwrap();
            assert_eq!(panel.ua_stylesheets.len(), 1);
            assert_eq!(panel.ua_stylesheets[0], "body { margin: 0; }");

            mxr_destroy_session(session);
        }
    }
}
