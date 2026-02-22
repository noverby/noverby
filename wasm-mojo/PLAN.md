# Phase 25 — Freeing Allocator

Replace the bump allocator (which never reclaims memory) with a free-list
allocator across all three runtimes.

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

## Key insight

Mojo's borrow checker already ensures frees are called at the right time.
**The only task is making `alignedFree` actually free.** Once the allocator
honors free calls, WASM-internal memory automatically stays bounded.

## Design

A **free-list allocator** with block headers, operating on WASM linear memory.

### Block layout

```txt
┌──────────────────────┬──────────────────────────────┐
│  Header (16 bytes)   │  User data (size bytes)      │
│  ┌────────┬────────┐ │                              │
│  │ size   │ _pad   │ │  ← alignedAlloc returns here │
│  │ (i64)  │ (i64)  │ │                              │
│  └────────┴────────┘ │                              │
└──────────────────────┴──────────────────────────────┘
```

- `alignedAlloc(align, size)` returns a pointer to user data (header + 16).
- `alignedFree(ptr)` reads `size` from `ptr - 16` to recover the block.
- Header is 16 bytes so user data is naturally 16-byte aligned (covers all
  alignments Mojo requests: 1, 4, 8, 16).

### Free list

- Singly-linked list of free blocks, sorted by address.
- Free blocks reuse the user-data region for the link: `{ next_ptr, size }`.
- Minimum allocation: 16 bytes (to hold the link when freed).
- On free: insert into sorted position, coalesce with adjacent blocks.
- On alloc: first-fit search. Split oversized blocks. Fall back to bump
  pointer if no free block fits.

## Steps

### P25.1 — Free-list allocator in TypeScript (`runtime/memory.ts`)

Replace the bump allocator. Same export signatures — drop-in replacement.

- `alignedAlloc(align, size)` with header + free-list + bump fallback.
- `alignedFree(ptr)` with free-list insertion + coalescing.

**Tests** — `test-js/allocator.test.ts`:

- Alloc returns aligned pointers.
- Free + re-alloc reuses memory.
- Coalescing (adjacent frees merge into one block).
- Split (alloc from an oversized free block).
- Bump fallback when free list is empty.
- Rapid alloc/free cycles — heap stats stable.

### P25.2 — Free-list allocator in JavaScript (`examples/lib/env.js`)

Port P25.1 to plain JS for the browser examples runtime.

**Verify**: `just serve` — counter, todo, bench all work.

### P25.3 — Free-list allocator in Mojo (`test/wasm_harness.mojo`)

Port to the Mojo test harness (`SharedState.aligned_alloc` + `_cb_aligned_free`).

**Verify**: `just test` — all Mojo tests pass. Memory usage stays bounded.

### P25.4 — JS-side string struct frees

`writeStringStruct()` allocates a 24-byte struct + data buffer per call
(every keystroke). Add a scratch arena that bulk-frees after each flush:

- `scratchAlloc(align, size)` records allocations.
- `scratchFreeAll()` frees them all after flush.
- Wire into `launch()` flush helper and EventBridge dispatch.

**Tests**: 1,000 writeStringStruct → flush → scratchFreeAll cycles, heap stable.

### P25.5 — Validation

- Multi-app: create → destroy → create → destroy × 10, heap bounded.
- Bench: create 10k rows, clear × 10, heap bounded.
- Update `AGENTS.md` and `README.md`, remove OOM caveats.

## Estimated size

| Step  | Scope                        | ~Lines |
|-------|------------------------------|--------|
| P25.1 | TS allocator + tests         | ~300   |
| P25.2 | JS allocator port            | ~80    |
| P25.3 | Mojo allocator port          | ~80    |
| P25.4 | Scratch arena + string frees | ~100   |
| P25.5 | Validation + docs            | ~150   |
| **Total** |                          | **~710** |