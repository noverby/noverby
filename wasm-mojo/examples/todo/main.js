// Todo App — Browser Entry Point
//
// Uses shared runtime from examples/lib/ for WASM env, protocol, and interpreter.
//
// Flow:
//   1. Load WASM via shared loadWasm()
//   2. Initialize todo app in WASM (runtime, signals, handlers, templates)
//   3. Build matching template DOM on JS side
//   4. Create interpreter with onNewListener wired BEFORE first mount
//   5. Apply initial mount mutations (events get wired up in the same pass)
//   6. User interactions → WASM exports → flush → apply mutations → DOM updated
//
// Event flow:
//   - "Add" button click → read input value → todo_add_item(text) → todo_flush
//   - "✓" button click → todo_toggle_item(id) → todo_flush
//   - "✕" button click → todo_remove_item(id) → todo_flush
//   - Enter key in input → same as Add button

import { loadWasm, createInterpreter, allocBuffer, applyMutations, writeStringStruct } from "../lib/boot.js";

const BUF_CAPACITY = 65536;

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

    // ── Handler wiring ──────────────────────────────────────────────
    const itemActions = new Map(); // elementId → { action: 'toggle'|'remove', itemId }
    let inputEl = null;

    function flush() {
      const len = fns.todo_flush(appPtr, bufPtr, BUF_CAPACITY);
      if (len > 0) {
        applyMutations(interp, bufPtr, len);
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
      const ul = rootEl.querySelector("ul");
      if (!ul) return;
      const lis = ul.querySelectorAll(":scope > li");
      const itemCount = fns.todo_item_count(appPtr);

      for (let i = 0; i < lis.length && i < itemCount; i++) {
        const li = lis[i];
        const itemId = fns.todo_item_id_at(appPtr, i);
        li.dataset.itemId = itemId;

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
      for (const [eid, node] of interp.nodes) {
        if (node === domNode) return eid;
      }
      return null;
    }

    // 4. Wire event listeners
    interp.onNewListener = (elementId, eventName) => {
      return (evt) => {
        const el = interp.nodes.get(elementId);
        if (!el) return;

        // Check if this is the Add button (direct child of app div)
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

        // Fallback: figure out from DOM structure
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

    // 5. Initial mount
    const mountLen = fns.todo_rebuild(appPtr, bufPtr, BUF_CAPACITY);
    if (mountLen > 0) applyMutations(interp, bufPtr, mountLen);
    scanItemIds();

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
