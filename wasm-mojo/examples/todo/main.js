// Todo App — Browser Entry Point
//
// Uses shared runtime from examples/lib/ for WASM env, protocol, and interpreter.
// Uses EventBridge for automatic event wiring via handler IDs in the mutation protocol.
//
// Flow:
//   1. Load WASM via shared loadWasm()
//   2. Initialize todo app in WASM (runtime, signals, handlers, templates)
//   3. Build matching template DOM on JS side
//   4. Create interpreter + EventBridge BEFORE first mount
//   5. Apply initial mount mutations (events get wired up in the same pass)
//   6. User interactions → EventBridge → WASM dispatch → flush → apply mutations → DOM updated
//
// Event flow:
//   - "Add" button click → read input value → todo_add_item(text) → todo_flush
//   - "✓" button click → handler ID dispatched directly via EventBridge
//   - "✕" button click → handler ID dispatched directly via EventBridge
//   - Enter key in input → same as Add button

import { loadWasm, createInterpreter, allocBuffer, applyMutations, EventBridge, writeStringStruct } from "../lib/boot.js";

const BUF_CAPACITY = 65536;
const EVT_CLICK = 0;

async function boot() {
  const rootEl = document.getElementById("root");

  try {
    const fns = await loadWasm(new URL("../../build/out.wasm", import.meta.url));

    // 1. Initialize todo app in WASM
    const appPtr = fns.todo_init();
    const appTmplId = fns.todo_app_template_id(appPtr);
    const itemTmplId = fns.todo_item_template_id(appPtr);
    const addHandlerId = fns.todo_add_handler(appPtr);

    // 2. Build matching template DOM structures
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

    // 3. Clear loading indicator and create interpreter
    rootEl.innerHTML = "";
    const interp = createInterpreter(rootEl, templateRoots);
    const bufPtr = allocBuffer(BUF_CAPACITY);

    // Helper: read input value and add a todo item
    let inputEl = null;
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

    function flush() {
      const len = fns.todo_flush(appPtr, bufPtr, BUF_CAPACITY);
      if (len > 0) applyMutations(interp, bufPtr, len);
    }

    // 4. Wire events via EventBridge — handler IDs come from the mutation protocol
    new EventBridge(interp, (handlerId, eventName, domEvent) => {
      // The "Add" button handler needs special treatment: read the input value first
      if (handlerId === addHandlerId) {
        addItem();
        return;
      }

      // All other handlers (toggle, remove) dispatch directly
      fns.todo_handle_event(appPtr, handlerId, EVT_CLICK);
      flush();
    });

    // 5. Initial mount
    const mountLen = fns.todo_rebuild(appPtr, bufPtr, BUF_CAPACITY);
    if (mountLen > 0) applyMutations(interp, bufPtr, mountLen);

    // 6. Wire up input field for Enter key
    inputEl = rootEl.querySelector("input");
    if (inputEl) {
      inputEl.addEventListener("keydown", (e) => {
        if (e.key === "Enter") addItem();
      });
    }

    console.log("🔥 Mojo Todo app running!");
  } catch (err) {
    console.error("Failed to boot:", err);
    rootEl.innerHTML = `<p class="error">Failed to load: ${err.message}</p>`;
  }
}

boot();
