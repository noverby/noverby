// Counter App — Browser Entry Point
//
// Uses shared runtime from examples/lib/ for WASM env, protocol, and interpreter.
// Uses EventBridge for automatic event wiring via handler IDs in the mutation protocol.
// Templates are automatically registered from WASM via RegisterTemplate mutations.
//
// Flow:
//   1. Load WASM via shared loadWasm()
//   2. Initialize counter app in WASM (runtime, signals, handlers, template)
//   3. Create interpreter with empty template map (templates come from WASM)
//   4. Wire EventBridge for automatic event dispatch
//   5. Apply initial mount mutations (templates + events wired in one pass)
//   6. Clicks → EventBridge → WASM dispatch → flush → apply mutations → DOM updated

import { loadWasm, createInterpreter, allocBuffer, applyMutations, EventBridge } from "../lib/boot.js";

const BUF_CAPACITY = 16384;
const EVT_CLICK = 0;

async function boot() {
  const rootEl = document.getElementById("root");

  try {
    const fns = await loadWasm(new URL("../../build/out.wasm", import.meta.url));

    // 1. Initialize counter app in WASM
    const appPtr = fns.counter_init();

    // 2. Clear loading indicator and create interpreter (empty — templates come from WASM)
    rootEl.innerHTML = "";
    const interp = createInterpreter(rootEl, new Map());
    const bufPtr = allocBuffer(BUF_CAPACITY);

    // 3. Wire events via EventBridge — handler IDs come from the mutation protocol
    new EventBridge(interp, (handlerId) => {
      fns.counter_handle_event(appPtr, handlerId, EVT_CLICK);
      const len = fns.counter_flush(appPtr, bufPtr, BUF_CAPACITY);
      if (len > 0) applyMutations(interp, bufPtr, len);
    });

    // 4. Initial mount (RegisterTemplate + LoadTemplate + events in one pass)
    const mountLen = fns.counter_rebuild(appPtr, bufPtr, BUF_CAPACITY);
    if (mountLen > 0) applyMutations(interp, bufPtr, mountLen);

    console.log("🔥 Mojo Counter app running!");
  } catch (err) {
    console.error("Failed to boot:", err);
    rootEl.innerHTML = `<p class="error">Failed to load: ${err.message}</p>`;
  }
}

boot();
