// Shared WASM environment — memory management + import object.
//
// Used by all browser examples to avoid duplicating ~70 lines each.

// ── WASM memory state ───────────────────────────────────────────────────────

let wasmMemory = null;
let heapPointer = 0n;

/** Get the current WASM memory (after initMemory has been called). */
export function getMemory() {
  return wasmMemory;
}

/** Bump-allocate `size` bytes with the given alignment from the WASM heap. */
export function alignedAlloc(align, size) {
  const remainder = heapPointer % align;
  if (remainder !== 0n) heapPointer += align - remainder;
  const ptr = heapPointer;
  heapPointer += size;
  return ptr;
}

/** Initialize memory state from WASM exports (called after instantiation). */
export function initMemory(exports) {
  wasmMemory = exports.memory;
  heapPointer = exports.__heap_base.value;
}

// ── WASM import object ──────────────────────────────────────────────────────

export const env = {
  memory: new WebAssembly.Memory({ initial: 4096 }),
  __cxa_atexit: () => 0,
  KGEN_CompilerRT_AlignedAlloc: alignedAlloc,
  KGEN_CompilerRT_AlignedFree: () => 1,
  KGEN_CompilerRT_GetStackTrace: () => 0n,
  KGEN_CompilerRT_fprintf: () => 0,
  write: (fd, ptr, len) => {
    if (len === 0n || !wasmMemory) return 0;
    const text = new TextDecoder().decode(
      new Uint8Array(wasmMemory.buffer, Number(ptr), Number(len)),
    );
    console.log(text);
    return Number(len);
  },
  free: () => 1,
  dup: () => 1,
  fdopen: () => 1,
  fflush: () => 1,
  fclose: () => 1,
  __multi3: (resultPtr, aLo, aHi, bLo, bHi) => {
    if (!wasmMemory) return;
    const mask = 0xffffffffffffffffn;
    const product =
      (((aHi & mask) << 64n) | (aLo & mask)) *
      (((bHi & mask) << 64n) | (bLo & mask));
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

// ── WASM loader ─────────────────────────────────────────────────────────────

/**
 * Load and instantiate a Mojo WASM binary.
 *
 * @param {string|URL} wasmUrl - URL to the .wasm file.
 * @returns {Promise<object>} The WASM instance exports.
 */
export async function loadWasm(wasmUrl) {
  const wasmBuffer = await fetch(wasmUrl).then((r) => r.arrayBuffer());
  const { instance } = await WebAssembly.instantiate(wasmBuffer, { env });
  const fns = instance.exports;
  initMemory(fns);
  return fns;
}
