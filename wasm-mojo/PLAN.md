# Phase 25 — Freeing Allocator

Replace the bump allocator (which never reclaims memory) with a tracking
allocator across all three runtimes, with the goal of enabling memory reuse
once WASM-side use-after-free bugs are resolved.

## Problem

The current bump allocator in all three runtimes (`runtime/memory.ts`,
`examples/lib/env.js`, `test/wasm_harness.mojo`) only advances a heap pointer.
`KGEN_CompilerRT_AlignedFree` is a no-op:

```ts
export const alignedFree = (_ptr: bigint): number => {
    return 1; // no-op
};
```

Mojo's ownership model already emits `KGEN_CompilerRT_AlignedFree` calls at the
right time — when `String`s, `List`s, and `UnsafePointer`s go out of scope, their
destructors fire and call free. Those calls just hit a no-op today, so the memory
is never reclaimed.

### Consequences

- **Test OOM**: After ~1,200 JS tests the bump pointer exhausts linear memory.
  Creating multiple bench app instances in sequence is already impossible.
- **WASM-internal leaks**: Mojo emits proper free calls for destroyed strings,
  resized lists, freed pointers — all silently ignored.
- **JS-side leaks**: `writeStringStruct()` allocates on every keystroke and
  never frees (smaller issue, separable).
- **No app restart**: Cannot destroy an app and create a new one — the old
  app's memory is permanently consumed.

## Key insight (revised after P25.1)

Mojo's borrow checker ensures frees are called for Mojo-level ownership, but
the compiled WASM output contains **use-after-free patterns** that were
previously masked by the no-op free:

- `create_vnode` frees internal vnode storage (e.g. 32-byte list backing
  buffers) that is still referenced by the vnode for future diffs.
- Enabling immediate reuse causes those freed blocks to be handed out for
  new allocations, corrupting the old vnode's data.

This was discovered during P25.1 by enabling reuse and observing diff tests
produce 0 mutations (old and new vnodes sharing the same backing memory).

**The allocator tracks all frees so `heapStats()` reports reclaimable memory.
Actual reuse is gated behind `setAllocatorReuse(true)` and disabled by default
until the Mojo vnode code is fixed.**

## Design (revised)

A **JS-side size-class map** allocator — no headers in WASM linear memory.

### Why not WASM-side headers?

The initial plan called for a 16-byte header before each allocation in WASM
linear memory. This approach was abandoned because:

1. **Pre-init allocs**: Some allocations happen during WASM instantiation
   before the JS runtime has a memory reference, so headers can't be written.
2. **Alignment overhead**: Headers shift all pointers by 16 bytes, wasting
   space and complicating alignment.
3. **Performance**: Reading headers from WASM memory (via `DataView`) on every
   free is slower than a JS-side `Map.get()`.

### Current design

```txt
alignedAlloc(align, size)
  ├─ if reuseEnabled: check freeMap[size] for a cached pointer (O(1) pop)
  └─ else: bump allocator (same as before, O(1))
       └─ record ptr→size in ptrSize Map

alignedFree(ptr)
  ├─ look up size = ptrSize.get(ptr)
  └─ push ptr onto freeMap[size] bucket
```

- `ptrSize: Map<bigint, bigint>` — every bump-allocated pointer → its size.
- `freeMap: Map<bigint, bigint[]>` — size-class buckets of freed pointers.
- `reuseEnabled: boolean` — gates whether `alignedAlloc` pops from `freeMap`.
- `heapStats()` — walks `freeMap` to report free blocks/bytes.
- Zero WASM memory reads/writes in the allocator — fully transparent.

## Steps

### P25.1 — Size-class map allocator in TypeScript (`runtime/memory.ts`) ✅

Replaced the bump allocator with JS-side tracking. Same export signatures.

- `alignedAlloc(align, size)` with size-class reuse (gated) + bump fallback.
- `alignedFree(ptr)` with JS-side size lookup and free-list push.
- `heapStats()` reports free blocks and bytes.
- `setAllocatorReuse(on)` toggle for enabling/disabling reuse.
- `saveAllocator()` / `restoreAllocator()` / `initTestAllocator()` for test
  isolation.

**Tests** — `test-js/allocator.test.ts` (with reuse enabled in isolated memory):

- ✅ Alloc returns correctly aligned pointers.
- ✅ Free + re-alloc reuses memory (same size).
- ✅ Different sizes use different buckets.
- ✅ Mismatched size falls through to bump.
- ✅ Bump fallback when free list is empty.
- ✅ LIFO ordering (stack-like pop).
- ✅ Rapid alloc/free cycles — heap stats stable.
- ✅ Mixed-size rapid cycles stay bounded.
- ✅ Checkerboard (interleaved) alloc/free pattern.
- ✅ HeapStats accuracy.
- ✅ Large allocations (4 KiB).
- ✅ free(0) is a safe no-op.

**Result**: 1,338 tests pass (1,278 existing + 60 new allocator tests), 0 failures, ~1.2s runtime.

**Discovery**: WASM vnode code has use-after-free — reuse disabled by default
until Mojo source is fixed. See "Key insight (revised)" above.

### P25.2 — Size-class map allocator in JavaScript (`examples/lib/env.js`) ✅

Ported P25.1 to plain JS for the browser examples runtime.

- `alignedAlloc(align, size)` with size-class reuse (gated) + bump fallback.
- `alignedFree(ptr)` with JS-side size lookup and free-list push.
- `heapStats()` reports free blocks and bytes.
- `setAllocatorReuse(on)` toggle for enabling/disabling reuse.
- `initMemory()` resets `freeMap` and `ptrSize` on WASM reload.
- `KGEN_CompilerRT_AlignedFree` wired to `alignedFree` (was no-op `() => 1`).

**Verify**: `just test-js` — 1,338 tests pass, 0 failures.

### P25.3 — Size-class map allocator in Mojo (`test/wasm_harness.mojo`) ✅

Ported the size-class map allocator to the Mojo test harness.

- `SharedState.aligned_alloc(align, size)` with size-class reuse (gated) + bump fallback.
- `SharedState.aligned_free(ptr)` with `Dict`-based size lookup and free-list push.
- `SharedState.heap_stats()` reports (heap_pointer, free_blocks, free_bytes).
- `ptr_size: Dict[Int, Int]` and `free_map: Dict[Int, List[Int]]` for tracking.
- `reuse_enabled: Bool` toggle (disabled by default).
- `_cb_aligned_free` wired to `state[].aligned_free(ptr)` (was no-op).

**Verify**: `just test` — 29 modules, 946 tests, 0 failures (~16s).

### P25.4 — JS-side string struct frees ✅

Added a scratch arena for transient `writeStringStruct` allocations across
both the TS runtime and JS browser examples runtime.

- `scratchAlloc(align, size)` wraps `alignedAlloc` and records the pointer.
- `scratchFreeAll()` bulk-frees all recorded scratch pointers.
- `writeStringStruct()` now uses `scratchAlloc` (both `runtime/strings.ts`
  and `examples/lib/strings.js`).
- TS runtime: `scratchFreeAll()` called in `EventBridge.handleEvent()` after
  string dispatch (WASM consumes data synchronously before free).
- JS examples: `scratchFreeAll()` called in `launch()` flush helper after
  mutations are applied.
- `scratchPtrs` reset on `initialize()` / `initMemory()` / `initTestAllocator()`.
- Exported from `runtime/mod.ts` for test and consumer access.

**Tests** — `test-js/allocator.test.ts` (19 new scratch arena assertions):

- ✅ scratchAlloc returns aligned, distinct pointers.
- ✅ scratchFreeAll moves scratch blocks to free list.
- ✅ Re-alloc reuses freed scratch blocks (LIFO).
- ✅ scratchFreeAll on empty arena is a safe no-op (including double-call).
- ✅ 1,000 writeStringStruct-like cycles — heap pointer stays bounded.
- ✅ scratchFreeAll does not affect direct (non-scratch) allocations.

**Result**: 1,357 tests pass (1,338 existing + 19 new scratch tests), 0 failures.

### P25.5 — Fix WASM use-after-free and enable reuse

Investigate and fix the use-after-free in the Mojo vnode/diff code:

- `create_vnode` frees internal list buffers still needed by the vnode.
- Once fixed, enable `setAllocatorReuse(true)` by default.
- Multi-app: create → destroy → create → destroy × 10, heap bounded.
- Bench: create 10k rows, clear × 10, heap bounded.
- Update `AGENTS.md` and `README.md`, remove OOM caveats.

## Estimated size (revised)

| Step  | Scope                            | ~Lines | Status |
|-------|----------------------------------|--------|--------|
| P25.1 | TS allocator + tests             | ~250   | ✅ Done |
| P25.2 | JS allocator port                | ~50    | ✅ Done |
| P25.3 | Mojo allocator port              | ~80    | ✅ Done |
| P25.4 | Scratch arena + string frees     | ~100   | ✅ Done |
| P25.5 | Fix use-after-free + validation  | ~200   | |
| **Total** |                              | **~680** | |