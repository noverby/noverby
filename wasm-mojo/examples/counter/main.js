// Counter App — Browser Entry Point
//
// Boots the Mojo WASM counter app in a browser environment.
//
// Flow:
//   1. Provide WASM import environment (memory, stubs)
//   2. Load and instantiate the WASM binary
//   3. Initialize counter app in WASM (runtime, signals, handlers, template)
//   4. Build matching template DOM on JS side
//   5. Create interpreter with onNewListener wired BEFORE first mount
//   6. Apply initial mount mutations (events get wired up in the same pass)
//   7. Clicks → WASM dispatch → flush → apply mutations → DOM updated

// ── Constants ───────────────────────────────────────────────────────────────

const BUF_CAPACITY = 16384;
const EVT_CLICK = 0;

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
      default: throw new Error(`Unknown opcode 0x${op.toString(16)}`);
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
        break;
      }

      case Op.ReplaceWith: {
        const old = this.nodes.get(m.id);
        const reps = this.stack.splice(-m.m, m.m);
        const parent = old.parentNode;
        if (parent) {
          parent.replaceChild(reps[0], old);
          for (let i = 1; i < reps.length; i++) {
            parent.insertBefore(reps[i], reps[i - 1].nextSibling);
          }
        }
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
        const parent = ref.parentNode;
        let point = ref.nextSibling;
        for (const n of news) { parent.insertBefore(n, point); point = n.nextSibling; }
        break;
      }

      case Op.InsertBefore: {
        const ref = this.nodes.get(m.id);
        const news = this.stack.splice(-m.m, m.m);
        const parent = ref.parentNode;
        for (const n of news) parent.insertBefore(n, ref);
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

    // 2. Initialize counter app in WASM
    const appPtr = fns.counter_init();
    const tmplId = fns.counter_tmpl_id(appPtr);
    const incrHandler = fns.counter_incr_handler(appPtr);
    const decrHandler = fns.counter_decr_handler(appPtr);

    // 3. Build matching template DOM
    //    Template: div > [ span > "", button > "+", button > "−" ]
    const templateRoots = new Map();
    {
      const div = document.createElement("div");

      const span = document.createElement("span");
      span.appendChild(document.createTextNode(""));
      div.appendChild(span);

      const btnPlus = document.createElement("button");
      btnPlus.appendChild(document.createTextNode("+"));
      div.appendChild(btnPlus);

      const btnMinus = document.createElement("button");
      btnMinus.appendChild(document.createTextNode("\u2212"));
      div.appendChild(btnMinus);

      templateRoots.set(tmplId, [div.cloneNode(true)]);
    }

    // 4. Clear loading indicator
    rootEl.innerHTML = "";

    // 5. Create interpreter and wire up event listener callback FIRST
    const interp = new Interpreter(rootEl, templateRoots);
    const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

    // Handler mapping: NewEventListener mutations arrive in template order:
    //   dynamic_attr[0] → incr button click
    //   dynamic_attr[1] → decr button click
    const handlerOrder = [incrHandler, decrHandler];
    const handlerMap = new Map(); // "elementId:eventName" → handlerId
    let listenerIdx = 0;

    interp.onNewListener = (elementId, eventName) => {
      const hid = handlerOrder[listenerIdx++] ?? incrHandler;
      const key = `${elementId}:${eventName}`;
      handlerMap.set(key, hid);

      // Return the actual DOM event listener
      return () => {
        const handlerId = handlerMap.get(key);
        if (handlerId === undefined) return;

        // Dispatch event to WASM → signal update → scope dirty
        fns.counter_handle_event(appPtr, handlerId, EVT_CLICK);

        // Flush: re-render dirty scopes, diff, get mutations
        const len = fns.counter_flush(appPtr, bufPtr, BUF_CAPACITY);
        if (len > 0) {
          interp.applyMutations(wasmMemory.buffer, Number(bufPtr), len);
        }
      };
    };

    // 6. Initial mount — rebuild and apply (onNewListener is already set)
    const mountLen = fns.counter_rebuild(appPtr, bufPtr, BUF_CAPACITY);
    if (mountLen > 0) {
      interp.applyMutations(wasmMemory.buffer, Number(bufPtr), mountLen);
    }

    console.log("🔥 Mojo Counter app running!");
  } catch (err) {
    console.error("Failed to boot:", err);
    rootEl.innerHTML = `<p class="error">Failed to load: ${err.message}</p>`;
  }
}

boot();
