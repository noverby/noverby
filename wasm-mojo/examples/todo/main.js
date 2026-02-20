// Todo App — Browser Entry Point
//
// Boots the Mojo WASM todo app in a browser environment.
//
// Flow:
//   1. Provide WASM import environment (memory, stubs)
//   2. Load and instantiate the WASM binary
//   3. Initialize todo app in WASM (runtime, signals, handlers, templates)
//   4. Build matching template DOM on JS side
//   5. Create interpreter with onNewListener wired BEFORE first mount
//   6. Apply initial mount mutations (events get wired up in the same pass)
//   7. User interactions → WASM exports → flush → apply mutations → DOM updated
//
// Event flow:
//   - "Add" button click → read input value → todo_add_item(text) → todo_flush
//   - "✓" button click → todo_toggle_item(id) → todo_flush
//   - "✕" button click → todo_remove_item(id) → todo_flush
//   - Enter key in input → same as Add button

// ── Constants ───────────────────────────────────────────────────────────────

const BUF_CAPACITY = 65536;

// ── WASM runtime state ──────────────────────────────────────────────────────

let wasmMemory = null;
let heapPointer = 0n;

function alignedAlloc(align, size) {
  const remainder = heapPointer % align;
  if (remainder !== 0n) heapPointer += align - remainder;
  const ptr = heapPointer;
  heapPointer += size;
  return ptr;
}

// ── Mojo String ABI ─────────────────────────────────────────────────────────
//
// Mojo String is a 24-byte struct: { data_ptr: i64, len: i64, capacity: i64 }
// We must allocate this struct in WASM linear memory and pass a pointer to it.

const textEncoder = new TextEncoder();

function writeStringStruct(str) {
  const bytes = textEncoder.encode(str);
  const dataLen = BigInt(bytes.length);

  // Allocate buffer for string data (with null terminator)
  const dataPtr = alignedAlloc(1n, dataLen + 1n);
  new Uint8Array(wasmMemory.buffer).set(bytes, Number(dataPtr));
  new Uint8Array(wasmMemory.buffer)[Number(dataPtr + dataLen)] = 0;

  // Allocate 24-byte String struct
  const structPtr = alignedAlloc(8n, 24n);
  const view = new DataView(wasmMemory.buffer);
  view.setBigInt64(Number(structPtr), dataPtr, true);       // data_ptr
  view.setBigInt64(Number(structPtr) + 8, dataLen, true);   // len
  view.setBigInt64(Number(structPtr) + 16, dataLen + 1n, true); // capacity

  return structPtr;
}

// ── Mutation protocol decoder ───────────────────────────────────────────────

const Op = {
  End: 0x00, AppendChildren: 0x01, AssignId: 0x02, CreatePlaceholder: 0x03,
  CreateTextNode: 0x04, LoadTemplate: 0x05, ReplaceWith: 0x06,
  ReplacePlaceholder: 0x07, InsertAfter: 0x08, InsertBefore: 0x09,
  SetAttribute: 0x0a, SetText: 0x0b, NewEventListener: 0x0c,
  RemoveEventListener: 0x0d, Remove: 0x0e, PushRoot: 0x0f,
};

class MutationReader {
  constructor(buffer, byteOffset, byteLength) {
    this.view = new DataView(buffer, byteOffset, byteLength);
    this.bytes = new Uint8Array(buffer, byteOffset, byteLength);
    this.offset = 0;
    this.end = byteLength;
  }
  readU8()  { const v = this.view.getUint8(this.offset);              this.offset += 1; return v; }
  readU16() { const v = this.view.getUint16(this.offset, true);       this.offset += 2; return v; }
  readU32() { const v = this.view.getUint32(this.offset, true);       this.offset += 4; return v; }
  readStr() {
    const len = this.readU32();
    if (len === 0) return "";
    const s = new TextDecoder().decode(this.bytes.subarray(this.offset, this.offset + len));
    this.offset += len;
    return s;
  }
  readShortStr() {
    const len = this.readU16();
    if (len === 0) return "";
    const s = new TextDecoder().decode(this.bytes.subarray(this.offset, this.offset + len));
    this.offset += len;
    return s;
  }
  readPath() {
    const len = this.readU8();
    const p = this.bytes.slice(this.offset, this.offset + len);
    this.offset += len;
    return p;
  }
  next() {
    if (this.offset >= this.end) return null;
    const op = this.readU8();
    switch (op) {
      case Op.End:               return null;
      case Op.AppendChildren:    return { op, id: this.readU32(), m: this.readU32() };
      case Op.AssignId:          return { op, path: this.readPath(), id: this.readU32() };
      case Op.CreatePlaceholder: return { op, id: this.readU32() };
      case Op.CreateTextNode:    return { op, id: this.readU32(), text: this.readStr() };
      case Op.LoadTemplate:      return { op, tmplId: this.readU32(), index: this.readU32(), id: this.readU32() };
      case Op.ReplaceWith:       return { op, id: this.readU32(), m: this.readU32() };
      case Op.ReplacePlaceholder:return { op, path: this.readPath(), m: this.readU32() };
      case Op.InsertAfter:       return { op, id: this.readU32(), m: this.readU32() };
      case Op.InsertBefore:      return { op, id: this.readU32(), m: this.readU32() };
      case Op.SetAttribute:      { const id = this.readU32(), ns = this.readU8(), name = this.readShortStr(), value = this.readStr(); return { op, id, ns, name, value }; }
      case Op.SetText:           return { op, id: this.readU32(), text: this.readStr() };
      case Op.NewEventListener:  return { op, id: this.readU32(), name: this.readShortStr() };
      case Op.RemoveEventListener: return { op, id: this.readU32(), name: this.readShortStr() };
      case Op.Remove:            return { op, id: this.readU32() };
      case Op.PushRoot:          return { op, id: this.readU32() };
      default: throw new Error(`Unknown opcode 0x${op.toString(16)} at offset ${this.offset - 1}`);
    }
  }
}

// ── Minimal DOM Interpreter ─────────────────────────────────────────────────

class Interpreter {
  constructor(root, templateRoots) {
    this.stack = [];
    this.nodes = new Map();
    this.templateRoots = templateRoots;
    this.doc = root.ownerDocument;
    this.root = root;
    this.listeners = new Map();
    this.onNewListener = null;
    this.nodes.set(0, root);
  }

  applyMutations(buffer, byteOffset, byteLength) {
    const reader = new MutationReader(buffer, byteOffset, byteLength);
    for (let m = reader.next(); m !== null; m = reader.next()) {
      this.handle(m);
    }
  }

  handle(m) {
    switch (m.op) {
      case Op.PushRoot:
        this.stack.push(this.nodes.get(m.id));
        break;

      case Op.AppendChildren: {
        const parent = this.nodes.get(m.id);
        const children = this.stack.splice(-m.m, m.m);
        for (const c of children) parent.appendChild(c);
        break;
      }

      case Op.CreateTextNode: {
        const n = this.doc.createTextNode(m.text);
        this.nodes.set(m.id, n);
        this.stack.push(n);
        break;
      }

      case Op.CreatePlaceholder: {
        const n = this.doc.createComment("placeholder");
        this.nodes.set(m.id, n);
        this.stack.push(n);
        break;
      }

      case Op.LoadTemplate: {
        const roots = this.templateRoots.get(m.tmplId);
        if (!roots) throw new Error(`Template ${m.tmplId} not registered`);
        const n = roots[m.index].cloneNode(true);
        this.nodes.set(m.id, n);
        this.stack.push(n);
        break;
      }

      case Op.AssignId: {
        let n = this.stack[this.stack.length - 1];
        for (const idx of m.path) n = n.childNodes[idx];
        this.nodes.set(m.id, n);
        break;
      }

      case Op.SetAttribute: {
        const n = this.nodes.get(m.id);
        if (n && n.setAttribute) n.setAttribute(m.name, m.value);
        break;
      }

      case Op.SetText: {
        const n = this.nodes.get(m.id);
        if (n) n.textContent = m.text;
        break;
      }

      case Op.NewEventListener: {
        const el = this.nodes.get(m.id);
        if (!el) break;
        const listener = this.onNewListener
          ? this.onNewListener(m.id, m.name)
          : () => {};
        let elMap = this.listeners.get(m.id);
        if (!elMap) { elMap = new Map(); this.listeners.set(m.id, elMap); }
        const prev = elMap.get(m.name);
        if (prev) el.removeEventListener(m.name, prev);
        el.addEventListener(m.name, listener);
        elMap.set(m.name, listener);
        break;
      }

      case Op.RemoveEventListener: {
        const el = this.nodes.get(m.id);
        if (!el) break;
        const elMap = this.listeners.get(m.id);
        if (!elMap) break;
        const fn = elMap.get(m.name);
        if (fn) { el.removeEventListener(m.name, fn); elMap.delete(m.name); }
        break;
      }

      case Op.Remove: {
        const n = this.nodes.get(m.id);
        if (n && n.parentNode) n.parentNode.removeChild(n);
        this.nodes.delete(m.id);
        break;
      }

      case Op.ReplaceWith: {
        const old = this.nodes.get(m.id);
        const reps = this.stack.splice(-m.m, m.m);
        if (old && old.parentNode) {
          const parent = old.parentNode;
          parent.replaceChild(reps[0], old);
          for (let i = 1; i < reps.length; i++) {
            parent.insertBefore(reps[i], reps[i - 1].nextSibling);
          }
        }
        this.nodes.delete(m.id);
        break;
      }

      case Op.ReplacePlaceholder: {
        const reps = this.stack.splice(-m.m, m.m);
        let target = this.stack[this.stack.length - 1];
        for (const idx of m.path) target = target.childNodes[idx];
        const parent = target.parentNode;
        if (parent) {
          for (const r of reps) parent.insertBefore(r, target);
          parent.removeChild(target);
        }
        break;
      }

      case Op.InsertAfter: {
        const ref = this.nodes.get(m.id);
        const news = this.stack.splice(-m.m, m.m);
        if (ref && ref.parentNode) {
          const parent = ref.parentNode;
          let point = ref.nextSibling;
          for (const n of news) { parent.insertBefore(n, point); point = n.nextSibling; }
        }
        break;
      }

      case Op.InsertBefore: {
        const ref = this.nodes.get(m.id);
        const news = this.stack.splice(-m.m, m.m);
        if (ref && ref.parentNode) {
          const parent = ref.parentNode;
          for (const n of news) parent.insertBefore(n, ref);
        }
        break;
      }
    }
  }
}

// ── WASM environment (import object) ────────────────────────────────────────

const env = {
  memory: new WebAssembly.Memory({ initial: 256 }),
  __cxa_atexit: () => 0,
  KGEN_CompilerRT_AlignedAlloc: alignedAlloc,
  KGEN_CompilerRT_AlignedFree: () => 1,
  KGEN_CompilerRT_GetStackTrace: () => 0n,
  KGEN_CompilerRT_fprintf: () => 0,
  write: (fd, ptr, len) => {
    if (len === 0n || !wasmMemory) return 0;
    const text = new TextDecoder().decode(new Uint8Array(wasmMemory.buffer, Number(ptr), Number(len)));
    console.log(text);
    return Number(len);
  },
  free: () => 1, dup: () => 1, fdopen: () => 1, fflush: () => 1, fclose: () => 1,
  __multi3: (resultPtr, aLo, aHi, bLo, bHi) => {
    if (!wasmMemory) return;
    const mask = 0xffffffffffffffffn;
    const product = (((aHi & mask) << 64n) | (aLo & mask)) * (((bHi & mask) << 64n) | (bLo & mask));
    const view = new DataView(wasmMemory.buffer);
    view.setBigInt64(Number(resultPtr), product & mask, true);
    view.setBigInt64(Number(resultPtr) + 8, (product >> 64n) & mask, true);
  },
  fmaf: (x, y, z) => Math.fround(Math.fround(x * y) + z),
  fminf: (x, y) => (x > y ? y : x),
  fmaxf: (x, y) => (x > y ? x : y),
  fma: (x, y, z) => x * y + z,
  fmin: (x, y) => (x > y ? y : x),
  fmax: (x, y) => (x > y ? x : y),
};

// ── Boot ────────────────────────────────────────────────────────────────────

async function boot() {
  const rootEl = document.getElementById("root");

  try {
    // 1. Load WASM
    const wasmUrl = new URL("../../build/out.wasm", import.meta.url);
    const wasmBuffer = await fetch(wasmUrl).then((r) => r.arrayBuffer());
    const { instance } = await WebAssembly.instantiate(wasmBuffer, { env });

    const fns = instance.exports;
    wasmMemory = fns.memory;
    heapPointer = fns.__heap_base.value;

    // 2. Initialize todo app in WASM
    const appPtr = fns.todo_init();
    const appTmplId = fns.todo_app_template_id(appPtr);
    const itemTmplId = fns.todo_item_template_id(appPtr);
    const addHandlerId = fns.todo_add_handler(appPtr);

    // 3. Build matching template DOM structures

    const templateRoots = new Map();

    // "todo-app" template: div > [ input, button("Add"), ul > placeholder ]
    {
      const div = document.createElement("div");

      const input = document.createElement("input");
      input.setAttribute("type", "text");
      input.setAttribute("placeholder", "What needs to be done?");
      div.appendChild(input);

      const btnAdd = document.createElement("button");
      btnAdd.appendChild(document.createTextNode("Add"));
      div.appendChild(btnAdd);

      const ul = document.createElement("ul");
      // The placeholder for dynamic[0] — a comment node that CreateEngine
      // will replace via ReplacePlaceholder
      ul.appendChild(document.createComment("placeholder"));
      div.appendChild(ul);

      templateRoots.set(appTmplId, [div.cloneNode(true)]);
    }

    // "todo-item" template: li > [ span > "", button("✓"), button("✕") ]
    {
      const li = document.createElement("li");

      const span = document.createElement("span");
      span.appendChild(document.createTextNode(""));
      li.appendChild(span);

      const btnToggle = document.createElement("button");
      btnToggle.appendChild(document.createTextNode("✓"));
      li.appendChild(btnToggle);

      const btnRemove = document.createElement("button");
      btnRemove.appendChild(document.createTextNode("✕"));
      li.appendChild(btnRemove);

      templateRoots.set(itemTmplId, [li.cloneNode(true)]);
    }

    // 4. Clear loading indicator
    rootEl.innerHTML = "";

    // 5. Create interpreter and wire up event listener callbacks
    const interp = new Interpreter(rootEl, templateRoots);
    const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

    // ── Handler wiring ──────────────────────────────────────────────
    //
    // When NewEventListener mutations arrive, we need to know:
    //   - Which element is getting the listener
    //   - What action to perform
    //
    // Strategy:
    //   - The Add button's handler ID is known (addHandlerId).
    //   - For item toggle/remove buttons, we maintain a mapping from
    //     elementId to { action, itemId } built during each render cycle.
    //
    // During each render/flush cycle, the WASM side creates new item VNodes
    // with fresh handler IDs. On the JS side, the NewEventListener mutation
    // tells us the element ID and event name. We use the DOM structure to
    // figure out context:
    //   - If the element is a <button> inside a <li>, it's an item button.
    //   - First <button> in <li> = toggle, second = remove.
    //   - The item ID comes from the <li>'s data-item-id attribute (or we
    //     track it via the VNode key).
    //
    // Simpler approach: Track handler→action via the order that
    // NewEventListener mutations arrive during each render cycle.
    // For each item, the WASM side emits:
    //   1. NewEventListener for toggle (dynamic_attr[0])
    //   2. NewEventListener for remove (dynamic_attr[1])
    //   3. SetAttribute for class (dynamic_attr[2])
    //
    // We maintain a map: elementId → { type: 'toggle'|'remove', itemId }
    // by inspecting the DOM element's parent <li> and position.

    // Map from element IDs to item actions
    const itemActions = new Map(); // elementId → { action: 'toggle'|'remove', itemId }

    // We'll use the input element reference to get text
    let inputEl = null;

    function flush() {
      const len = fns.todo_flush(appPtr, bufPtr, BUF_CAPACITY);
      if (len > 0) {
        interp.applyMutations(wasmMemory.buffer, Number(bufPtr), len);
        // After mutations, re-scan for item IDs
        scanItemIds();
      }
    }

    function addItem() {
      if (!inputEl) inputEl = rootEl.querySelector("input");
      if (!inputEl) return;
      const text = inputEl.value.trim();
      if (!text) return;
      const strPtr = writeStringStruct(text);
      fns.todo_add_item(appPtr, strPtr);
      inputEl.value = "";
      flush();
    }

    function scanItemIds() {
      // After a render cycle, scan the DOM to associate <li> elements
      // with item IDs from the WASM side.
      // Items in WASM are ordered; <li> elements in the <ul> match that order.
      const ul = rootEl.querySelector("ul");
      if (!ul) return;
      const lis = ul.querySelectorAll(":scope > li");
      const itemCount = fns.todo_item_count(appPtr);

      for (let i = 0; i < lis.length && i < itemCount; i++) {
        const li = lis[i];
        const itemId = fns.todo_item_id_at(appPtr, i);
        li.dataset.itemId = itemId;

        // Map the toggle and remove buttons
        const buttons = li.querySelectorAll(":scope > button");
        if (buttons[0]) {
          const toggleEid = findElementId(interp, buttons[0]);
          if (toggleEid !== null) {
            itemActions.set(toggleEid, { action: "toggle", itemId });
          }
        }
        if (buttons[1]) {
          const removeEid = findElementId(interp, buttons[1]);
          if (removeEid !== null) {
            itemActions.set(removeEid, { action: "remove", itemId });
          }
        }
      }
    }

    function findElementId(interp, domNode) {
      // Reverse lookup: DOM node → element ID
      for (const [eid, node] of interp.nodes) {
        if (node === domNode) return eid;
      }
      return null;
    }

    interp.onNewListener = (elementId, eventName) => {
      return (evt) => {
        const el = interp.nodes.get(elementId);
        if (!el) return;

        // Check if this is the Add button
        // The Add button is a direct child <button> of the app div
        if (el.tagName === "BUTTON" && el.parentElement && el.parentElement === rootEl.querySelector(":scope > div")) {
          addItem();
          return;
        }

        // Check if this is an item button (inside a <li>)
        const action = itemActions.get(elementId);
        if (action) {
          if (action.action === "toggle") {
            fns.todo_toggle_item(appPtr, action.itemId);
            flush();
          } else if (action.action === "remove") {
            fns.todo_remove_item(appPtr, action.itemId);
            flush();
          }
          return;
        }

        // Fallback: try to figure out from DOM structure
        if (el.tagName === "BUTTON" && el.closest("li")) {
          const li = el.closest("li");
          const itemId = parseInt(li.dataset.itemId, 10);
          if (!isNaN(itemId)) {
            const buttons = li.querySelectorAll(":scope > button");
            if (el === buttons[0]) {
              fns.todo_toggle_item(appPtr, itemId);
              flush();
            } else if (el === buttons[1]) {
              fns.todo_remove_item(appPtr, itemId);
              flush();
            }
          }
        }
      };
    };

    // 6. Initial mount
    const mountLen = fns.todo_rebuild(appPtr, bufPtr, BUF_CAPACITY);
    if (mountLen > 0) {
      interp.applyMutations(wasmMemory.buffer, Number(bufPtr), mountLen);
    }

    // Scan item IDs after initial mount
    scanItemIds();

    // 7. Wire up the input field for Enter key
    inputEl = rootEl.querySelector("input");
    if (inputEl) {
      inputEl.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
          addItem();
        }
      });
    }

    console.log("🔥 Mojo Todo app running!");
  } catch (err) {
    console.error("Failed to boot:", err);
    rootEl.innerHTML = `<p class="error">Failed to load: ${err.message}</p>`;
  }
}

boot();
