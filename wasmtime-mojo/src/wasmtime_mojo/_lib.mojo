"""Wasmtime shared library loader and raw FFI function wrappers.

This module loads libwasmtime.so via DLHandle and provides thin typed
wrappers around the C API functions needed by wasmtime-mojo.

The DLHandle is loaded lazily on first use via `get_lib()`.
"""

from sys.ffi import DLHandle
from memory import UnsafePointer

from ._types import (
    EnginePtr,
    StorePtr,
    ContextPtr,
    ModulePtr,
    LinkerPtr,
    ErrorPtr,
    TrapPtr,
    FuncTypePtr,
    ValTypePtr,
    CallerPtr,
    WasmtimeVal,
    WasmtimeFunc,
    WasmtimeInstance,
    WasmtimeGlobal,
    WasmtimeMemory,
    WasmtimeExtern,
    WasmByteVec,
    WasmValtypeVec,
    WasmtimeCallback,
    FinalizerCallback,
)

# ---------------------------------------------------------------------------
# Library loading
# ---------------------------------------------------------------------------


@always_inline
fn get_lib() raises -> DLHandle:
    """Return a handle to the wasmtime shared library.

    Creates a new DLHandle each call.  On Linux the underlying dlopen(3)
    caches library handles, so repeated calls are cheap and always return
    the same loaded library.
    """
    return DLHandle("libwasmtime.so")


# ═══════════════════════════════════════════════════════════════════════════
# Engine
# ═══════════════════════════════════════════════════════════════════════════


fn wasm_engine_new() raises -> EnginePtr:
    """Create a new wasm engine."""
    var lib = get_lib()
    var f = lib.get_function[fn () -> EnginePtr]("wasm_engine_new")
    return f()


fn wasm_engine_delete(engine: EnginePtr) raises:
    """Delete a wasm engine."""
    var lib = get_lib()
    var f = lib.get_function[fn (EnginePtr) -> None]("wasm_engine_delete")
    f(engine)


# ═══════════════════════════════════════════════════════════════════════════
# Store / Context
# ═══════════════════════════════════════════════════════════════════════════


fn wasmtime_store_new(
    engine: EnginePtr,
    data: UnsafePointer[NoneType],
    finalizer: UnsafePointer[NoneType],
) raises -> StorePtr:
    """Create a new wasmtime store."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            EnginePtr, UnsafePointer[NoneType], UnsafePointer[NoneType]
        ) -> StorePtr
    ]("wasmtime_store_new")
    return f(engine, data, finalizer)


fn wasmtime_store_delete(store: StorePtr) raises:
    """Delete a wasmtime store."""
    var lib = get_lib()
    var f = lib.get_function[fn (StorePtr) -> None]("wasmtime_store_delete")
    f(store)


fn wasmtime_store_context(store: StorePtr) raises -> ContextPtr:
    """Get the context from a wasmtime store."""
    var lib = get_lib()
    var f = lib.get_function[fn (StorePtr) -> ContextPtr](
        "wasmtime_store_context"
    )
    return f(store)


# ═══════════════════════════════════════════════════════════════════════════
# Module
# ═══════════════════════════════════════════════════════════════════════════


fn wasmtime_module_new(
    engine: EnginePtr,
    wasm: UnsafePointer[UInt8],
    wasm_len: Int,
    ret: UnsafePointer[ModulePtr],
) raises -> ErrorPtr:
    """Compile a WASM binary into a module."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            EnginePtr, UnsafePointer[UInt8], Int, UnsafePointer[ModulePtr]
        ) -> ErrorPtr
    ]("wasmtime_module_new")
    return f(engine, wasm, wasm_len, ret)


fn wasmtime_module_delete(module: ModulePtr) raises:
    """Delete a compiled module."""
    var lib = get_lib()
    var f = lib.get_function[fn (ModulePtr) -> None]("wasmtime_module_delete")
    f(module)


# ═══════════════════════════════════════════════════════════════════════════
# Linker
# ═══════════════════════════════════════════════════════════════════════════


fn wasmtime_linker_new(engine: EnginePtr) raises -> LinkerPtr:
    """Create a new linker for the given engine."""
    var lib = get_lib()
    var f = lib.get_function[fn (EnginePtr) -> LinkerPtr]("wasmtime_linker_new")
    return f(engine)


fn wasmtime_linker_delete(linker: LinkerPtr) raises:
    """Delete a linker."""
    var lib = get_lib()
    var f = lib.get_function[fn (LinkerPtr) -> None]("wasmtime_linker_delete")
    f(linker)


fn wasmtime_linker_define_func(
    linker: LinkerPtr,
    module_name: UnsafePointer[UInt8],
    module_name_len: Int,
    func_name: UnsafePointer[UInt8],
    func_name_len: Int,
    func_type: FuncTypePtr,
    callback: WasmtimeCallback,
    env: UnsafePointer[NoneType],
    finalizer: UnsafePointer[NoneType],
) raises -> ErrorPtr:
    """Define a host function in the linker."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            LinkerPtr,
            UnsafePointer[UInt8],
            Int,
            UnsafePointer[UInt8],
            Int,
            FuncTypePtr,
            WasmtimeCallback,
            UnsafePointer[NoneType],
            UnsafePointer[NoneType],
        ) -> ErrorPtr
    ]("wasmtime_linker_define_func")
    return f(
        linker,
        module_name,
        module_name_len,
        func_name,
        func_name_len,
        func_type,
        callback,
        env,
        finalizer,
    )


fn wasmtime_linker_instantiate(
    linker: LinkerPtr,
    context: ContextPtr,
    module: ModulePtr,
    instance: UnsafePointer[WasmtimeInstance],
    trap: UnsafePointer[TrapPtr],
) raises -> ErrorPtr:
    """Instantiate a module using the linker definitions."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            LinkerPtr,
            ContextPtr,
            ModulePtr,
            UnsafePointer[WasmtimeInstance],
            UnsafePointer[TrapPtr],
        ) -> ErrorPtr
    ]("wasmtime_linker_instantiate")
    return f(linker, context, module, instance, trap)


# ═══════════════════════════════════════════════════════════════════════════
# Instance exports
# ═══════════════════════════════════════════════════════════════════════════


fn wasmtime_instance_export_get(
    context: ContextPtr,
    instance: UnsafePointer[WasmtimeInstance],
    name: UnsafePointer[UInt8],
    name_len: Int,
    item: UnsafePointer[WasmtimeExtern],
) raises -> Bool:
    """Look up an export from an instance by name."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            ContextPtr,
            UnsafePointer[WasmtimeInstance],
            UnsafePointer[UInt8],
            Int,
            UnsafePointer[WasmtimeExtern],
        ) -> Bool
    ]("wasmtime_instance_export_get")
    return f(context, instance, name, name_len, item)


# ═══════════════════════════════════════════════════════════════════════════
# Function calls
# ═══════════════════════════════════════════════════════════════════════════


fn wasmtime_func_call(
    context: ContextPtr,
    func: UnsafePointer[WasmtimeFunc],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
    trap: UnsafePointer[TrapPtr],
) raises -> ErrorPtr:
    """Call a WASM function."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            ContextPtr,
            UnsafePointer[WasmtimeFunc],
            UnsafePointer[WasmtimeVal],
            Int,
            UnsafePointer[WasmtimeVal],
            Int,
            UnsafePointer[TrapPtr],
        ) -> ErrorPtr
    ]("wasmtime_func_call")
    return f(context, func, args, nargs, results, nresults, trap)


# ═══════════════════════════════════════════════════════════════════════════
# Global access
# ═══════════════════════════════════════════════════════════════════════════


fn wasmtime_global_get(
    context: ContextPtr,
    global_: UnsafePointer[WasmtimeGlobal],
    result: UnsafePointer[WasmtimeVal],
) raises:
    """Read the value of a WASM global."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            ContextPtr,
            UnsafePointer[WasmtimeGlobal],
            UnsafePointer[WasmtimeVal],
        ) -> None
    ]("wasmtime_global_get")
    f(context, global_, result)


# ═══════════════════════════════════════════════════════════════════════════
# Memory access
# ═══════════════════════════════════════════════════════════════════════════


fn wasmtime_memory_data(
    context: ContextPtr,
    memory: UnsafePointer[WasmtimeMemory],
) raises -> UnsafePointer[UInt8]:
    """Get a pointer to WASM linear memory data."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (ContextPtr, UnsafePointer[WasmtimeMemory]) -> UnsafePointer[UInt8]
    ]("wasmtime_memory_data")
    return f(context, memory)


fn wasmtime_memory_data_size(
    context: ContextPtr,
    memory: UnsafePointer[WasmtimeMemory],
) raises -> Int:
    """Get the current size of WASM linear memory in bytes."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (ContextPtr, UnsafePointer[WasmtimeMemory]) -> Int
    ]("wasmtime_memory_data_size")
    return f(context, memory)


# ═══════════════════════════════════════════════════════════════════════════
# Val types and func types — needed for defining import signatures
# ═══════════════════════════════════════════════════════════════════════════


fn wasm_valtype_new(kind: UInt8) raises -> ValTypePtr:
    """Create a new WASM value type from a kind constant."""
    var lib = get_lib()
    var f = lib.get_function[fn (UInt8) -> ValTypePtr]("wasm_valtype_new")
    return f(kind)


fn wasm_valtype_delete(vt: ValTypePtr) raises:
    """Delete a WASM value type."""
    var lib = get_lib()
    var f = lib.get_function[fn (ValTypePtr) -> None]("wasm_valtype_delete")
    f(vt)


fn wasm_functype_new(
    params: UnsafePointer[WasmValtypeVec],
    results: UnsafePointer[WasmValtypeVec],
) raises -> FuncTypePtr:
    """Create a function type from param and result type vecs.

    Note: takes ownership of both vecs and the valtypes within them.
    Do NOT delete them after calling this.
    """
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            UnsafePointer[WasmValtypeVec], UnsafePointer[WasmValtypeVec]
        ) -> FuncTypePtr
    ]("wasm_functype_new")
    return f(params, results)


fn wasm_functype_delete(ft: FuncTypePtr) raises:
    """Delete a function type."""
    var lib = get_lib()
    var f = lib.get_function[fn (FuncTypePtr) -> None]("wasm_functype_delete")
    f(ft)


fn wasm_valtype_vec_new(
    result: UnsafePointer[WasmValtypeVec],
    size: Int,
    data: UnsafePointer[ValTypePtr],
) raises:
    """Create a new valtype vec from an array of valtype pointers."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            UnsafePointer[WasmValtypeVec], Int, UnsafePointer[ValTypePtr]
        ) -> None
    ]("wasm_valtype_vec_new")
    f(result, size, data)


fn wasm_valtype_vec_new_empty(result: UnsafePointer[WasmValtypeVec]) raises:
    """Create a new empty valtype vec."""
    var lib = get_lib()
    var f = lib.get_function[fn (UnsafePointer[WasmValtypeVec]) -> None](
        "wasm_valtype_vec_new_empty"
    )
    f(result)


fn wasm_valtype_vec_delete(vec: UnsafePointer[WasmValtypeVec]) raises:
    """Delete a valtype vec."""
    var lib = get_lib()
    var f = lib.get_function[fn (UnsafePointer[WasmValtypeVec]) -> None](
        "wasm_valtype_vec_delete"
    )
    f(vec)


# ═══════════════════════════════════════════════════════════════════════════
# Error handling
# ═══════════════════════════════════════════════════════════════════════════


fn wasmtime_error_message(
    error: ErrorPtr,
    message: UnsafePointer[WasmByteVec],
) raises:
    """Extract the error message from a wasmtime error."""
    var lib = get_lib()
    var f = lib.get_function[fn (ErrorPtr, UnsafePointer[WasmByteVec]) -> None](
        "wasmtime_error_message"
    )
    f(error, message)


fn wasmtime_error_delete(error: ErrorPtr) raises:
    """Delete a wasmtime error."""
    var lib = get_lib()
    var f = lib.get_function[fn (ErrorPtr) -> None]("wasmtime_error_delete")
    f(error)


fn wasm_byte_vec_delete(vec: UnsafePointer[WasmByteVec]) raises:
    """Delete a byte vec."""
    var lib = get_lib()
    var f = lib.get_function[fn (UnsafePointer[WasmByteVec]) -> None](
        "wasm_byte_vec_delete"
    )
    f(vec)


# ═══════════════════════════════════════════════════════════════════════════
# Trap handling
# ═══════════════════════════════════════════════════════════════════════════


fn wasm_trap_message(
    trap: TrapPtr,
    message: UnsafePointer[WasmByteVec],
) raises:
    """Extract the message from a trap."""
    var lib = get_lib()
    var f = lib.get_function[fn (TrapPtr, UnsafePointer[WasmByteVec]) -> None](
        "wasm_trap_message"
    )
    f(trap, message)


fn wasm_trap_delete(trap: TrapPtr) raises:
    """Delete a trap."""
    var lib = get_lib()
    var f = lib.get_function[fn (TrapPtr) -> None]("wasm_trap_delete")
    f(trap)


# ═══════════════════════════════════════════════════════════════════════════
# Helper: build a wasm_functype_t from lists of WASM val kinds
# ═══════════════════════════════════════════════════════════════════════════


fn make_functype(
    param_kinds: List[UInt8], result_kinds: List[UInt8]
) raises -> FuncTypePtr:
    """Create a wasm_functype_t from parameter and result kind lists.

    Each element should be one of WASM_I32, WASM_I64, WASM_F32, WASM_F64.
    The returned FuncTypePtr must be freed with wasm_functype_delete
    (though wasmtime_linker_define_func takes ownership).
    """
    # Build params vec
    var params = WasmValtypeVec()
    var params_ptr = UnsafePointer(to=params)
    if len(param_kinds) == 0:
        wasm_valtype_vec_new_empty(params_ptr)
    else:
        var ptypes = UnsafePointer[ValTypePtr].alloc(len(param_kinds))
        for i in range(len(param_kinds)):
            ptypes[i] = wasm_valtype_new(param_kinds[i])
        wasm_valtype_vec_new(params_ptr, len(param_kinds), ptypes)
        ptypes.free()

    # Build results vec
    var results = WasmValtypeVec()
    var results_ptr = UnsafePointer(to=results)
    if len(result_kinds) == 0:
        wasm_valtype_vec_new_empty(results_ptr)
    else:
        var rtypes = UnsafePointer[ValTypePtr].alloc(len(result_kinds))
        for i in range(len(result_kinds)):
            rtypes[i] = wasm_valtype_new(result_kinds[i])
        wasm_valtype_vec_new(results_ptr, len(result_kinds), rtypes)
        rtypes.free()

    # wasm_functype_new takes ownership of both vecs
    return wasm_functype_new(params_ptr, results_ptr)


# ═══════════════════════════════════════════════════════════════════════════
# Helper: extract error/trap message as String
# ═══════════════════════════════════════════════════════════════════════════


fn error_message(error: ErrorPtr) raises -> String:
    """Extract the message from a wasmtime_error_t, delete it, and return
    the message as a Mojo String."""
    var msg = WasmByteVec()
    var msg_ptr = UnsafePointer(to=msg)
    wasmtime_error_message(error, msg_ptr)
    var result = String("")
    if msg.size > 0 and msg.data:
        var buf = List[UInt8](capacity=msg.size + 1)
        for i in range(msg.size):
            buf.append(msg.data[i])
        buf.append(0)  # null-terminate
        result = String(bytes=buf)
    wasm_byte_vec_delete(msg_ptr)
    wasmtime_error_delete(error)
    return result


fn trap_message(trap: TrapPtr) raises -> String:
    """Extract the message from a wasm_trap_t, delete it, and return
    the message as a Mojo String."""
    var msg = WasmByteVec()
    var msg_ptr = UnsafePointer(to=msg)
    wasm_trap_message(trap, msg_ptr)
    var result = String("")
    if msg.size > 0 and msg.data:
        var buf = List[UInt8](capacity=msg.size + 1)
        for i in range(msg.size):
            buf.append(msg.data[i])
        buf.append(0)  # null-terminate
        result = String(bytes=buf)
    wasm_byte_vec_delete(msg_ptr)
    wasm_trap_delete(trap)
    return result


# ═══════════════════════════════════════════════════════════════════════════
# Helper: check error/trap and raise if present
# ═══════════════════════════════════════════════════════════════════════════


fn check_error(error: ErrorPtr, trap: UnsafePointer[TrapPtr]) raises:
    """Check the error and trap pointers returned from a wasmtime API call.
    If either is non-null, raises with the appropriate message."""
    if error:
        raise Error("wasmtime error: " + error_message(error))
    if trap[]:
        raise Error("wasmtime trap: " + trap_message(trap[]))


fn check_error_only(error: ErrorPtr) raises:
    """Check the error pointer returned from a wasmtime API call.
    If non-null, raises with the message."""
    if error:
        raise Error("wasmtime error: " + error_message(error))
