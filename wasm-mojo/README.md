# wasm-mojo

Proof-of-concept for compiling [Mojo](https://www.modular.com/mojo) to WebAssembly and running it in the browser.

## How it works

The build pipeline compiles Mojo source code to WASM through LLVM:

```txt
Mojo → LLVM IR → WASM Object → WASM Binary
```

1. `mojo build` emits LLVM IR as a shared library
2. `llc` compiles the IR to a wasm64-wasi object file
3. `wasm-ld` links the object into a `.wasm` binary

The TypeScript runtime (`runtime/`) provides the necessary environment stubs for the WASM module, including:

- **Memory management** — a bump allocator for `KGEN_CompilerRT_AlignedAlloc`/`AlignedFree`
- **I/O** — `write` routed to `console.log`/`console.error` for stdout/stderr
- **Math builtins** — `fma`, `fmin`, `fmax` and their float variants
- **Libc stubs** — `dup`, `fdopen`, `fflush`, `fclose`, `__cxa_atexit`
- **String ABI** — helpers for reading/writing Mojo `String` structs (including SSO)

## Project structure

```txt
wasm-mojo/
├── src/
│   └── main.mojo              # Mojo source with all @export functions
├── runtime/
│   ├── mod.ts                 # Entry point — instantiate WASM + re-exports
│   ├── types.ts               # WasmExports interface (typed WASM bindings)
│   ├── memory.ts              # Bump allocator and WASM memory state
│   ├── env.ts                 # Environment imports (I/O, math, libc stubs)
│   └── strings.ts             # Mojo String ABI helpers (read/write/alloc)
├── test/
│   ├── run.ts                 # Test entry point — loads WASM, runs all suites
│   ├── harness.ts             # Test harness (suite, assert, assertClose, summary)
│   ├── arithmetic.test.ts     # add, sub, mul, div, mod, pow
│   ├── unary.test.ts          # neg, abs
│   ├── minmax.test.ts         # min, max, clamp
│   ├── bitwise.test.ts        # bitand, bitor, bitxor, bitnot, shl, shr
│   ├── comparison.test.ts     # eq, ne, lt, le, gt, ge, boolean logic
│   ├── algorithms.test.ts     # fib, factorial, gcd
│   ├── identity.test.ts       # identity / passthrough
│   ├── print.test.ts          # print functions
│   ├── strings.test.ts        # string I/O and operations
│   └── consistency.test.ts    # cross-function consistency checks
├── build/                     # Build output (generated)
├── justfile                   # Build and test commands
└── default.nix                # Nix dev shell definition
```

## Exported Mojo functions

The Mojo source (`src/main.mojo`) exports functions demonstrating interop across several categories:

| Category | Functions |
|---|---|
| Add | `add_int32`, `add_int64`, `add_float32`, `add_float64` |
| Subtract | `sub_int32`, `sub_int64`, `sub_float32`, `sub_float64` |
| Multiply | `mul_int32`, `mul_int64`, `mul_float32`, `mul_float64` |
| Division | `div_int32`, `div_int64`, `div_float32`, `div_float64` |
| Modulo | `mod_int32`, `mod_int64` |
| Power | `pow_int32`, `pow_int64`, `pow_float32`, `pow_float64` |
| Negate | `neg_int32`, `neg_int64`, `neg_float32`, `neg_float64` |
| Absolute value | `abs_int32`, `abs_int64`, `abs_float32`, `abs_float64` |
| Min / Max | `min_int32`, `max_int32`, `min_int64`, `max_int64`, `min_float64`, `max_float64` |
| Clamp | `clamp_int32`, `clamp_float64` |
| Bitwise | `bitand_int32`, `bitor_int32`, `bitxor_int32`, `bitnot_int32`, `shl_int32`, `shr_int32` |
| Comparison | `eq_int32`, `ne_int32`, `lt_int32`, `le_int32`, `gt_int32`, `ge_int32` |
| Boolean logic | `bool_and`, `bool_or`, `bool_not` |
| Fibonacci | `fib_int32`, `fib_int64` |
| Factorial | `factorial_int32`, `factorial_int64` |
| GCD | `gcd_int32` |
| Identity | `identity_int32`, `identity_int64`, `identity_float32`, `identity_float64` |
| Print | `print_static_string`, `print_int32`, `print_int64`, `print_float32`, `print_float64` |
| String I/O | `print_input_string`, `return_static_string`, `return_input_string` |
| String ops | `string_length`, `string_concat`, `string_repeat`, `string_eq` |

## Prerequisites

Enter the dev shell (requires [Nix](https://nixos.org/)):

```sh
nix develop .#wasm-mojo
```

This provides `just`, `mojo`, `deno`, `llc`, and `wasm-ld`.

## Usage

Build the WASM binary:

```sh
just build
```

Run the tests:

```sh
just test
```
