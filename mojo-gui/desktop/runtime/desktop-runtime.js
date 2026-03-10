// desktop-runtime.js — Standalone JS runtime for mojo-gui desktop webview.
//
// This is a self-contained version of the web runtime's Interpreter and
// EventBridge, adapted for desktop IPC. Instead of reading from WASM linear
// memory, the interpreter receives mutation buffers via base64-encoded strings
// sent from the native Mojo process through webview_eval().
//
// Communication channels:
//   Mutations (Mojo → JS):  window.__mojo_apply_mutations(base64String)
//   Events   (JS → Mojo):   window.mojo_post(JSON.stringify({h, t, v?}))
//
// The mojo_post function is injected by the C shim (mojo_webview.c) via
// WebKitUserContentManager's script message handler.
//
// This file is designed to be injected into the webview via mwv_init() or
// mwv_set_html() before any app content is rendered.

(function () {
  "use strict";

  // ── Protocol opcodes (must match mojo-gui/core/src/bridge/protocol.mojo) ──

  const Op = {
    End: 0x00,
    AppendChildren: 0x01,
    AssignId: 0x02,
    CreatePlaceholder: 0x03,
    CreateTextNode: 0x04,
    LoadTemplate: 0x05,
    ReplaceWith: 0x06,
    ReplacePlaceholder: 0x07,
    InsertAfter: 0x08,
    InsertBefore: 0x09,
    SetAttribute: 0x0a,
    SetText: 0x0b,
    NewEventListener: 0x0c,
    RemoveEventListener: 0x0d,
    Remove: 0x0e,
    PushRoot: 0x0f,
    RegisterTemplate: 0x10,
    RemoveAttribute: 0x11,
  };

  // ── Template node types (must match template.mojo) ────────────────────────

  const TNODE_ELEMENT = 0;
  const TNODE_TEXT = 1;
  const TNODE_DYNAMIC = 2;
  const TNODE_DYNAMIC_TEXT = 3;

  const TATTR_STATIC = 0;
  const TATTR_DYNAMIC = 1;

  // ── Event type tags (must match events/registry.mojo) ─────────────────────

  const EventType = {
    Click: 0,
    Input: 1,
    KeyDown: 2,
    KeyUp: 3,
    MouseMove: 4,
    Focus: 5,
    Blur: 6,
    Submit: 7,
    Change: 8,
    MouseDown: 9,
    MouseUp: 10,
    MouseEnter: 11,
    MouseLeave: 12,
    Custom: 255,
  };

  const EVENT_NAME_TO_TYPE = {
    click: EventType.Click,
    input: EventType.Input,
    keydown: EventType.KeyDown,
    keyup: EventType.KeyUp,
    mousemove: EventType.MouseMove,
    focus: EventType.Focus,
    blur: EventType.Blur,
    submit: EventType.Submit,
    change: EventType.Change,
    mousedown: EventType.MouseDown,
    mouseup: EventType.MouseUp,
    mouseenter: EventType.MouseEnter,
    mouseleave: EventType.MouseLeave,
  };

  // ── HTML tag names (must match html/tags.mojo TAG_* constants) ────────────

  const TAG_NAMES = [
    "div",
    "span",
    "p",
    "a",
    "button",
    "input",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "table",
    "tr",
    "td",
    "th",
    "thead",
    "tbody",
    "img",
    "label",
    "select",
    "option",
    "textarea",
    "nav",
    "header",
    "footer",
    "main",
    "section",
    "article",
    "aside",
    "details",
    "summary",
    "pre",
    "code",
  ];

  // ── Attribute namespace URIs ───────────────────────────────────────────────

  const NS_URIS = [
    null, // 0 = no namespace
    "http://www.w3.org/1999/xlink", // 1 = xlink
    "http://www.w3.org/XML/1998/namespace", // 2 = xml
    "http://www.w3.org/2000/xmlns/", // 3 = xmlns
  ];

  // ── Base64 decoder ────────────────────────────────────────────────────────

  /**
   * Decode a base64 string to an ArrayBuffer.
   * Uses atob() which is available in all webview environments.
   */
  function base64ToArrayBuffer(base64) {
    const binary = atob(base64);
    const len = binary.length;
    const bytes = new Uint8Array(len);
    for (let i = 0; i < len; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes.buffer;
  }

  // ── MutationReader ────────────────────────────────────────────────────────

  /**
   * Reads binary-encoded mutations from an ArrayBuffer.
   * Port of the web runtime's MutationReader (protocol.ts).
   */
  class MutationReader {
    constructor(buffer, byteOffset, byteLength) {
      this._view = new DataView(buffer, byteOffset, byteLength);
      this._offset = 0;
      this._length = byteLength;
    }

    _readU8() {
      if (this._offset >= this._length) return -1;
      const v = this._view.getUint8(this._offset);
      this._offset += 1;
      return v;
    }

    _readU16() {
      if (this._offset + 2 > this._length) return 0;
      const v = this._view.getUint16(this._offset, true);
      this._offset += 2;
      return v;
    }

    _readU32() {
      if (this._offset + 4 > this._length) return 0;
      const v = this._view.getUint32(this._offset, true);
      this._offset += 4;
      return v;
    }

    _readStr() {
      const len = this._readU32();
      if (this._offset + len > this._length) return "";
      const bytes = new Uint8Array(
        this._view.buffer,
        this._view.byteOffset + this._offset,
        len
      );
      this._offset += len;
      return new TextDecoder().decode(bytes);
    }

    _readShortStr() {
      const len = this._readU16();
      if (this._offset + len > this._length) return "";
      const bytes = new Uint8Array(
        this._view.buffer,
        this._view.byteOffset + this._offset,
        len
      );
      this._offset += len;
      return new TextDecoder().decode(bytes);
    }

    _readPath() {
      const len = this._readU8();
      const path = [];
      for (let i = 0; i < len; i++) {
        path.push(this._readU8());
      }
      return path;
    }

    /**
     * Read the next mutation from the buffer.
     * Returns null on End sentinel or buffer exhaustion.
     */
    next() {
      if (this._offset >= this._length) return null;

      const op = this._readU8();
      if (op < 0 || op === Op.End) return null;

      switch (op) {
        case Op.AppendChildren:
          return { op, id: this._readU32(), m: this._readU32() };

        case Op.AssignId:
          return { op, path: this._readPath(), id: this._readU32() };

        case Op.CreatePlaceholder:
          return { op, id: this._readU32() };

        case Op.CreateTextNode:
          return { op, id: this._readU32(), text: this._readStr() };

        case Op.LoadTemplate:
          return {
            op,
            tmplId: this._readU32(),
            index: this._readU32(),
            id: this._readU32(),
          };

        case Op.ReplaceWith:
          return { op, id: this._readU32(), m: this._readU32() };

        case Op.ReplacePlaceholder:
          return { op, path: this._readPath(), m: this._readU32() };

        case Op.InsertAfter:
          return { op, id: this._readU32(), m: this._readU32() };

        case Op.InsertBefore:
          return { op, id: this._readU32(), m: this._readU32() };

        case Op.SetAttribute:
          return {
            op,
            id: this._readU32(),
            ns: this._readU8(),
            name: this._readShortStr(),
            value: this._readStr(),
          };

        case Op.SetText:
          return { op, id: this._readU32(), text: this._readStr() };

        case Op.NewEventListener:
          return {
            op,
            id: this._readU32(),
            handlerId: this._readU32(),
            name: this._readShortStr(),
          };

        case Op.RemoveEventListener:
          return { op, id: this._readU32(), name: this._readShortStr() };

        case Op.Remove:
          return { op, id: this._readU32() };

        case Op.PushRoot:
          return { op, id: this._readU32() };

        case Op.RegisterTemplate:
          return this._readRegisterTemplate();

        case Op.RemoveAttribute:
          return {
            op,
            id: this._readU32(),
            ns: this._readU8(),
            name: this._readShortStr(),
          };

        default:
          throw new Error(`Unknown opcode: 0x${op.toString(16)}`);
      }
    }

    _readRegisterTemplate() {
      const tmplId = this._readU32();
      const name = this._readShortStr();
      const numRoots = this._readU32();
      const numNodes = this._readU32();
      const numAttrs = this._readU32();

      const nodes = [];
      for (let i = 0; i < numNodes; i++) {
        const nodeType = this._readU8();
        switch (nodeType) {
          case TNODE_ELEMENT: {
            const tag = this._readU32();
            const numChildren = this._readU32();
            const children = [];
            for (let c = 0; c < numChildren; c++) {
              children.push(this._readU32());
            }
            const numNodeAttrs = this._readU32();
            const attrs = [];
            for (let a = 0; a < numNodeAttrs; a++) {
              attrs.push(this._readU32());
            }
            nodes.push({
              type: "element",
              tag,
              children,
              attrs,
            });
            break;
          }
          case TNODE_TEXT: {
            const text = this._readStr();
            nodes.push({ type: "text", text });
            break;
          }
          case TNODE_DYNAMIC: {
            const dynIndex = this._readU32();
            nodes.push({ type: "dynamic", dynIndex });
            break;
          }
          case TNODE_DYNAMIC_TEXT: {
            const dynIndex = this._readU32();
            nodes.push({ type: "dynamic_text", dynIndex });
            break;
          }
          default:
            throw new Error(`Unknown template node type: ${nodeType}`);
        }
      }

      const attrDefs = [];
      for (let i = 0; i < numAttrs; i++) {
        const attrType = this._readU8();
        if (attrType === TATTR_STATIC) {
          const attrName = this._readShortStr();
          const attrValue = this._readStr();
          attrDefs.push({ type: "static", name: attrName, value: attrValue });
        } else if (attrType === TATTR_DYNAMIC) {
          const dynIndex = this._readU32();
          attrDefs.push({ type: "dynamic", dynIndex });
        }
      }

      const roots = [];
      for (let i = 0; i < numRoots; i++) {
        roots.push(this._readU32());
      }

      return {
        op: Op.RegisterTemplate,
        tmplId,
        name,
        nodes,
        attrs: attrDefs,
        roots,
      };
    }
  }

  // ── TemplateCache ─────────────────────────────────────────────────────────

  /**
   * Caches pre-built DOM DocumentFragments for templates.
   * When a LoadTemplate mutation arrives, we clone from the cache.
   */
  class TemplateCache {
    constructor(doc) {
      this._doc = doc;
      /** Map<templateId, DocumentFragment[]> — one fragment per root */
      this._cache = new Map();
      /** Map<templateId, templateDef> — for debugging / introspection */
      this._defs = new Map();
    }

    /**
     * Register a template definition (from RegisterTemplate mutation).
     * Builds the DOM fragment and caches it for future cloning.
     */
    register(tmpl) {
      this._defs.set(tmpl.tmplId, tmpl);
      const fragments = this._buildFragments(tmpl);
      this._cache.set(tmpl.tmplId, fragments);
    }

    /**
     * Clone and return a specific root fragment of a template.
     */
    clone(tmplId, rootIndex) {
      const fragments = this._cache.get(tmplId);
      if (!fragments) {
        throw new Error(`Template ${tmplId} not registered`);
      }
      if (rootIndex >= fragments.length) {
        throw new Error(
          `Template ${tmplId} root index ${rootIndex} out of range (${fragments.length} roots)`
        );
      }
      return fragments[rootIndex].cloneNode(true);
    }

    _buildFragments(tmpl) {
      const fragments = [];
      for (const rootIdx of tmpl.roots) {
        const frag = this._doc.createDocumentFragment();
        const node = this._buildNode(tmpl, rootIdx);
        frag.appendChild(node);
        fragments.push(frag);
      }
      return fragments;
    }

    _buildNode(tmpl, nodeIdx) {
      const nodeDef = tmpl.nodes[nodeIdx];
      switch (nodeDef.type) {
        case "element": {
          const tagName = TAG_NAMES[nodeDef.tag] || "div";
          const el = this._doc.createElement(tagName);

          // Apply static attributes.
          for (const attrIdx of nodeDef.attrs) {
            const attrDef = tmpl.attrs[attrIdx];
            if (attrDef && attrDef.type === "static") {
              el.setAttribute(attrDef.name, attrDef.value);
            }
            // Dynamic attributes are handled at runtime via SetAttribute.
          }

          // Build children.
          for (const childIdx of nodeDef.children) {
            const child = this._buildNode(tmpl, childIdx);
            el.appendChild(child);
          }
          return el;
        }
        case "text": {
          return this._doc.createTextNode(nodeDef.text);
        }
        case "dynamic": {
          // Dynamic node placeholder — will be replaced at runtime.
          return this._doc.createComment("dyn");
        }
        case "dynamic_text": {
          // Dynamic text — content set at runtime via SetText.
          return this._doc.createTextNode("");
        }
        default:
          return this._doc.createComment("unknown");
      }
    }
  }

  // ── Interpreter ───────────────────────────────────────────────────────────

  /**
   * Stack-based DOM mutation interpreter.
   * Port of the web runtime's Interpreter (interpreter.ts).
   */
  class Interpreter {
    constructor(root, templates, doc) {
      this._stack = [];
      this._nodes = new Map();
      this._templates = templates;
      this._doc = doc || root.ownerDocument;
      this._root = root;
      this._listeners = new Map();

      /** Callback for NewEventListener: (elementId, eventName, handlerId) => listener */
      this.onNewListener = null;

      // Register root element with id 0.
      this._nodes.set(0, root);
    }

    /**
     * Decode and apply all mutations from an ArrayBuffer.
     */
    applyMutations(buffer, byteOffset, byteLength) {
      const reader = new MutationReader(buffer, byteOffset, byteLength);
      for (let m = reader.next(); m !== null; m = reader.next()) {
        this._handleMutation(m);
      }
    }

    getNode(id) {
      return this._nodes.get(id);
    }

    _push(node) {
      this._stack.push(node);
    }

    _popMany(count) {
      if (count <= 0) return [];
      return this._stack.splice(this._stack.length - count, count);
    }

    _handleMutation(m) {
      switch (m.op) {
        case Op.PushRoot:
          this._opPushRoot(m);
          break;
        case Op.AppendChildren:
          this._opAppendChildren(m);
          break;
        case Op.CreateTextNode:
          this._opCreateTextNode(m);
          break;
        case Op.CreatePlaceholder:
          this._opCreatePlaceholder(m);
          break;
        case Op.LoadTemplate:
          this._opLoadTemplate(m);
          break;
        case Op.AssignId:
          this._opAssignId(m);
          break;
        case Op.SetAttribute:
          this._opSetAttribute(m);
          break;
        case Op.RemoveAttribute:
          this._opRemoveAttribute(m);
          break;
        case Op.SetText:
          this._opSetText(m);
          break;
        case Op.NewEventListener:
          this._opNewEventListener(m);
          break;
        case Op.RemoveEventListener:
          this._opRemoveEventListener(m);
          break;
        case Op.Remove:
          this._opRemove(m);
          break;
        case Op.ReplaceWith:
          this._opReplaceWith(m);
          break;
        case Op.ReplacePlaceholder:
          this._opReplacePlaceholder(m);
          break;
        case Op.InsertAfter:
          this._opInsertAfter(m);
          break;
        case Op.InsertBefore:
          this._opInsertBefore(m);
          break;
        case Op.RegisterTemplate:
          this._templates.register(m);
          break;
        default:
          console.warn(
            `[mojo-gui desktop] Unknown mutation op: 0x${m.op.toString(16)}`
          );
      }
    }

    _opPushRoot(m) {
      const node = this._nodes.get(m.id);
      if (!node) {
        console.error(`[mojo-gui desktop] PushRoot: unknown id ${m.id}`);
        return;
      }
      this._push(node);
    }

    _opAppendChildren(m) {
      const parent = this._nodes.get(m.id);
      if (!parent) {
        console.error(`[mojo-gui desktop] AppendChildren: unknown id ${m.id}`);
        return;
      }
      const children = this._popMany(m.m);
      for (const child of children) {
        parent.appendChild(child);
      }
    }

    _opCreateTextNode(m) {
      const node = this._doc.createTextNode(m.text);
      this._nodes.set(m.id, node);
      this._push(node);
    }

    _opCreatePlaceholder(m) {
      const node = this._doc.createComment("placeholder");
      this._nodes.set(m.id, node);
      this._push(node);
    }

    _opLoadTemplate(m) {
      const node = this._templates.clone(m.tmplId, m.index);
      // The cloned fragment's first child is the root element.
      const root = node.firstChild;
      if (root) {
        this._nodes.set(m.id, root);
        this._push(root);
      }
    }

    _opAssignId(m) {
      let node = null;
      // Walk the path from the last stack top.
      if (this._stack.length > 0) {
        node = this._stack[this._stack.length - 1];
        for (const childIndex of m.path) {
          const children = node.childNodes;
          if (childIndex < children.length) {
            node = children[childIndex];
          } else {
            console.error(
              `[mojo-gui desktop] AssignId: path index ${childIndex} out of bounds`
            );
            return;
          }
        }
      }
      if (node) {
        this._nodes.set(m.id, node);
      }
    }

    _opSetAttribute(m) {
      const node = this._nodes.get(m.id);
      if (!node) {
        console.error(`[mojo-gui desktop] SetAttribute: unknown id ${m.id}`);
        return;
      }
      const el = /** @type {Element} */ (node);
      if (!el.setAttribute) return;

      // Handle special attribute mappings.
      if (m.name === "value" && "value" in el) {
        /** @type {any} */ (el).value = m.value;
        return;
      }
      if (m.name === "checked" && "checked" in el) {
        /** @type {any} */ (el).checked = m.value !== "";
        return;
      }

      const nsUri = m.ns > 0 && m.ns < NS_URIS.length ? NS_URIS[m.ns] : null;
      if (nsUri) {
        el.setAttributeNS(nsUri, m.name, m.value);
      } else {
        el.setAttribute(m.name, m.value);
      }
    }

    _opRemoveAttribute(m) {
      const node = this._nodes.get(m.id);
      if (!node) return;
      const el = /** @type {Element} */ (node);
      if (!el.removeAttribute) return;

      const nsUri = m.ns > 0 && m.ns < NS_URIS.length ? NS_URIS[m.ns] : null;
      if (nsUri) {
        el.removeAttributeNS(nsUri, m.name);
      } else {
        el.removeAttribute(m.name);
      }
    }

    _opSetText(m) {
      const node = this._nodes.get(m.id);
      if (node) {
        node.textContent = m.text;
      }
    }

    _opNewEventListener(m) {
      const node = this._nodes.get(m.id);
      if (!node) return;
      const el = /** @type {Element} */ (node);
      if (!el.addEventListener) return;

      // Set data-eid attribute for event delegation lookup.
      if (el.setAttribute) {
        el.setAttribute("data-eid", String(m.id));
      }

      if (this.onNewListener) {
        this.onNewListener(m.id, m.name, m.handlerId);
      }

      // Also install a direct listener for immediate dispatch.
      const listener = (event) => {
        this._dispatchEvent(m.handlerId, m.name, event);
      };

      // Track listeners for removal.
      let elListeners = this._listeners.get(m.id);
      if (!elListeners) {
        elListeners = new Map();
        this._listeners.set(m.id, elListeners);
      }
      // Remove previous listener for the same event if any.
      const prev = elListeners.get(m.name);
      if (prev) {
        el.removeEventListener(m.name, prev);
      }
      elListeners.set(m.name, listener);
      el.addEventListener(m.name, listener);
    }

    _opRemoveEventListener(m) {
      const node = this._nodes.get(m.id);
      if (!node) return;
      const el = /** @type {Element} */ (node);
      const elListeners = this._listeners.get(m.id);
      if (!elListeners) return;
      const listener = elListeners.get(m.name);
      if (listener) {
        el.removeEventListener(m.name, listener);
        elListeners.delete(m.name);
      }
    }

    _opRemove(m) {
      const node = this._nodes.get(m.id);
      if (node && node.parentNode) {
        node.parentNode.removeChild(node);
      }
      this._nodes.delete(m.id);
      this._listeners.delete(m.id);
    }

    _opReplaceWith(m) {
      const oldNode = this._nodes.get(m.id);
      if (!oldNode) {
        console.error(`[mojo-gui desktop] ReplaceWith: unknown id ${m.id}`);
        return;
      }
      const replacements = this._popMany(m.m);
      const parent = oldNode.parentNode;
      if (parent) {
        for (let i = replacements.length - 1; i >= 0; i--) {
          if (i === 0) {
            parent.replaceChild(replacements[i], oldNode);
          } else {
            // Insert additional nodes after the first replacement.
            const ref = replacements[0].nextSibling;
            if (ref) {
              parent.insertBefore(replacements[i], ref);
            } else {
              parent.appendChild(replacements[i]);
            }
          }
        }
      }
      this._nodes.delete(m.id);
      this._listeners.delete(m.id);
    }

    _opReplacePlaceholder(m) {
      const replacements = this._popMany(m.m);
      // Walk the path from the template root (last on stack after pops).
      const templateRoot = this._stack[this._stack.length - 1];
      if (!templateRoot) {
        console.error(
          "[mojo-gui desktop] ReplacePlaceholder: empty stack after pops"
        );
        return;
      }

      let target = templateRoot;
      for (const childIndex of m.path) {
        const children = target.childNodes;
        if (childIndex < children.length) {
          target = children[childIndex];
        } else {
          console.error(
            `[mojo-gui desktop] ReplacePlaceholder: path ${childIndex} out of bounds`
          );
          return;
        }
      }

      const parent = target.parentNode;
      if (parent && replacements.length > 0) {
        parent.replaceChild(replacements[0], target);
        let insertPoint = replacements[0];
        for (let i = 1; i < replacements.length; i++) {
          const next = insertPoint.nextSibling;
          if (next) {
            parent.insertBefore(replacements[i], next);
          } else {
            parent.appendChild(replacements[i]);
          }
          insertPoint = replacements[i];
        }
      }
    }

    _opInsertAfter(m) {
      const ref = this._nodes.get(m.id);
      if (!ref) {
        console.error(`[mojo-gui desktop] InsertAfter: unknown id ${m.id}`);
        return;
      }
      const newNodes = this._popMany(m.m);
      const parent = ref.parentNode;
      if (!parent) return;
      let insertPoint = ref;
      for (const node of newNodes) {
        const next = insertPoint.nextSibling;
        if (next) {
          parent.insertBefore(node, next);
        } else {
          parent.appendChild(node);
        }
        insertPoint = node;
      }
    }

    _opInsertBefore(m) {
      const ref = this._nodes.get(m.id);
      if (!ref) {
        console.error(`[mojo-gui desktop] InsertBefore: unknown id ${m.id}`);
        return;
      }
      const newNodes = this._popMany(m.m);
      const parent = ref.parentNode;
      if (!parent) return;
      for (const node of newNodes) {
        parent.insertBefore(node, ref);
      }
    }

    /**
     * Dispatch a DOM event to the native Mojo side via mojo_post.
     */
    _dispatchEvent(handlerId, eventName, domEvent) {
      const eventType =
        EVENT_NAME_TO_TYPE[eventName] !== undefined
          ? EVENT_NAME_TO_TYPE[eventName]
          : EventType.Custom;

      let msg;

      // For input/change events, include the string value.
      if (
        (eventName === "input" || eventName === "change") &&
        domEvent.target &&
        "value" in domEvent.target
      ) {
        const value = String(domEvent.target.value);
        // Escape the value for JSON.
        const escaped = value
          .replace(/\\/g, "\\\\")
          .replace(/"/g, '\\"')
          .replace(/\n/g, "\\n")
          .replace(/\r/g, "\\r")
          .replace(/\t/g, "\\t");
        msg = '{"h":' + handlerId + ',"t":' + eventType + ',"v":"' + escaped + '"}';
      } else if (eventName === "keydown" || eventName === "keyup") {
        // For key events, include the key string.
        const key = domEvent.key || "";
        const escaped = key
          .replace(/\\/g, "\\\\")
          .replace(/"/g, '\\"');
        msg = '{"h":' + handlerId + ',"t":' + eventType + ',"v":"' + escaped + '"}';
      } else {
        msg = '{"h":' + handlerId + ',"t":' + eventType + "}";
      }

      // Send to native side.
      if (typeof window.mojo_post === "function") {
        window.mojo_post(msg);
      } else {
        console.warn(
          "[mojo-gui desktop] window.mojo_post not available, event dropped:",
          msg
        );
      }
    }
  }

  // ── Global state ──────────────────────────────────────────────────────────

  let _interpreter = null;
  let _templates = null;
  let _initialized = false;

  /**
   * Initialize the desktop runtime.
   * Called automatically when the page loads, or can be called manually.
   *
   * @param {string} [rootSelector="#root"] - CSS selector for the mount element.
   */
  function init(rootSelector) {
    if (_initialized) return;

    const root = document.querySelector(rootSelector || "#root");
    if (!root) {
      console.error(
        `[mojo-gui desktop] Mount element "${rootSelector || "#root"}" not found`
      );
      return;
    }

    _templates = new TemplateCache(document);
    _interpreter = new Interpreter(root, _templates, document);
    _initialized = true;

    console.log("[mojo-gui desktop] Runtime initialized, mount:", root.tagName);
  }

  /**
   * Apply a base64-encoded mutation buffer.
   * Called from the native side via webview_eval:
   *   window.__mojo_apply_mutations("base64string")
   */
  function applyMutations(base64) {
    if (!_initialized) {
      init();
    }
    if (!_interpreter) {
      console.error(
        "[mojo-gui desktop] Cannot apply mutations: interpreter not initialized"
      );
      return;
    }

    try {
      const buffer = base64ToArrayBuffer(base64);
      _interpreter.applyMutations(buffer, 0, buffer.byteLength);
    } catch (err) {
      console.error("[mojo-gui desktop] Error applying mutations:", err);
    }
  }

  // ── Expose to global scope ────────────────────────────────────────────────

  window.__mojo_apply_mutations = applyMutations;
  window.__mojo_init = init;

  // Also expose for debugging / testing.
  window.__mojo_desktop = {
    init,
    applyMutations,
    get interpreter() {
      return _interpreter;
    },
    get templates() {
      return _templates;
    },
    get initialized() {
      return _initialized;
    },
    Op,
    EventType,
    TAG_NAMES,
    MutationReader,
    TemplateCache,
    Interpreter,
  };

  // ── Auto-init on DOMContentLoaded ─────────────────────────────────────────

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => init());
  } else {
    // DOM already loaded (script injected after load).
    init();
  }
})();
