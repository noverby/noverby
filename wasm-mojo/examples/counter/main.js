// Counter App — Browser Entry Point
//
// Uses shared runtime from examples/lib/ for WASM env, protocol, and interpreter.
//
// Flow:
//   1. Load WASM via shared loadWasm()
//   2. Initialize counter app in WASM (runtime, signals, handlers, template)
//   3. Build matching template DOM on JS side
//   4. Create interpreter with onNewListener wired BEFORE first mount
//   5. Apply initial mount mutations (events get wired up in the same pass)
//   6. Clicks → WASM dispatch → flush → apply mutations → DOM updated

import { loadWasm, createInterpreter, allocBuffer, applyMutations } from "../lib/boot.js";

const BUF_CAPACITY = 16384;
const EVT_CLICK = 0;

async function boot() {
  const rootEl = document.getElementById("root");

  try {
    const fns = await loadWasm(new URL("../../build/out.wasm", import.meta.url));

    // 1. Initialize counter app in WASM
    const appPtr = fns.counter_init();
    const tmplId = fns.counter_tmpl_id(appPtr);
    const incrHandler = fns.counter_incr_handler(appPtr);
    const decrHandler = fns.counter_decr_handler(appPtr);

    // 2. Build matching template DOM: div > [ span > "", button > "+", button > "−" ]
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

    // 3. Clear loading indicator and create interpreter
    rootEl.innerHTML = "";
    const interp = createInterpreter(rootEl, templateRoots);
    const bufPtr = allocBuffer(BUF_CAPACITY);

    // 4. Wire event listeners — handler order matches dynamic_attr order
    const handlerOrder = [incrHandler, decrHandler];
    const handlerMap = new Map();
    let listenerIdx = 0;

    interp.onNewListener = (elementId, eventName) => {
      const hid = handlerOrder[listenerIdx++] ?? incrHandler;
      const key = `${elementId}:${eventName}`;
      handlerMap.set(key, hid);

      return () => {
        const handlerId = handlerMap.get(key);
        if (handlerId === undefined) return;
        fns.counter_handle_event(appPtr, handlerId, EVT_CLICK);
        const len = fns.counter_flush(appPtr, bufPtr, BUF_CAPACITY);
        if (len > 0) applyMutations(interp, bufPtr, len);
      };
    };

    // 5. Initial mount
    const mountLen = fns.counter_rebuild(appPtr, bufPtr, BUF_CAPACITY);
    if (mountLen > 0) applyMutations(interp, bufPtr, mountLen);

    console.log("🔥 Mojo Counter app running!");
  } catch (err) {
    console.error("Failed to boot:", err);
    rootEl.innerHTML = `<p class="error">Failed to load: ${err.message}</p>`;
  }
}

boot();
