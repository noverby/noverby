//! mojo-blitz-shim — C FFI shim for the Blitz HTML/CSS rendering engine.
//!
//! This Rust cdylib exposes Blitz's DOM manipulation, event handling, and
//! rendering pipeline via a flat C ABI so that Mojo can drive it through
//! `DLHandle` FFI calls.
//!
//! Architecture:
//!   Mojo → extern "C" calls → BlitzContext → blitz-dom / blitz-shell / blitz-paint
//!
//! Key design decisions:
//!   - Polling-based: no callbacks across FFI. Events are buffered in a ring buffer.
//!   - Node IDs are u32 (mapped from Blitz's usize slab keys).
//!   - Templates are stored as detached DOM subtrees, deep-cloned on use.
//!   - All functions must be called from the main/UI thread.

use std::collections::HashMap;
use std::ffi::CStr;
use std::slice;
use std::sync::Arc;

use blitz_dom::{Attribute, BaseDocument, DocumentConfig, ElementData, NodeData};
use blitz_traits::shell::Viewport;
use markup5ever::{local_name, ns, LocalName, Namespace, Prefix, QualName};

// ═══════════════════════════════════════════════════════════════════════════
// Event types — must match mojo-gui/core event type constants
// ═══════════════════════════════════════════════════════════════════════════

const EVT_CLICK: u8 = 0;
const EVT_INPUT: u8 = 1;
const _EVT_CHANGE: u8 = 2;
const _EVT_KEYDOWN: u8 = 3;
const _EVT_KEYUP: u8 = 4;
const _EVT_FOCUS: u8 = 5;
const _EVT_BLUR: u8 = 6;
const _EVT_SUBMIT: u8 = 7;
const _EVT_MOUSEDOWN: u8 = 8;
const _EVT_MOUSEUP: u8 = 9;
const _EVT_MOUSEMOVE: u8 = 10;

// ═══════════════════════════════════════════════════════════════════════════
// Buffered event
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
struct BufferedEvent {
    handler_id: u32,
    event_type: u8,
    value: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Event handler registration
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
struct EventHandler {
    handler_id: u32,
    event_name: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// BlitzContext — owns the document, templates, event queue, and node ID map
// ═══════════════════════════════════════════════════════════════════════════

struct BlitzContext {
    /// The Blitz DOM document.
    doc: BaseDocument,

    /// Node ID assigned to the mount point (the <body> or <div id="root">).
    /// mojo-gui's element ID 0 maps to this.
    mount_point_id: usize,

    /// Map from mojo-gui element IDs (u32) to Blitz slab node IDs (usize).
    /// Element ID 0 is always the mount point.
    id_to_node: HashMap<u32, usize>,

    /// Reverse map: Blitz node ID → mojo-gui element ID.
    /// Used for event dispatch (we need to find handler_id from the clicked node).
    node_to_id: HashMap<usize, u32>,

    /// Next available mojo-gui element ID for internally created nodes
    /// that don't get an explicit AssignId.
    next_internal_id: u32,

    /// Registered templates: template_id → root Blitz node ID.
    templates: HashMap<u32, usize>,

    /// Event handlers: Blitz node ID → list of handlers.
    event_handlers: HashMap<usize, Vec<EventHandler>>,

    /// Buffered events ready for Mojo to poll.
    event_queue: Vec<BufferedEvent>,

    /// Temporary storage for the last polled event's value string,
    /// kept alive until the next poll.
    last_polled_value: String,

    /// Stack for mutation interpretation (mirrors the JS interpreter's stack).
    /// Contains Blitz node IDs.
    stack: Vec<usize>,

    /// Whether the window is still alive.
    alive: bool,

    /// Debug mode flag.
    debug: bool,

    /// Whether we're currently inside a begin_mutations/end_mutations batch.
    in_mutation_batch: bool,
}

impl BlitzContext {
    fn new(title: &str, width: u32, height: u32, debug: bool) -> Self {
        let viewport = Viewport {
            window_size: (width, height),
            ..Default::default()
        };

        let config = DocumentConfig {
            viewport: Some(viewport),
            ..Default::default()
        };

        let mut doc = BaseDocument::new(config);

        // Build a minimal DOM structure: Document → <html> → <body>
        // The document root (node 0) is created by BaseDocument::new().
        // We need to create <html> and <body> elements.
        let html_name = QualName::new(None::<Prefix>, ns!(html), local_name!("html"));
        let html_id = doc.create_node(NodeData::Element(ElementData::new(html_name, vec![])));

        let body_name = QualName::new(None::<Prefix>, ns!(html), local_name!("body"));
        let body_id = doc.create_node(NodeData::Element(ElementData::new(body_name, vec![])));

        // Attach <html> to document root, <body> to <html>
        {
            let mut mutator = doc.mutate();
            mutator.append_children(0, &[html_id]);
            mutator.append_children(html_id, &[body_id]);
        }

        // Set window title via a <title> element
        if !title.is_empty() {
            let head_name = QualName::new(None::<Prefix>, ns!(html), local_name!("head"));
            let head_id = doc.create_node(NodeData::Element(ElementData::new(head_name, vec![])));

            let title_name = QualName::new(None::<Prefix>, ns!(html), local_name!("title"));
            let title_el_id =
                doc.create_node(NodeData::Element(ElementData::new(title_name, vec![])));
            let title_text_id = doc.create_text_node(title);

            let mut mutator = doc.mutate();
            mutator.insert_before(html_id, &[head_id]);
            mutator.append_children(head_id, &[title_el_id]);
            mutator.append_children(title_el_id, &[title_text_id]);
        }

        let mut id_to_node = HashMap::new();
        let mut node_to_id = HashMap::new();
        // Element ID 0 → body (mount point)
        id_to_node.insert(0, body_id);
        node_to_id.insert(body_id, 0);

        BlitzContext {
            doc,
            mount_point_id: body_id,
            id_to_node,
            node_to_id,
            next_internal_id: 0x8000_0000, // Internal IDs start high to avoid collision
            templates: HashMap::new(),
            event_handlers: HashMap::new(),
            event_queue: Vec::new(),
            last_polled_value: String::new(),
            stack: Vec::new(),
            alive: true,
            debug,
            in_mutation_batch: false,
        }
    }

    /// Resolve a mojo-gui element ID to a Blitz node ID.
    fn resolve_id(&self, mojo_id: u32) -> Option<usize> {
        self.id_to_node.get(&mojo_id).copied()
    }

    /// Assign a mojo-gui element ID to a Blitz node ID.
    fn assign_id(&mut self, mojo_id: u32, blitz_id: usize) {
        self.id_to_node.insert(mojo_id, blitz_id);
        self.node_to_id.insert(blitz_id, mojo_id);
    }

    /// Allocate an internal element ID for nodes that don't get explicit AssignId.
    fn alloc_internal_id(&mut self) -> u32 {
        let id = self.next_internal_id;
        self.next_internal_id += 1;
        id
    }

    /// Create an HTML element by tag name string.
    fn create_element(&mut self, tag: &str) -> usize {
        let local = LocalName::from(tag);
        let name = QualName::new(None::<Prefix>, ns!(html), local);
        self.doc
            .create_node(NodeData::Element(ElementData::new(name, vec![])))
    }

    /// Create a text node.
    fn create_text_node(&mut self, text: &str) -> usize {
        self.doc.create_text_node(text)
    }

    /// Create a comment/placeholder node.
    fn create_placeholder(&mut self) -> usize {
        self.doc.create_node(NodeData::Comment)
    }

    /// Set an attribute on a node (via DocumentMutator).
    fn set_attribute(&mut self, node_id: usize, name: &str, value: &str) {
        let qname = QualName::new(None::<Prefix>, ns!(), LocalName::from(name));
        let mut mutator = self.doc.mutate();
        mutator.set_attribute(node_id, qname, value);
    }

    /// Remove an attribute from a node.
    fn remove_attribute(&mut self, node_id: usize, name: &str) {
        let qname = QualName::new(None::<Prefix>, ns!(), LocalName::from(name));
        let mut mutator = self.doc.mutate();
        mutator.clear_attribute(node_id, qname);
    }

    /// Set text content of a text node.
    fn set_text_content(&mut self, node_id: usize, text: &str) {
        let mut mutator = self.doc.mutate();
        mutator.set_node_text(node_id, text);
    }

    /// Append children to a parent.
    fn append_children(&mut self, parent_id: usize, child_ids: &[usize]) {
        let mut mutator = self.doc.mutate();
        mutator.append_children(parent_id, child_ids);
    }

    /// Insert nodes before an anchor.
    fn insert_before(&mut self, anchor_id: usize, new_ids: &[usize]) {
        let mut mutator = self.doc.mutate();
        mutator.insert_nodes_before(anchor_id, new_ids);
    }

    /// Insert nodes after an anchor.
    fn insert_after(&mut self, anchor_id: usize, new_ids: &[usize]) {
        let mut mutator = self.doc.mutate();
        mutator.insert_nodes_after(anchor_id, new_ids);
    }

    /// Replace a node with new nodes.
    fn replace_with(&mut self, old_id: usize, new_ids: &[usize]) {
        let mut mutator = self.doc.mutate();
        mutator.replace_node_with(old_id, new_ids);
    }

    /// Remove and drop a node.
    fn remove_node(&mut self, node_id: usize) {
        // Clean up ID mappings
        if let Some(mojo_id) = self.node_to_id.remove(&node_id) {
            self.id_to_node.remove(&mojo_id);
        }
        self.event_handlers.remove(&node_id);

        let mut mutator = self.doc.mutate();
        mutator.remove_and_drop_node(node_id);
    }

    /// Deep clone a node.
    fn deep_clone_node(&mut self, node_id: usize) -> usize {
        self.doc.deep_clone_node(node_id)
    }

    /// Navigate to a child at path from a starting node.
    fn node_at_path(&self, start_id: usize, path: &[u8]) -> usize {
        let mutator = self.doc.mutate();
        mutator.node_at_path(start_id, path)
    }

    /// Add an event handler registration.
    fn add_event_listener(&mut self, node_id: usize, handler_id: u32, event_name: &str) {
        let handlers = self.event_handlers.entry(node_id).or_default();
        handlers.push(EventHandler {
            handler_id,
            event_name: event_name.to_string(),
        });
    }

    /// Remove an event handler registration.
    fn remove_event_listener(&mut self, node_id: usize, event_name: &str) {
        if let Some(handlers) = self.event_handlers.get_mut(&node_id) {
            handlers.retain(|h| h.event_name != event_name);
            if handlers.is_empty() {
                self.event_handlers.remove(&node_id);
            }
        }
    }

    /// Queue a synthetic event (for testing or programmatic dispatch).
    fn queue_event(&mut self, handler_id: u32, event_type: u8, value: String) {
        self.event_queue.push(BufferedEvent {
            handler_id,
            event_type,
            value,
        });
    }

    /// Poll the next event from the queue.
    fn poll_event(&mut self) -> Option<BufferedEvent> {
        if self.event_queue.is_empty() {
            None
        } else {
            Some(self.event_queue.remove(0))
        }
    }

    /// Push a node ID onto the interpreter stack.
    fn stack_push(&mut self, node_id: usize) {
        self.stack.push(node_id);
    }

    /// Pop N node IDs from the interpreter stack.
    fn stack_pop_n(&mut self, n: usize) -> Vec<usize> {
        let start = self.stack.len().saturating_sub(n);
        self.stack.drain(start..).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper: read a UTF-8 string from a pointer + length (no null terminator)
// ═══════════════════════════════════════════════════════════════════════════

unsafe fn str_from_ptr(ptr: *const u8, len: u32) -> &'static str {
    if ptr.is_null() || len == 0 {
        return "";
    }
    let bytes = unsafe { slice::from_raw_parts(ptr, len as usize) };
    std::str::from_utf8(bytes).unwrap_or("")
}

// ═══════════════════════════════════════════════════════════════════════════
// FFI event structure
// ═══════════════════════════════════════════════════════════════════════════

#[repr(C)]
pub struct MblitzEvent {
    pub valid: i32,
    pub handler_id: u32,
    pub event_type: u8,
    pub value_ptr: *const u8,
    pub value_len: u32,
}

impl Default for MblitzEvent {
    fn default() -> Self {
        MblitzEvent {
            valid: 0,
            handler_id: 0,
            event_type: 0,
            value_ptr: std::ptr::null(),
            value_len: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Lifecycle FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_create(
    title: *const u8,
    title_len: u32,
    width: u32,
    height: u32,
    debug: i32,
) -> *mut BlitzContext {
    let title_str = unsafe { str_from_ptr(title, title_len) };
    let ctx = BlitzContext::new(title_str, width, height, debug != 0);
    Box::into_raw(Box::new(ctx))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_destroy(ctx: *mut BlitzContext) {
    if !ctx.is_null() {
        drop(unsafe { Box::from_raw(ctx) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_is_alive(ctx: *mut BlitzContext) -> i32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &*ctx };
    if ctx.alive {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_step(ctx: *mut BlitzContext, blocking: i32) -> i32 {
    if ctx.is_null() {
        return 0;
    }
    // TODO: Integrate with Winit event loop.
    // For now, this is a placeholder. The actual Winit integration requires
    // running the event loop on the main thread with ApplicationHandler.
    // Phase 4 implementation will use blitz-shell's BlitzApplication.
    //
    // Current approach: the shim buffers mutations and events without a real
    // window. A follow-up commit will add the Winit/Vello rendering pipeline.
    let _ctx = unsafe { &mut *ctx };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_request_redraw(ctx: *mut BlitzContext) {
    if ctx.is_null() {
        return;
    }
    // TODO: Trigger Winit redraw request when window integration is complete.
    let _ctx = unsafe { &*ctx };
}

// ═══════════════════════════════════════════════════════════════════════════
// Window management FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_set_title(
    ctx: *mut BlitzContext,
    title: *const u8,
    title_len: u32,
) {
    if ctx.is_null() {
        return;
    }
    let _title = unsafe { str_from_ptr(title, title_len) };
    // TODO: Update the Winit window title when window integration is complete.
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_set_size(ctx: *mut BlitzContext, width: u32, height: u32) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let viewport = Viewport {
        window_size: (width, height),
        ..Default::default()
    };
    ctx.doc.set_viewport(viewport);
}

// ═══════════════════════════════════════════════════════════════════════════
// User-agent stylesheet FFI
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_add_ua_stylesheet(
    ctx: *mut BlitzContext,
    css: *const u8,
    css_len: u32,
) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let css_str = unsafe { str_from_ptr(css, css_len) };
    ctx.doc.add_user_agent_stylesheet(css_str);
}

// ═══════════════════════════════════════════════════════════════════════════
// DOM node creation FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_create_element(
    ctx: *mut BlitzContext,
    tag: *const u8,
    tag_len: u32,
) -> u32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &mut *ctx };
    let tag_str = unsafe { str_from_ptr(tag, tag_len) };
    let node_id = ctx.create_element(tag_str);
    node_id as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_create_text_node(
    ctx: *mut BlitzContext,
    text: *const u8,
    text_len: u32,
) -> u32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &mut *ctx };
    let text_str = unsafe { str_from_ptr(text, text_len) };
    let node_id = ctx.create_text_node(text_str);
    node_id as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_create_placeholder(ctx: *mut BlitzContext) -> u32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &mut *ctx };
    let node_id = ctx.create_placeholder();
    node_id as u32
}

// ═══════════════════════════════════════════════════════════════════════════
// Template FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_register_template(
    ctx: *mut BlitzContext,
    tmpl_id: u32,
    root_id: u32,
) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    ctx.templates.insert(tmpl_id, root_id as usize);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_clone_template(ctx: *mut BlitzContext, tmpl_id: u32) -> u32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &mut *ctx };
    let Some(&root_id) = ctx.templates.get(&tmpl_id) else {
        return 0;
    };
    let cloned = ctx.deep_clone_node(root_id);
    cloned as u32
}

// ═══════════════════════════════════════════════════════════════════════════
// DOM tree mutation FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_append_children(
    ctx: *mut BlitzContext,
    parent_id: u32,
    child_ids: *const u32,
    child_count: u32,
) {
    if ctx.is_null() || child_ids.is_null() || child_count == 0 {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let ids_slice = unsafe { slice::from_raw_parts(child_ids, child_count as usize) };

    // Resolve the parent: mojo element ID 0 → mount point
    let parent_blitz = if parent_id == 0 {
        ctx.mount_point_id
    } else {
        ctx.resolve_id(parent_id).unwrap_or(parent_id as usize)
    };

    // Child IDs may be raw Blitz node IDs (from create_element) or
    // mojo element IDs (from the stack). Try resolving, fall back to raw.
    let blitz_children: Vec<usize> = ids_slice
        .iter()
        .map(|&id| ctx.resolve_id(id).unwrap_or(id as usize))
        .collect();

    ctx.append_children(parent_blitz, &blitz_children);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_insert_before(
    ctx: *mut BlitzContext,
    anchor_id: u32,
    new_ids: *const u32,
    new_count: u32,
) {
    if ctx.is_null() || new_ids.is_null() || new_count == 0 {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let ids_slice = unsafe { slice::from_raw_parts(new_ids, new_count as usize) };
    let anchor_blitz = ctx.resolve_id(anchor_id).unwrap_or(anchor_id as usize);
    let blitz_new: Vec<usize> = ids_slice
        .iter()
        .map(|&id| ctx.resolve_id(id).unwrap_or(id as usize))
        .collect();
    ctx.insert_before(anchor_blitz, &blitz_new);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_insert_after(
    ctx: *mut BlitzContext,
    anchor_id: u32,
    new_ids: *const u32,
    new_count: u32,
) {
    if ctx.is_null() || new_ids.is_null() || new_count == 0 {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let ids_slice = unsafe { slice::from_raw_parts(new_ids, new_count as usize) };
    let anchor_blitz = ctx.resolve_id(anchor_id).unwrap_or(anchor_id as usize);
    let blitz_new: Vec<usize> = ids_slice
        .iter()
        .map(|&id| ctx.resolve_id(id).unwrap_or(id as usize))
        .collect();
    ctx.insert_after(anchor_blitz, &blitz_new);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_replace_with(
    ctx: *mut BlitzContext,
    old_id: u32,
    new_ids: *const u32,
    new_count: u32,
) {
    if ctx.is_null() || new_ids.is_null() || new_count == 0 {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let ids_slice = unsafe { slice::from_raw_parts(new_ids, new_count as usize) };
    let old_blitz = ctx.resolve_id(old_id).unwrap_or(old_id as usize);
    let blitz_new: Vec<usize> = ids_slice
        .iter()
        .map(|&id| ctx.resolve_id(id).unwrap_or(id as usize))
        .collect();

    // Clean up ID mapping for old node
    if let Some(mojo_id) = ctx.node_to_id.remove(&old_blitz) {
        ctx.id_to_node.remove(&mojo_id);
    }
    ctx.event_handlers.remove(&old_blitz);

    ctx.replace_with(old_blitz, &blitz_new);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_remove_node(ctx: *mut BlitzContext, node_id: u32) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let blitz_id = ctx.resolve_id(node_id).unwrap_or(node_id as usize);
    ctx.remove_node(blitz_id);
}

// ═══════════════════════════════════════════════════════════════════════════
// DOM attributes FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_set_attribute(
    ctx: *mut BlitzContext,
    node_id: u32,
    name: *const u8,
    name_len: u32,
    value: *const u8,
    value_len: u32,
) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let name_str = unsafe { str_from_ptr(name, name_len) };
    let value_str = unsafe { str_from_ptr(value, value_len) };
    let blitz_id = ctx.resolve_id(node_id).unwrap_or(node_id as usize);
    ctx.set_attribute(blitz_id, name_str, value_str);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_remove_attribute(
    ctx: *mut BlitzContext,
    node_id: u32,
    name: *const u8,
    name_len: u32,
) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let name_str = unsafe { str_from_ptr(name, name_len) };
    let blitz_id = ctx.resolve_id(node_id).unwrap_or(node_id as usize);
    ctx.remove_attribute(blitz_id, name_str);
}

// ═══════════════════════════════════════════════════════════════════════════
// DOM text content FFI export
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_set_text_content(
    ctx: *mut BlitzContext,
    node_id: u32,
    text: *const u8,
    text_len: u32,
) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let text_str = unsafe { str_from_ptr(text, text_len) };
    let blitz_id = ctx.resolve_id(node_id).unwrap_or(node_id as usize);
    ctx.set_text_content(blitz_id, text_str);
}

// ═══════════════════════════════════════════════════════════════════════════
// DOM tree traversal FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_node_at_path(
    ctx: *mut BlitzContext,
    start_id: u32,
    path: *const u8,
    path_len: u32,
) -> u32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &mut *ctx };
    let start_blitz = ctx.resolve_id(start_id).unwrap_or(start_id as usize);
    if path.is_null() || path_len == 0 {
        return start_blitz as u32;
    }
    let path_slice = unsafe { slice::from_raw_parts(path, path_len as usize) };
    let result = ctx.node_at_path(start_blitz, path_slice);
    result as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_child_at(ctx: *mut BlitzContext, node_id: u32, index: u32) -> u32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &*ctx };
    let blitz_id = ctx
        .id_to_node
        .get(&node_id)
        .copied()
        .unwrap_or(node_id as usize);
    let node = match ctx.doc.get_node(blitz_id) {
        Some(n) => n,
        None => return 0,
    };
    match node.children.get(index as usize) {
        Some(&child_id) => child_id as u32,
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_child_count(ctx: *mut BlitzContext, node_id: u32) -> u32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &*ctx };
    let blitz_id = ctx
        .id_to_node
        .get(&node_id)
        .copied()
        .unwrap_or(node_id as usize);
    let node = match ctx.doc.get_node(blitz_id) {
        Some(n) => n,
        None => return 0,
    };
    node.children.len() as u32
}

// ═══════════════════════════════════════════════════════════════════════════
// Event handling FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_add_event_listener(
    ctx: *mut BlitzContext,
    node_id: u32,
    handler_id: u32,
    event_name: *const u8,
    event_name_len: u32,
) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let name = unsafe { str_from_ptr(event_name, event_name_len) };
    let blitz_id = ctx.resolve_id(node_id).unwrap_or(node_id as usize);
    ctx.add_event_listener(blitz_id, handler_id, name);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_remove_event_listener(
    ctx: *mut BlitzContext,
    node_id: u32,
    event_name: *const u8,
    event_name_len: u32,
) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let name = unsafe { str_from_ptr(event_name, event_name_len) };
    let blitz_id = ctx.resolve_id(node_id).unwrap_or(node_id as usize);
    ctx.remove_event_listener(blitz_id, name);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_poll_event(ctx: *mut BlitzContext) -> MblitzEvent {
    if ctx.is_null() {
        return MblitzEvent::default();
    }
    let ctx = unsafe { &mut *ctx };
    match ctx.poll_event() {
        Some(event) => {
            ctx.last_polled_value = event.value.clone();
            MblitzEvent {
                valid: 1,
                handler_id: event.handler_id,
                event_type: event.event_type,
                value_ptr: if ctx.last_polled_value.is_empty() {
                    std::ptr::null()
                } else {
                    ctx.last_polled_value.as_ptr()
                },
                value_len: ctx.last_polled_value.len() as u32,
            }
        }
        None => MblitzEvent::default(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_event_count(ctx: *mut BlitzContext) -> u32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &*ctx };
    ctx.event_queue.len() as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_event_clear(ctx: *mut BlitzContext) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    ctx.event_queue.clear();
}

// ═══════════════════════════════════════════════════════════════════════════
// Mutation batch FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_begin_mutations(ctx: *mut BlitzContext) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    ctx.in_mutation_batch = true;
    // Note: In the future, we could acquire a DocumentMutator here and hold
    // it for the duration of the batch. For now, each DOM operation creates
    // its own short-lived mutator (which is correct but slightly less
    // efficient due to repeated flush cycles).
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_end_mutations(ctx: *mut BlitzContext) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    ctx.in_mutation_batch = false;
    // Trigger any pending style/layout invalidations.
    // The DocumentMutator's Drop impl handles deferred processing (style
    // elements, linked stylesheets, images, fonts, title updates, etc.).
    // Since we create/drop mutators per-operation right now, this is
    // already handled. Once we hold a long-lived mutator for the batch,
    // we'll drop it here to trigger the flush.
}

// ═══════════════════════════════════════════════════════════════════════════
// Document root access FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_root_node_id(ctx: *mut BlitzContext) -> u32 {
    // The Blitz document root is always node 0.
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_mount_point_id(ctx: *mut BlitzContext) -> u32 {
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &*ctx };
    ctx.mount_point_id as u32
}

// ═══════════════════════════════════════════════════════════════════════════
// Layout FFI export
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_resolve_layout(ctx: *mut BlitzContext) {
    if ctx.is_null() {
        return;
    }
    // TODO: Call Blitz's layout resolution (style computation + Taffy layout).
    // This requires the full Blitz pipeline integration with blitz-paint.
}

// ═══════════════════════════════════════════════════════════════════════════
// Stack operations (used by the Mojo mutation interpreter)
// ═══════════════════════════════════════════════════════════════════════════

/// Push a node onto the interpreter stack.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_stack_push(ctx: *mut BlitzContext, node_id: u32) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let blitz_id = ctx.resolve_id(node_id).unwrap_or(node_id as usize);
    ctx.stack_push(blitz_id);
}

/// Pop N nodes from the stack and append them as children of the given parent.
/// This mirrors OP_APPEND_CHILDREN behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_stack_pop_append(
    ctx: *mut BlitzContext,
    parent_id: u32,
    count: u32,
) {
    if ctx.is_null() || count == 0 {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let parent_blitz = if parent_id == 0 {
        ctx.mount_point_id
    } else {
        ctx.resolve_id(parent_id).unwrap_or(parent_id as usize)
    };
    let children = ctx.stack_pop_n(count as usize);
    ctx.append_children(parent_blitz, &children);
}

/// Pop N nodes from the stack and use them to replace the node with the given ID.
/// This mirrors OP_REPLACE_WITH behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_stack_pop_replace(ctx: *mut BlitzContext, old_id: u32, count: u32) {
    if ctx.is_null() || count == 0 {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let old_blitz = ctx.resolve_id(old_id).unwrap_or(old_id as usize);
    let replacements = ctx.stack_pop_n(count as usize);

    // Clean up ID mapping for old node
    if let Some(mojo_id) = ctx.node_to_id.remove(&old_blitz) {
        ctx.id_to_node.remove(&mojo_id);
    }
    ctx.event_handlers.remove(&old_blitz);

    ctx.replace_with(old_blitz, &replacements);
}

/// Pop N nodes from the stack and insert them before the anchor node.
/// This mirrors OP_INSERT_BEFORE behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_stack_pop_insert_before(
    ctx: *mut BlitzContext,
    anchor_id: u32,
    count: u32,
) {
    if ctx.is_null() || count == 0 {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let anchor_blitz = ctx.resolve_id(anchor_id).unwrap_or(anchor_id as usize);
    let new_nodes = ctx.stack_pop_n(count as usize);
    ctx.insert_before(anchor_blitz, &new_nodes);
}

/// Pop N nodes from the stack and insert them after the anchor node.
/// This mirrors OP_INSERT_AFTER behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_stack_pop_insert_after(
    ctx: *mut BlitzContext,
    anchor_id: u32,
    count: u32,
) {
    if ctx.is_null() || count == 0 {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    let anchor_blitz = ctx.resolve_id(anchor_id).unwrap_or(anchor_id as usize);
    let new_nodes = ctx.stack_pop_n(count as usize);
    ctx.insert_after(anchor_blitz, &new_nodes);
}

/// Assign a mojo-gui element ID to a Blitz node ID.
/// Used by the mutation interpreter for OP_ASSIGN_ID.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_assign_id(
    ctx: *mut BlitzContext,
    mojo_id: u32,
    blitz_node_id: u32,
) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    ctx.assign_id(mojo_id, blitz_node_id as usize);
}

// ═══════════════════════════════════════════════════════════════════════════
// Debug / diagnostics FFI exports
// ═══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_print_tree(ctx: *mut BlitzContext) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &*ctx };
    ctx.doc.print_tree();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_set_debug_overlay(ctx: *mut BlitzContext, enabled: i32) {
    if ctx.is_null() {
        return;
    }
    let ctx = unsafe { &mut *ctx };
    ctx.debug = enabled != 0;
}

static VERSION: &str = concat!("mojo-blitz-shim ", env!("CARGO_PKG_VERSION"), "\0");

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mblitz_version(out_ptr: *mut *const u8, out_len: *mut u32) {
    if !out_ptr.is_null() {
        unsafe { *out_ptr = VERSION.as_ptr() };
    }
    if !out_len.is_null() {
        unsafe { *out_len = (VERSION.len() - 1) as u32 }; // Exclude null terminator
    }
}
