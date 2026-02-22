# Phase 25 — Freeing Allocator ✅

Replace the bump allocator (which never reclaims memory) with a tracking
allocator across all three runtimes, enabling safe memory reuse.

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

## Key insight (revised after P25.5)

Mojo's borrow checker ensures frees are called for Mojo-level ownership, but
the compiled WASM output contains **double-free patterns** — the same pointer
freed more than once due to Mojo's destructor mechanics. With the old no-op
free this was invisible. With a tracking allocator, double-frees caused
duplicate entries in the free list: two allocations could pop the same pointer,
corrupting each other's data.

This was discovered during P25.1 by enabling reuse and observing diff tests
produce 0 mutations (old and new vnodes sharing the same backing memory).

**The fix**: `alignedFree` removes the pointer from `ptrSize` on first free,
so subsequent frees of the same pointer are detected as "unknown" and silently
ignored. When a freed block is reused, it is re-registered in `ptrSize` so
future frees work correctly. Reuse is now enabled by default in all runtimes.

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
  ├─ if not found → ignore (double-free or unknown pointer)
  ├─ delete ptr from ptrSize (prevents double-free stacking)
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
- `reuse_enabled: Bool` toggle (enabled by default).
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

### P25.5 — Fix double-free bug and enable reuse ✅

Diagnosed and fixed the root cause of memory corruption when reuse was enabled.

**Root cause**: The allocator did not remove pointers from `ptrSize` on free,
so double-frees (same pointer freed twice by WASM) stacked duplicate entries
in the free list. Two subsequent allocations could then pop the same pointer.

**Fix** (applied to all three runtimes):

- `alignedFree` deletes the pointer from `ptrSize` on first free. Subsequent
  frees of the same pointer find no entry and are silently ignored.
- `alignedAlloc` re-registers reused pointers in `ptrSize` so future frees
  work correctly.
- `mutation_buf_alloc` (Mojo) now zero-initializes the buffer with
  `memset_zero` so that unwritten positions read as `OP_END` (0x00), which is
  necessary because reused blocks may contain stale data.

**Tests** — `test-js/allocator.test.ts` (28 new WASM-integrated reuse assertions):

- ✅ Reuse ON: diff with changed dynamic text → SetText.
- ✅ Reuse ON: same dynamic text → 0 mutations.
- ✅ Reuse ON: dynamic attribute changed → SetAttribute.
- ✅ Reuse ON: sequential diffs (state chain, 5 steps).
- ✅ Reuse ON: text VNode diff (both created before `create_vnode` — the
  original failing pattern that exposed the double-free bug).
- ✅ Reuse ON: text VNode diff (new created after `create_vnode`).
- ✅ Reuse ON: fragment children text changed.
- ✅ Heap stats track frees during WASM operations.
- ✅ Reuse ON: placeholder diff → 0 mutations.
- ✅ Reuse ON: TemplateRef diff (both created before `create_vnode`).

**Validation**:

- `just test-js` — 1,385 tests pass (1,357 existing + 28 new reuse tests).
- `just test` — 29 modules, 946 tests pass with reuse enabled by default.
- Full suite with `setAllocatorReuse(true)` globally — 1,385 tests, 0 failures.

**Result**: Reuse enabled by default in all runtimes. Documentation updated.

## Estimated size (revised)

| Step  | Scope                            | ~Lines | Status |
|-------|----------------------------------|--------|--------|
| P25.1 | TS allocator + tests             | ~250   | ✅ Done |
| P25.2 | JS allocator port                | ~50    | ✅ Done |
| P25.3 | Mojo allocator port              | ~80    | ✅ Done |
| P25.4 | Scratch arena + string frees     | ~100   | ✅ Done |
| P25.5 | Fix double-free + enable reuse   | ~600   | ✅ Done |
| **Total** |                              | **~1080** | |