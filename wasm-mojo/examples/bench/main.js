// Benchmark App — Browser Entry Point
//
// Uses shared runtime from examples/lib/ for WASM env, protocol, and interpreter.
// Templates are automatically registered from WASM via RegisterTemplate mutations.
//
// Boots the Mojo WASM benchmark app (js-framework-benchmark style) in a
// browser environment. Provides Create/Append/Update/Swap/Clear/Select/Remove
// operations with timing display.

import { loadWasm, createInterpreter, allocBuffer, applyMutations, EventBridge } from "../lib/boot.js";

const BUF_CAPACITY = 8 * 1024 * 1024; // 8 MB mutation buffer

const statusEl = document.getElementById("status");
const tbody = document.getElementById("tbody");

function setStatus(text) {
  statusEl.innerHTML = text;
}

function timeOp(name, fn) {
  const start = performance.now();
  fn();
  const ms = (performance.now() - start).toFixed(1);
  setStatus(`<strong>${name}</strong>: <span class="timing">${ms}ms</span> — ${fns ? fns.bench_row_count(appPtr) : "?"} rows`);
}

let fns = null;
let appPtr = null;
let bufPtr = null;
let interp = null;

function flush() {
  const len = fns.bench_flush(appPtr, bufPtr, BUF_CAPACITY);
  if (len > 0) {
    applyMutations(interp, bufPtr, len);
  }
}

async function boot() {
  try {
    fns = await loadWasm(new URL("../../build/out.wasm", import.meta.url));

    // 1. Initialize benchmark app
    appPtr = fns.bench_init();

    // 2. Create interpreter (empty — templates come from WASM via RegisterTemplate mutations)
    interp = createInterpreter(tbody, new Map());
    bufPtr = allocBuffer(BUF_CAPACITY);

    // 3. Wire up event listener tracking via EventBridge (no-op dispatch —
    // bench uses event delegation on tbody, not per-element listeners)
    new EventBridge(interp, () => {});

    // 4. Initial mount (RegisterTemplate + LoadTemplate in one pass)
    const mountLen = fns.bench_rebuild(appPtr, bufPtr, BUF_CAPACITY);
    if (mountLen > 0) {
      applyMutations(interp, bufPtr, mountLen);
    }

    // 5. Event delegation on tbody
    tbody.addEventListener("click", (e) => {
      const a = e.target.closest("a");
      if (!a) return;
      const tr = a.closest("tr");
      if (!tr) return;

      const idText = tr.querySelector("td")?.textContent;
      if (!idText) return;
      const rowId = parseInt(idText, 10);
      if (isNaN(rowId)) return;

      if (a.classList.contains("remove")) {
        timeOp("Remove row", () => {
          fns.bench_remove(appPtr, rowId);
          flush();
        });
      } else {
        timeOp("Select row", () => {
          fns.bench_select(appPtr, rowId);
          flush();
        });
      }
    });

    // 6. Wire buttons
    document.getElementById("btn-create1k").onclick = () => {
      timeOp("Create 1,000 rows", () => {
        fns.bench_create(appPtr, 1000);
        flush();
      });
    };

    document.getElementById("btn-create10k").onclick = () => {
      timeOp("Create 10,000 rows", () => {
        fns.bench_create(appPtr, 10000);
        flush();
      });
    };

    document.getElementById("btn-append").onclick = () => {
      timeOp("Append 1,000 rows", () => {
        fns.bench_append(appPtr, 1000);
        flush();
      });
    };

    document.getElementById("btn-update").onclick = () => {
      timeOp("Update every 10th", () => {
        fns.bench_update(appPtr);
        flush();
      });
    };

    document.getElementById("btn-swap").onclick = () => {
      timeOp("Swap rows", () => {
        fns.bench_swap(appPtr);
        flush();
      });
    };

    document.getElementById("btn-clear").onclick = () => {
      timeOp("Clear", () => {
        fns.bench_clear(appPtr);
        flush();
      });
    };

    setStatus("Ready — click a button to start benchmarking");
    console.log("🔥 Mojo Benchmark app running!");

  } catch (err) {
    console.error("Failed to boot:", err);
    setStatus(`<span style="color:#ee5a6f">Failed to load: ${err.message}</span>`);
  }
}

boot();
