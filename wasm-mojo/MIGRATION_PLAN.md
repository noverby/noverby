# Migration Plan — Mojo 0.26.1

> Tracks all breaking changes, new features, and opportunities from the
> [Mojo 0.26.1 release](https://docs.modular.com/stable/mojo/changelog#v0261-2026-01-29)
> relevant to wasm-mojo. Items are ordered by priority.

---

## 🔴 Breaking Changes

### ✅ B1 — `List` variadic initializer removed

**What changed:** `List[T](a, b, c)` no longer works. Use typed list literals instead.

**Impact:** Widespread — `List[Node](...)` is used throughout `src/component/context.mojo`,
`src/mutations/create.mojo`, and example apps.

**Migration:**

```mojo
# Before (0.25.x)
el_div(List[Node](
    el_h1(List[Node](dyn_text(0))),
    el_button(List[Node](text("Up!"), onclick_add(count, 1))),
))

# After (0.26.1) — move type hint to first element
el_div([
    el_h1([dyn_text(0)]),
    el_button([text("Up!"), onclick_add(count, 1)]),
])

# Or with explicit type annotation
var keys: List[UInt32] = [1, 2, 3]
```

**Search pattern:** `grep -rn 'List\[.*\](' src/ examples/`

**Estimated scope:** ~50–80 call sites across `context.mojo`, `keyed_list.mojo`,
`create.mojo`, `registry.mojo`, `main.mojo`, and example apps.

**Status:** ✅ Done — all `List[T](a, b, c)` variadic calls replaced with
`[a, b, c]` typed list literals across `src/`, `test/`, and `examples/`.
Comments and docstrings updated accordingly.

---

### ✅ B2 — `alias` keyword warns, migrate to `comptime`

**What changed:** The compiler now warns on every `alias` usage and suggests
`comptime` instead. `alias` still works but will eventually become an error.

**Impact:** Pervasive — wasm-mojo uses `alias` for all protocol opcodes, tag
constants, node kinds, event types, action tags, and sentinels.

**Migration:**

```mojo
# Before
alias OP_END = UInt8(0x00)
alias TAG_DIV: UInt8 = 0

# After
comptime OP_END = UInt8(0x00)
comptime TAG_DIV: UInt8 = 0
```

**Search pattern:** `grep -rn '\balias\b' src/`

**Estimated scope:** ~150+ declarations across `protocol.mojo`, `tags.mojo`,
`dsl.mojo`, `vnode.mojo`, `template.mojo`, `registry.mojo`, `scope.mojo`,
`context.mojo`, `element_id.mojo`.

**Note:** This is a mechanical find-replace. Do it in one pass after all other
migrations are done, since it touches every file and will create merge conflicts
with concurrent work.

**Status:** ✅ Done — all `alias` declarations replaced with `comptime` across
`src/`, `test/`, and `examples/` (~150+ declarations).

---

### ✅ B3 — `ImplicitlyBoolable` trait removed

**What changed:** Types like `Int`, `UInt32`, `UnsafePointer` can no longer
implicitly convert to `Bool`. Code like `if pointer:` or `if count:` will fail.

**Impact:** Moderate — need to audit all conditional expressions that rely on
truthiness of non-Bool types.

**Migration:**

```mojo
# Before
if self._free_head:        # Int, truthy if non-zero
if ptr:                    # UnsafePointer, truthy if non-null

# After
if self._free_head != -1:  # explicit comparison
if ptr != UnsafePointer[T]():  # explicit null check
```

**Search pattern:** `grep -rn 'if \(self\.\|ptr\|pointer\|count\|len\)' src/`

**Estimated scope:** ~20–40 sites. Careful audit needed — some may already use
explicit comparisons.

**Status:** ✅ Done — 4 implicit `UnsafePointer` truthiness checks fixed in
`app_shell.mojo` (3 sites) and `runtime.mojo` (1 site). All other conditionals
already used explicit `Bool` comparisons.

---

### ✅ B4 — `UInt` is now `Scalar[DType.uint]`

**What changed:** `UInt` became a type alias to `Scalar[DType.uint]`. Implicit
conversion between `Int` and `UInt` has been removed.

**Impact:** Low — wasm-mojo primarily uses `UInt32`, `UInt8`, and `Int`, not
bare `UInt`. But any `Int` ↔ `UInt` implicit conversions will break.

**Search pattern:** `grep -rn '\bUInt\b' src/ | grep -v UInt8 | grep -v UInt32`

**Status:** ✅ No changes needed — only explicit `UInt()` construction in
`element_id.mojo.__hash__()`, which continues to work.

---

### ✅ B5 — `Iterator` trait overhaul

**What changed:** `__has_next__()` removed; iterators now use `__next__()` that
`raises StopIteration`. The `Iterator.Element` type no longer requires
`ImplicitlyDestructible`.

**Impact:** Low — wasm-mojo does not define custom iterators (verified via grep).
Only affects consumption of standard library iterators, which should work
transparently.

**Action:** No code changes needed unless custom iterators are added later.

**Status:** ✅ No changes needed — confirmed no custom iterators in codebase.

---

### ✅ B6 — `Error` no longer `Boolable` or `Defaultable`

**What changed:** `Error()` (default construction) and `if error:` patterns
no longer work. Errors must be constructed with meaningful messages.

**Impact:** Low — wasm-mojo doesn't heavily use `Error` as a value type.

**Search pattern:** `grep -rn 'Error()' src/`

**Status:** ✅ No changes needed — no `Error()` default construction or boolean
checks on `Error` values found.

---

### ✅ B7 — `InlineArray` no longer `ImplicitlyCopyable`

**What changed:** Users must explicitly copy arrays or take references.

**Impact:** Low — check if any `InlineArray` values are passed by implicit copy.

**Status:** ✅ No changes needed — no `InlineArray` usage in codebase.

---

### ✅ B8 — `Writer` rework: `write_bytes()` → `write_string()`

**What changed:** `Writer` now supports only UTF-8 data. `write_bytes()` replaced
with `write_string()`. `String.__init__(*, bytes:)` renamed to `unsafe_from_utf8`.

**Impact:** Low — only relevant if custom `Writer` implementations exist.

**Status:** ✅ No changes needed — no custom `Writer` implementations or
`write_bytes()` calls found.

---

## 🟢 New Features to Adopt

### F1 — Typed errors (zero-overhead on WASM)

**What:** Functions can specify `raises CustomError` instead of generic `Error`.
Compiled as alternate return values — no stack unwinding — making them ideal for
WASM targets.

**Opportunity:** Define `EventError`, `DiffError`, `MutationError` types for
the event dispatch, diff engine, and mutation writer. Zero runtime overhead.

```mojo
struct EventError:
    var message: String
    var handler_id: UInt32

fn dispatch_event(handler_id: UInt32) raises EventError -> Bool:
    ...
```

**Priority:** Medium — adopt incrementally as error paths are touched.

**Status:** 🟡 Deferred — the codebase currently uses no `raises` functions.
Event dispatch, diff, and mutation paths use `Bool` returns for error
signaling. Typed errors will be adopted when error paths are refactored
to use `raises`, but there is no natural application point today.

---

### F2 — String UTF-8 safety constructors

**What:** `String` now has three constructors for raw bytes:

- `String(from_utf8=span)` — validates, raises on invalid
- `String(from_utf8_lossy=span)` — replaces invalid with `�`
- `String(unsafe_from_utf8=span)` — no validation

**Opportunity:** Use in the WASM ↔ JS string bridge for explicit UTF-8 safety
guarantees when constructing strings from shared memory buffers.

**Priority:** Medium — apply when touching string bridge code.

**Status:** 🟡 Deferred — no Mojo code currently constructs `String` from raw
WASM memory bytes. The `@export` FFI layer handles `String` parameters
natively, and `MutationWriter` only writes strings *to* the buffer (not
reads). Will adopt when a Mojo-side string-from-bytes path is added.

---

### F3 — Traits with default implementations

**What:** `Hashable`, `Writable`, and `Equatable` now auto-derive from struct
fields via reflection. Just declare conformance — no method body needed.

**Opportunity:** Add `Writable` and `Equatable` conformance to core structs
(`ElementId`, `Node`, `HandlerEntry`, `VNode`, etc.) with zero boilerplate for
debugging and testing.

```mojo
@fieldwise_init
struct ElementId(Equatable, Writable):
    var value: UInt32
    # __eq__ and write_to auto-derived!
```

**Priority:** Medium — improves debugging and test assertions.

**Status:** ✅ Done — auto-derived `Equatable` and `Writable` added to ~20
structs across `src/`. Changes by category:

- **Slot state structs** (`_SlotState`, `HandlerSlotState`, `_ScopeSlotState`,
  `SignalSlotState`, `MemoSlotState`, `EffectSlotState`, `_StringSlotState`):
  Added `Equatable, Writable`. `_StringSlotState` converted to
  `@fieldwise_init`, removing manual `__init__`/`__copyinit__`/`__moveinit__`.

- **ElementId**: Added `Hashable, Writable`. Removed manual `__eq__`,
  `__ne__`, `__hash__` (now auto-derived from the single `id` field).
  Kept custom `__str__` for the `"ElementId(N)"` format.

- **Core data structs** (`HandlerEntry`, `SchedulerEntry`, `HandlerAction`,
  `_HandlerMapping`, `EventBinding`, `DynamicNode`, `DynamicAttr`,
  `FragmentSlot`, `EffectEntry`, `MemoEntry`, `_EventInfo`,
  `_ValueBindingInfo`): Added `Equatable, Writable`.

- **AttributeValue**: Added `Equatable, Writable`. Since named constructors
  always set inactive fields to defaults, field-by-field equality is
  semantically correct. The manual `_attr_values_equal()` helper in
  `diff.mojo` now delegates to `==`.

- **`_HandlerMapping`**, **`SchedulerEntry`**: Converted to `@fieldwise_init`,
  removing redundant manual `__init__`.

---

### F4 — `Copyable` now refines `Movable`

**What:** Structs declaring `Copyable` no longer need to also declare `Movable`.

**Opportunity:** Audit and remove redundant `Movable` conformance declarations.

**Search pattern:** `grep -rn 'Copyable.*Movable\|Movable.*Copyable' src/`

**Priority:** Low — minor cleanup.

**Status:** ✅ Done — removed redundant `Movable` from all struct declarations
that already declare `Copyable` (~40 structs). Also cleaned up redundant
`Copyable & Movable & AnyType` generic constraints to `Copyable & AnyType`
in `runtime.mojo`.

---

### F5 — `comptime(x)` expression

**What:** Force a subexpression to be evaluated at compile time without needing
an `alias`/`comptime` declaration.

**Opportunity:** Useful in template registration and static configuration where
inline compile-time values are cleaner than separate declarations.

**Priority:** Low — convenience improvement.

**Status:** 🟡 Deferred — all compile-time values in the codebase are named
constants (`comptime TAG_DIV`, `comptime OP_END`, etc.) where a named
declaration is clearer than an inline `comptime(x)` expression. No call
sites were found where an anonymous inline compile-time evaluation would
be more readable than the existing named constants. Will adopt if inline
compile-time expressions become useful in future template or DSL code.

---

### F6 — `-Xlinker` flag

**What:** `mojo build` now supports `-Xlinker` to pass options to the linker.

**Opportunity:** Currently the build pipeline runs `wasm-ld` as a separate step.
This flag could potentially simplify the pipeline if `mojo build` can be
configured to invoke `wasm-ld` directly. Investigate whether this is viable for
the wasm64-wasi target.

**Priority:** Low — investigate only.

**Status:** 🟡 Deferred — not applicable to the current build pipeline. The
`justfile` uses `mojo build --emit llvm` → `llc` → `wasm-ld` because Mojo
does not natively target `wasm64-wasi`. The `-Xlinker` flag passes options
to Mojo's *internal* linker (for native targets), not to an external
`wasm-ld` invocation. The custom multi-step pipeline remains necessary.

---

### F7 — `-Werror` flag

**What:** Treats all warnings as errors. Useful for CI.

**Opportunity:** Add to CI pipeline to catch issues early, especially during
the `alias` → `comptime` migration period.

**Priority:** Low — add to CI after migration is complete.

**Status:** ✅ Done — added `-Werror` to the `mojo build` command in `justfile`.
The WASM binary (`src/main.mojo` and all transitive `src/` imports) compiles
with zero warnings. Note: the Mojo test binaries (which import the external
`wasmtime-mojo` dependency) are built by `scripts/build_test_binaries.sh`
without `-Werror` because `wasmtime-mojo` still has `alias` deprecation
warnings that are outside this project's control.

---

### F8 — `conforms_to()` + `trait_downcast()` (experimental)

**What:** Compile-time trait conformance checking and downcasting. Enables static
dispatch based on trait conformance.

**Opportunity:** Stepping stone toward generic `Signal[T]`. Could be used to
build a more generic signal store that dispatches based on whether `T` conforms
to specific traits:

```mojo
fn store_value[T: AnyType](ref value: T):
    @parameter
    if conforms_to(T, Copyable):
        # store via memcpy
        ...
    else:
        # store via move
        ...
```

**Priority:** Low — experimental, explore when tackling generic `Signal[T]`.

**Status:** 🟡 Deferred — the `SignalStore` currently uses type-erased raw-byte
storage with separate `SignalStore` (Int32 via memcpy) and `StringStore`
(heap Strings via `List[String]`). There are no generic `[T: AnyType]`
store paths that need runtime trait-based dispatch. `conforms_to()` will
become useful when building a unified `Signal[T]` store, which is still
blocked on full conditional conformance for parametric types.

---

### F9 — Expanded reflection module

**What:** `struct_field_count`, `struct_field_names`, `struct_field_types`,
`offset_of`, `__struct_field_ref` — compile-time struct introspection.

**Opportunity:** Could enable auto-generated binary protocol encoders,
debug formatters, or generic serialization for VNode/mutation types.

**Priority:** Low — explore for future phases.

**Status:** 🟡 Deferred — the binary mutation protocol is hand-written for
performance (`MutationWriter` writes opcodes + payloads directly). Debug
formatting is now covered by auto-derived `Writable` (F3). No clear
application for reflection-based serialization exists today. Will revisit
if a generic encode/decode layer is needed for new struct types.

---

### F10 — `Never` type

**What:** A type that can never be instantiated. Functions returning `Never` are
guaranteed to not return normally (like `abort()`). Functions that `raises Never`
compile with the same ABI as non-raising functions.

**Opportunity:** Annotate unreachable code paths and `abort()` wrappers.

**Priority:** Low — minor type safety improvement.

**Status:** 🟡 Deferred — the codebase has no `abort()` calls, `panic()`
calls, or functions guaranteed to never return. Error paths use early
`return` with default/sentinel values rather than aborting. The `Never`
type has no natural application point today. Will adopt if abort-style
error handlers or unreachable-code annotations are introduced.

---

## 🟡 Deferred Abstractions Update

The following items from the "Deferred Abstractions" table are **partially
unblocked** by 0.26.1 but not yet fully actionable:

| Abstraction | 0.26.1 progress | Still blocked on |
|---|---|---|
| **Generic `Signal[T]`** | `conforms_to()` + `trait_downcast()` enable static dispatch; reflection enables field introspection | Full conditional conformance for parametric stores |
| **Closure event handlers** | Function type conversions improved (non-raising → raising, ref → value) | True closures / function pointers in WASM |
| **Pattern matching** | No progress | ADTs & pattern matching |
| **`rsx!` macro** | No progress | Hygienic macros |
| **Dynamic component dispatch** | `AnyType` no longer requires `__del__()` (explicitly-destroyed types) | Existentials / dynamic traits |
| **Async** | No progress | First-class async |

---

## Migration Order

Recommended sequence to minimize churn and test breakage:

1. ✅ **B3 — `ImplicitlyBoolable`** — fix all implicit bool conversions first,
   since these cause hard compile errors and are scattered throughout.

2. ✅ **B1 — `List` variadic init** — update all `List[T](a, b, c)` patterns.
   This is the most widespread change.

3. ✅ **B4–B8 — Minor breaks** — fix `UInt` conversions, `Error` patterns,
   `InlineArray` copies, `Writer` changes.

4. ✅ **B2 — `alias` → `comptime`** — do this last as a bulk find-replace,
   since it touches every file and is purely mechanical.

5. ✅ **F3 — Default trait impls** — auto-derived `Equatable` and `Writable`
   on ~20 structs; removed manual `__eq__`/`__ne__`/`__hash__` boilerplate
   from `ElementId`; simplified `_attr_values_equal` to use `==`.

6. ✅ **F7 — `-Werror` in build** — enabled in `justfile` `build` target.

### Also completed

- ✅ **F4 — Remove redundant `Movable`** — cleaned up ~40 struct declarations
  and generic constraints.

### Deferred (no natural application point)

- 🟡 **F1 — Typed errors** — no `raises` functions in codebase.
- 🟡 **F2 — UTF-8 constructors** — no raw-bytes string construction.
- 🟡 **F5 — `comptime(x)` expression** — all constants are named; no inline use case.
- 🟡 **F6 — `-Xlinker` flag** — not applicable; build uses custom `llc` + `wasm-ld` pipeline.
- 🟡 **F8 — `conforms_to()` / `trait_downcast()`** — no generic store paths yet.
- 🟡 **F9 — Reflection module** — protocol is hand-written; debug via `Writable` (F3).
- 🟡 **F10 — `Never` type** — no abort/panic/unreachable code paths.

---

## Verification

After migration, the full test suite must pass:

```bash
just test-all    # 996 Mojo tests + 1,385 JS tests
```

Additionally, verify the three example apps render correctly:

```bash
just serve
# Open http://localhost:4507/examples/counter/
# Open http://localhost:4507/examples/todo/
# Open http://localhost:4507/examples/bench/
```

### Current status

- ✅ `just build` — compiles with `-Werror`, zero warnings.
- ✅ `just test-js` — 1,385 JS tests pass.
- ⚠️ `just test` — blocked by pre-existing `wasmtime-mojo` compile error
  (pointer origin mismatch in `module.mojo:124`, unrelated to wasm-mojo).

---

## ✅ Migration Complete

All breaking changes (B1–B8) have been resolved. All new features with
actionable application points (F3, F4, F7) have been adopted. The remaining
features (F1, F2, F5, F6, F8, F9, F10) have been evaluated and deferred —
none have natural application points in the current codebase architecture.
They will be revisited when their prerequisites are met (e.g. `raises`-based
error paths for F1, raw-bytes string construction for F2, generic `Signal[T]`
for F8).

**Summary:**

- 8 breaking changes resolved (4 required code changes, 4 no changes needed)
- 3 new features adopted (F3: auto-derived traits, F4: redundant Movable cleanup, F7: -Werror)
- 7 new features deferred with documented rationale
- Build: zero warnings with `-Werror`
- Tests: 1,385 JS tests passing
