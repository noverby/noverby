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

The browser-side JavaScript runtime (`index.html`) provides the necessary environment stubs for the WASM module, including:

- **Memory management** — a bump allocator for `KGEN_CompilerRT_AlignedAlloc`/`AlignedFree`
- **I/O** — `write` routed to `console.log`/`console.error` for stdout/stderr
- **Math builtins** — `fma`, `fmin`, `fmax` and their float variants
- **Libc stubs** — `dup`, `fdopen`, `fflush`, `fclose`, `__cxa_atexit`

## Exported Mojo functions

The Mojo source (`src/main.mojo`) exports functions demonstrating interop across several categories:

| Category | Functions |
|---|---|
| Arithmetic | `add_int32`, `add_int64`, `add_float32`, `add_float64` |
| Power | `pow_int32`, `pow_int64`, `pow_float32`, `pow_float64` |
| Print | `print_static_string`, `print_int32`, `print_int64`, `print_float32`, `print_float64` |
| String I/O | `print_input_string`, `return_static_string`, `return_input_string` |
| Global state | `get_global_string`, `set_global_string` |

## Prerequisites

Enter the dev shell (requires [Nix](https://nixos.org/)):

```sh
nix develop .#mojo-wasm
```

This provides `just`, `mojo`, `python3`, `llc`, and `wasm-ld`.

## Usage

Build the WASM binary:

```sh
just build
```

Start the dev server:

```sh
just server
```

Open <http://localhost:8000> and check the browser console for output.