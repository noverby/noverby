"""High-level wrapper for wasmtime_module_t.

A Module represents a compiled WebAssembly module. It is created from
raw WASM bytes (binary format) using an Engine for compilation settings.

Modules are immutable once compiled and can be instantiated multiple times
with different Stores.

Usage:
    var engine = Engine()
    var wasm_bytes = read_wasm_file("module.wasm")
    var module = Module(engine.ptr(), wasm_bytes)
    # ... use module with a Linker to create instances ...
"""

from memory import UnsafePointer

from ._types import EnginePtr, ModulePtr, ErrorPtr
from ._lib import (
    wasmtime_module_new,
    wasmtime_module_delete,
    error_message,
)


struct Module:
    """RAII wrapper around wasmtime_module_t.

    Owns the underlying module pointer and deletes it on destruction.
    A Module is created by compiling WASM bytes with an Engine.
    """

    var _ptr: ModulePtr

    fn __init__(
        out self, engine_ptr: EnginePtr, wasm_bytes: List[UInt8]
    ) raises:
        """Compile a WASM binary into a Module.

        Args:
            engine_ptr: Raw pointer to the wasm_engine_t used for compilation.
                The engine must outlive this module.
            wasm_bytes: The raw WASM binary bytes (.wasm format).

        Raises:
            Error: If compilation fails (e.g. invalid WASM binary).
        """
        var module_out = UnsafePointer[ModulePtr].alloc(1)
        module_out[] = ModulePtr()

        var data_ptr = wasm_bytes.unsafe_ptr()
        var err = wasmtime_module_new(
            engine_ptr,
            data_ptr,
            len(wasm_bytes),
            module_out,
        )

        if err:
            var msg = error_message(err)
            module_out.free()
            raise Error("Failed to compile WASM module: " + msg)

        self._ptr = module_out[]
        module_out.free()

    fn __init__(out self, *, var _ptr: ModulePtr):
        """Create a Module from a raw pointer (takes ownership).

        Args:
            _ptr: Raw pointer to an already-compiled wasmtime_module_t.
                This Module takes ownership and will delete it on destruction.
        """
        self._ptr = _ptr

    fn __del__(deinit self):
        """Delete the module, freeing compilation artifacts."""
        if self._ptr:
            try:
                wasmtime_module_delete(self._ptr)
            except:
                pass

    fn __moveinit__(out self, deinit other: Self):
        """Move constructor — transfers ownership of the module pointer."""
        self._ptr = other._ptr

    fn ptr(self) -> ModulePtr:
        """Return the raw module pointer for FFI calls."""
        return self._ptr
