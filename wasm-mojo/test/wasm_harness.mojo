"""WASM test harness using wasmtime-mojo (pure Mojo FFI bindings).

Provides a WasmInstance for loading the Mojo WASM binary and interacting with
its exported functions, including string struct read/write operations
that mirror the TypeScript runtime/strings.ts and runtime/memory.ts.

This module is designed to be imported from Mojo test files:

    from wasm_harness import WasmInstance, get_instance

Import signatures are derived from `wasm-objdump -j Import -x build/out.wasm`:

  Import[16]:
   - func[0]  sig=0  (i64, i64) -> i64      KGEN_CompilerRT_AlignedAlloc
   - func[1]  sig=1  (i64) -> nil            KGEN_CompilerRT_AlignedFree
   - func[2]  sig=2  (f32, f32, f32) -> f32  fmaf
   - func[3]  sig=3  (f32, f32) -> f32       fminf
   - func[4]  sig=3  (f32, f32) -> f32       fmaxf
   - func[5]  sig=4  (f64, f64, f64) -> f64  fma
   - func[6]  sig=5  (f64, f64) -> f64       fmin
   - func[7]  sig=5  (f64, f64) -> f64       fmax
   - func[8]  sig=0  (i64, i64) -> i64       KGEN_CompilerRT_GetStackTrace
   - func[9]  sig=1  (i64) -> nil            free
   - func[10] sig=6  (i32) -> i32            dup
   - func[11] sig=7  (i32, i64) -> i64       fdopen
   - func[12] sig=8  (i64) -> i32            fflush
   - func[13] sig=8  (i64) -> i32            fclose
   - func[14] sig=9  (i64, i64, i64) -> i32  KGEN_CompilerRT_fprintf
   - func[15] sig=9  (i64, i64, i64) -> i32  write
"""

from memory import UnsafePointer, memcpy, memset_zero
from pathlib import Path
from sys.ffi import DLHandle

from wasmtime_mojo import (
    Engine,
    Store,
    Module,
    Linker,
    WasmtimeVal,
    WasmtimeFunc,
    WasmtimeInstance,
    WasmtimeGlobal,
    WasmtimeMemory,
    WasmtimeExtern,
    WasmtimeCallback,
    ContextPtr,
    WASM_I32,
    WASM_I64,
    WASM_F32,
    WASM_F64,
    WASMTIME_I32,
    WASMTIME_I64,
    WASMTIME_F32,
    WASMTIME_F64,
    instance_get_func,
    instance_get_memory,
    instance_get_global,
    global_get_i32,
    global_get_i64,
    func_call,
    func_call_0,
    func_call_i32,
    func_call_i64,
    func_call_f32,
    func_call_f64,
    memory_data_ptr,
    memory_data_size,
    memory_read_bytes,
    memory_write_bytes,
    memory_read_i64_le,
    memory_write_i64_le,
    memory_read_u64_le,
)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

alias STRING_STRUCT_SIZE: Int = 24
alias STRING_STRUCT_ALIGN: Int = 8

alias SSO_FLAG: UInt64 = 0x8000_0000_0000_0000
alias SSO_LEN_MASK: UInt64 = 0x1F00_0000_0000_0000

alias WASM_PATH = "build/out.wasm"


# ---------------------------------------------------------------------------
# SharedState — heap-allocated state shared by all import callbacks
# ---------------------------------------------------------------------------


struct SharedState:
    """Mutable state shared across all WASM import callbacks.

    Allocated on the heap so a stable pointer can be passed as the
    callback `env` parameter.
    """

    var bump_ptr: Int
    var context: ContextPtr
    var memory: WasmtimeMemory
    var captured_stdout: List[String]
    var has_memory: Bool

    fn __init__(out self):
        self.bump_ptr = 0
        self.context = ContextPtr()
        self.memory = WasmtimeMemory()
        self.captured_stdout = List[String]()
        self.has_memory = False

    fn aligned_alloc(mut self, align: Int, size: Int) -> Int:
        """Bump-allocate *size* bytes with the given alignment."""
        var remainder = self.bump_ptr % align
        if remainder != 0:
            self.bump_ptr += align - remainder
        var ptr = self.bump_ptr
        self.bump_ptr += size
        return ptr


# ---------------------------------------------------------------------------
# Import callbacks
#
# Each callback has the wasmtime_func_callback_t signature:
#   fn(env, caller, args, nargs, results, nresults) -> trap_ptr
#
# Return null pointer (UnsafePointer[NoneType]()) on success.
# ---------------------------------------------------------------------------


# Helper to get the SharedState from the env pointer.
@always_inline
fn _state(
    env: UnsafePointer[NoneType],
) -> UnsafePointer[SharedState]:
    return env.bitcast[SharedState]()


# -- func[0] KGEN_CompilerRT_AlignedAlloc: (i64, i64) -> i64 --------------


fn _cb_aligned_alloc(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    var state = _state(env)
    var align = Int(args[0].get_i64())
    var size = Int(args[1].get_i64())
    var ptr = state[].aligned_alloc(align, size)
    results[0] = WasmtimeVal.from_i64(Int64(ptr))
    return UnsafePointer[NoneType]()


# -- func[1] KGEN_CompilerRT_AlignedFree: (i64) -> nil --------------------


fn _cb_aligned_free(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    # Bump allocator never reclaims.
    return UnsafePointer[NoneType]()


# -- func[2] fmaf: (f32, f32, f32) -> f32 ---------------------------------


fn _cb_fmaf(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    var x = args[0].get_f32()
    var y = args[1].get_f32()
    var z = args[2].get_f32()
    # fused multiply-add (truncated to f32 precision)
    var r = x * y + z
    results[0] = WasmtimeVal.from_f32(r)
    return UnsafePointer[NoneType]()


# -- func[3] fminf: (f32, f32) -> f32 -------------------------------------


fn _cb_fminf(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    var x = args[0].get_f32()
    var y = args[1].get_f32()
    var r = y if x > y else x
    results[0] = WasmtimeVal.from_f32(r)
    return UnsafePointer[NoneType]()


# -- func[4] fmaxf: (f32, f32) -> f32 -------------------------------------


fn _cb_fmaxf(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    var x = args[0].get_f32()
    var y = args[1].get_f32()
    var r = x if x > y else y
    results[0] = WasmtimeVal.from_f32(r)
    return UnsafePointer[NoneType]()


# -- func[5] fma: (f64, f64, f64) -> f64 ----------------------------------


fn _cb_fma(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    var x = args[0].get_f64()
    var y = args[1].get_f64()
    var z = args[2].get_f64()
    var r = x * y + z
    results[0] = WasmtimeVal.from_f64(r)
    return UnsafePointer[NoneType]()


# -- func[6] fmin: (f64, f64) -> f64 --------------------------------------


fn _cb_fmin(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    var x = args[0].get_f64()
    var y = args[1].get_f64()
    var r = y if x > y else x
    results[0] = WasmtimeVal.from_f64(r)
    return UnsafePointer[NoneType]()


# -- func[7] fmax: (f64, f64) -> f64 --------------------------------------


fn _cb_fmax(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    var x = args[0].get_f64()
    var y = args[1].get_f64()
    var r = x if x > y else y
    results[0] = WasmtimeVal.from_f64(r)
    return UnsafePointer[NoneType]()


# -- func[8] KGEN_CompilerRT_GetStackTrace: (i64, i64) -> i64 -------------


fn _cb_get_stack_trace(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    results[0] = WasmtimeVal.from_i64(0)
    return UnsafePointer[NoneType]()


# -- func[9] free: (i64) -> nil -------------------------------------------


fn _cb_free(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    # Bump allocator never reclaims.
    return UnsafePointer[NoneType]()


# -- func[10] dup: (i32) -> i32 -------------------------------------------


fn _cb_dup(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    results[0] = WasmtimeVal.from_i32(1)
    return UnsafePointer[NoneType]()


# -- func[11] fdopen: (i32, i64) -> i64 -----------------------------------


fn _cb_fdopen(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    results[0] = WasmtimeVal.from_i64(1)
    return UnsafePointer[NoneType]()


# -- func[12] fflush: (i64) -> i32 ----------------------------------------


fn _cb_fflush(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    results[0] = WasmtimeVal.from_i32(1)
    return UnsafePointer[NoneType]()


# -- func[13] fclose: (i64) -> i32 ----------------------------------------


fn _cb_fclose(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    results[0] = WasmtimeVal.from_i32(1)
    return UnsafePointer[NoneType]()


# -- func[14] KGEN_CompilerRT_fprintf: (i64, i64, i64) -> i32 -------------


fn _cb_fprintf(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    results[0] = WasmtimeVal.from_i32(0)
    return UnsafePointer[NoneType]()


# -- func[15] write: (i64, i64, i64) -> i32 -------------------------------


fn _cb_write(
    env: UnsafePointer[NoneType],
    caller: UnsafePointer[NoneType],
    args: UnsafePointer[WasmtimeVal],
    nargs: Int,
    results: UnsafePointer[WasmtimeVal],
    nresults: Int,
) -> UnsafePointer[NoneType]:
    var state = _state(env)
    var fd = Int(args[0].get_i64())
    var ptr = Int(args[1].get_i64())
    var length = Int(args[2].get_i64())

    if length == 0:
        results[0] = WasmtimeVal.from_i32(0)
        return UnsafePointer[NoneType]()

    if not state[].has_memory:
        results[0] = WasmtimeVal.from_i32(-1)
        return UnsafePointer[NoneType]()

    if fd == 1:
        # stdout — capture the written text
        var bytes = memory_read_bytes(
            state[].context, state[].memory, ptr, length
        )
        # Build a String from the raw bytes
        var buf = List[UInt8](capacity=length + 1)
        for i in range(length):
            buf.append(bytes[i])
        buf.append(0)  # null-terminate
        var text = String(buf)
        state[].captured_stdout.append(text)
        results[0] = WasmtimeVal.from_i32(Int32(length))
    elif fd == 2:
        # stderr — just report length, don't capture
        results[0] = WasmtimeVal.from_i32(Int32(length))
    else:
        results[0] = WasmtimeVal.from_i32(-1)

    return UnsafePointer[NoneType]()


# ---------------------------------------------------------------------------
# WasmInstance — high-level harness wrapping engine, store, linker, instance
# ---------------------------------------------------------------------------


struct WasmInstance:
    """Wraps a Wasmtime instance with helper methods mirroring the Python harness.

    Provides:
    - `call(name, args)` to invoke exported WASM functions
    - `write_string_struct(s)` / `read_string_struct(ptr)` for Mojo String structs
    - `alloc_string_struct()` for allocating empty string struct slots
    - Typed call shortcuts (call_i32, call_i64, call_f32, call_f64)
    """

    var _engine: Engine
    var _store: Store
    var _module: Module
    var _linker: Linker
    var _instance: WasmtimeInstance
    var _memory: WasmtimeMemory
    var _state_ptr: UnsafePointer[SharedState]

    fn __init__(out self, wasm_path: String) raises:
        """Create a WasmInstance by loading and instantiating the WASM binary.

        Args:
            wasm_path: Path to the .wasm binary file.
        """
        # Read the WASM binary
        var wasm_bytes: List[UInt8]
        with open(wasm_path, "rb") as f:
            var data = f.read_bytes()
            wasm_bytes = List[UInt8](capacity=len(data))
            for i in range(len(data)):
                wasm_bytes.append(data[i])

        # Allocate shared state on the heap
        self._state_ptr = UnsafePointer[SharedState].alloc(1)
        self._state_ptr[] = SharedState()

        var env = self._state_ptr.bitcast[NoneType]()
        var no_fin = UnsafePointer[NoneType]()

        # Create engine, store, linker
        self._engine = Engine()
        self._store = Store(self._engine.ptr())
        self._linker = Linker(self._engine.ptr())

        # ──────────────────────────────────────────────────────────────
        # Define all 16 imports
        # ──────────────────────────────────────────────────────────────

        # func[0] KGEN_CompilerRT_AlignedAlloc: (i64, i64) -> i64
        self._linker.define_func(
            "env",
            "KGEN_CompilerRT_AlignedAlloc",
            List[UInt8](WASM_I64, WASM_I64),
            List[UInt8](WASM_I64),
            _cb_aligned_alloc,
            env,
        )

        # func[1] KGEN_CompilerRT_AlignedFree: (i64) -> nil
        self._linker.define_func(
            "env",
            "KGEN_CompilerRT_AlignedFree",
            List[UInt8](WASM_I64),
            List[UInt8](),
            _cb_aligned_free,
            env,
        )

        # func[2] fmaf: (f32, f32, f32) -> f32
        self._linker.define_func(
            "env",
            "fmaf",
            List[UInt8](WASM_F32, WASM_F32, WASM_F32),
            List[UInt8](WASM_F32),
            _cb_fmaf,
            env,
        )

        # func[3] fminf: (f32, f32) -> f32
        self._linker.define_func(
            "env",
            "fminf",
            List[UInt8](WASM_F32, WASM_F32),
            List[UInt8](WASM_F32),
            _cb_fminf,
            env,
        )

        # func[4] fmaxf: (f32, f32) -> f32
        self._linker.define_func(
            "env",
            "fmaxf",
            List[UInt8](WASM_F32, WASM_F32),
            List[UInt8](WASM_F32),
            _cb_fmaxf,
            env,
        )

        # func[5] fma: (f64, f64, f64) -> f64
        self._linker.define_func(
            "env",
            "fma",
            List[UInt8](WASM_F64, WASM_F64, WASM_F64),
            List[UInt8](WASM_F64),
            _cb_fma,
            env,
        )

        # func[6] fmin: (f64, f64) -> f64
        self._linker.define_func(
            "env",
            "fmin",
            List[UInt8](WASM_F64, WASM_F64),
            List[UInt8](WASM_F64),
            _cb_fmin,
            env,
        )

        # func[7] fmax: (f64, f64) -> f64
        self._linker.define_func(
            "env",
            "fmax",
            List[UInt8](WASM_F64, WASM_F64),
            List[UInt8](WASM_F64),
            _cb_fmax,
            env,
        )

        # func[8] KGEN_CompilerRT_GetStackTrace: (i64, i64) -> i64
        self._linker.define_func(
            "env",
            "KGEN_CompilerRT_GetStackTrace",
            List[UInt8](WASM_I64, WASM_I64),
            List[UInt8](WASM_I64),
            _cb_get_stack_trace,
            env,
        )

        # func[9] free: (i64) -> nil
        self._linker.define_func(
            "env",
            "free",
            List[UInt8](WASM_I64),
            List[UInt8](),
            _cb_free,
            env,
        )

        # func[10] dup: (i32) -> i32
        self._linker.define_func(
            "env",
            "dup",
            List[UInt8](WASM_I32),
            List[UInt8](WASM_I32),
            _cb_dup,
            env,
        )

        # func[11] fdopen: (i32, i64) -> i64
        self._linker.define_func(
            "env",
            "fdopen",
            List[UInt8](WASM_I32, WASM_I64),
            List[UInt8](WASM_I64),
            _cb_fdopen,
            env,
        )

        # func[12] fflush: (i64) -> i32
        self._linker.define_func(
            "env",
            "fflush",
            List[UInt8](WASM_I64),
            List[UInt8](WASM_I32),
            _cb_fflush,
            env,
        )

        # func[13] fclose: (i64) -> i32
        self._linker.define_func(
            "env",
            "fclose",
            List[UInt8](WASM_I64),
            List[UInt8](WASM_I32),
            _cb_fclose,
            env,
        )

        # func[14] KGEN_CompilerRT_fprintf: (i64, i64, i64) -> i32
        self._linker.define_func(
            "env",
            "KGEN_CompilerRT_fprintf",
            List[UInt8](WASM_I64, WASM_I64, WASM_I64),
            List[UInt8](WASM_I32),
            _cb_fprintf,
            env,
        )

        # func[15] write: (i64, i64, i64) -> i32
        self._linker.define_func(
            "env",
            "write",
            List[UInt8](WASM_I64, WASM_I64, WASM_I64),
            List[UInt8](WASM_I32),
            _cb_write,
            env,
        )

        # ──────────────────────────────────────────────────────────────
        # Compile and instantiate the module
        # ──────────────────────────────────────────────────────────────

        self._module = Module(self._engine.ptr(), wasm_bytes)
        self._instance = self._linker.instantiate(
            self._store.context(), self._module.ptr()
        )

        # Obtain the memory export
        self._memory = instance_get_memory(
            self._store.context(), self._instance, "memory"
        )

        # Read __heap_base global to initialise the bump allocator
        var heap_base_global = instance_get_global(
            self._store.context(), self._instance, "__heap_base"
        )
        var heap_base = global_get_i64(self._store.context(), heap_base_global)

        # Update shared state with memory and context info
        self._state_ptr[].bump_ptr = Int(heap_base)
        self._state_ptr[].context = self._store.context()
        self._state_ptr[].memory = self._memory
        self._state_ptr[].has_memory = True

    fn __del__(deinit self):
        """Clean up: free the heap-allocated shared state."""
        if self._state_ptr:
            self._state_ptr.free()

    fn __moveinit__(out self, deinit other: Self):
        """Move constructor."""
        self._engine = other._engine^
        self._store = other._store^
        self._module = other._module^
        self._linker = other._linker^
        self._instance = other._instance
        self._memory = other._memory
        self._state_ptr = other._state_ptr

    # ------------------------------------------------------------------
    # Raw memory helpers
    # ------------------------------------------------------------------

    fn read_bytes(self, ptr: Int, length: Int) raises -> List[UInt8]:
        """Read *length* bytes from WASM memory at *ptr*."""
        return memory_read_bytes(
            self._store.context(), self._memory, ptr, length
        )

    fn write_bytes(self, ptr: Int, data: List[UInt8]) raises:
        """Write *data* bytes into WASM memory at *ptr*."""
        memory_write_bytes(self._store.context(), self._memory, ptr, data)

    fn read_i64_le(self, ptr: Int) raises -> Int64:
        """Read a little-endian i64 from WASM memory."""
        return memory_read_i64_le(self._store.context(), self._memory, ptr)

    fn read_u64_le(self, ptr: Int) raises -> UInt64:
        """Read a little-endian u64 from WASM memory."""
        return memory_read_u64_le(self._store.context(), self._memory, ptr)

    fn write_i64_le(self, ptr: Int, value: Int64) raises:
        """Write a little-endian i64 into WASM memory."""
        memory_write_i64_le(self._store.context(), self._memory, ptr, value)

    # ------------------------------------------------------------------
    # Bump allocator wrappers
    # ------------------------------------------------------------------

    fn aligned_alloc(self, align: Int, size: Int) -> Int:
        """Bump-allocate *size* bytes with the given alignment."""
        return self._state_ptr[].aligned_alloc(align, size)

    # ------------------------------------------------------------------
    # String struct operations (mirrors runtime/strings.ts)
    # ------------------------------------------------------------------

    fn write_string_struct(self, s: String) raises -> Int:
        """Allocate a Mojo String struct in WASM memory, populated with *s*.

        The struct is 24 bytes:
          - offset  0: data_ptr (i64) — pointer to UTF-8 bytes
          - offset  8: len      (i64) — byte length (no null terminator)
          - offset 16: capacity (i64) — buffer capacity (len + 1)

        Returns the WASM pointer to the struct.
        """
        var encoded = s.as_bytes()
        var data_len = len(s)

        # Allocate data buffer (with null terminator)
        var data_ptr = self.aligned_alloc(1, data_len + 1)
        var data_bytes = List[UInt8](capacity=data_len + 1)
        for i in range(len(encoded)):
            data_bytes.append(encoded[i])
        # Ensure null terminator
        if len(data_bytes) == 0 or data_bytes[len(data_bytes) - 1] != 0:
            data_bytes.append(0)
        self.write_bytes(data_ptr, data_bytes)

        # Allocate 24-byte String struct
        var struct_ptr = self.aligned_alloc(
            STRING_STRUCT_ALIGN, STRING_STRUCT_SIZE
        )
        self.write_i64_le(struct_ptr, Int64(data_ptr))
        self.write_i64_le(struct_ptr + 8, Int64(data_len))
        self.write_i64_le(struct_ptr + 16, Int64(data_len + 1))

        return struct_ptr

    fn alloc_string_struct(self) raises -> Int:
        """Allocate a zero-initialized 24-byte Mojo String struct.

        Returns the WASM pointer to the struct.
        """
        var struct_ptr = self.aligned_alloc(
            STRING_STRUCT_ALIGN, STRING_STRUCT_SIZE
        )
        var zeros = List[UInt8](capacity=STRING_STRUCT_SIZE)
        for _ in range(STRING_STRUCT_SIZE):
            zeros.append(0)
        self.write_bytes(struct_ptr, zeros)
        return struct_ptr

    fn read_string_struct(self, struct_ptr: Int) raises -> String:
        """Read a Mojo String struct back into a Mojo String.

        Handles both heap-allocated strings and SSO (small string optimization)
        strings.
        """
        var capacity = self.read_u64_le(struct_ptr + 16)

        var data_ptr: Int
        var length: Int

        if capacity & SSO_FLAG:
            # SSO: data inline at struct_ptr, length encoded in capacity
            data_ptr = struct_ptr
            length = Int((capacity & SSO_LEN_MASK) >> 56)
        else:
            data_ptr = Int(self.read_i64_le(struct_ptr))
            length = Int(self.read_i64_le(struct_ptr + 8))

        if length <= 0:
            return String("")

        var raw = self.read_bytes(data_ptr, length)
        var buf = List[UInt8](capacity=length + 1)
        for i in range(length):
            buf.append(raw[i])
        buf.append(0)  # null-terminate
        return String(buf)

    # ------------------------------------------------------------------
    # Captured stdout access
    # ------------------------------------------------------------------

    fn get_captured_stdout(self) raises -> List[String]:
        """Return the list of strings captured from WASM stdout writes."""
        return self._state_ptr[].captured_stdout.copy()

    fn clear_captured_stdout(self) raises:
        """Clear the captured stdout buffer."""
        self._state_ptr[].captured_stdout = List[String]()

    # ------------------------------------------------------------------
    # WASM function calling
    # ------------------------------------------------------------------

    fn get_func(self, name: String) raises -> WasmtimeFunc:
        """Look up an exported function by name.

        Args:
            name: The export function name.

        Returns:
            A WasmtimeFunc handle for the exported function.

        Raises:
            Error: If the export is not found or is not a function.
        """
        return instance_get_func(self._store.context(), self._instance, name)

    fn call(
        self,
        name: String,
        args: List[WasmtimeVal],
        nresults: Int = 1,
    ) raises -> List[WasmtimeVal]:
        """Call an exported WASM function by name.

        Args:
            name: The export function name.
            args: Arguments as WasmtimeVal values.
            nresults: Expected number of results (default 1).

        Returns:
            A List of WasmtimeVal results.

        Raises:
            Error: If the function is not found or the call fails/traps.
        """
        var func = self.get_func(name)
        return func_call(self._store.context(), func, args, nresults)

    fn call_void(self, name: String, args: List[WasmtimeVal]) raises:
        """Call an exported WASM function that returns no values.

        Args:
            name: The export function name.
            args: Arguments as WasmtimeVal values.

        Raises:
            Error: If the function is not found or the call fails/traps.
        """
        var func = self.get_func(name)
        func_call_0(self._store.context(), func, args)

    fn call_i32(self, name: String, args: List[WasmtimeVal]) raises -> Int32:
        """Call an exported WASM function and return a single i32 result.

        Args:
            name: The export function name.
            args: Arguments as WasmtimeVal values.

        Returns:
            The i32 result.

        Raises:
            Error: If the function is not found or the call fails/traps.
        """
        var func = self.get_func(name)
        return func_call_i32(self._store.context(), func, args)

    fn call_i64(self, name: String, args: List[WasmtimeVal]) raises -> Int64:
        """Call an exported WASM function and return a single i64 result.

        Args:
            name: The export function name.
            args: Arguments as WasmtimeVal values.

        Returns:
            The i64 result.

        Raises:
            Error: If the function is not found or the call fails/traps.
        """
        var func = self.get_func(name)
        return func_call_i64(self._store.context(), func, args)

    fn call_f32(self, name: String, args: List[WasmtimeVal]) raises -> Float32:
        """Call an exported WASM function and return a single f32 result.

        Args:
            name: The export function name.
            args: Arguments as WasmtimeVal values.

        Returns:
            The f32 result.

        Raises:
            Error: If the function is not found or the call fails/traps.
        """
        var func = self.get_func(name)
        return func_call_f32(self._store.context(), func, args)

    fn call_f64(self, name: String, args: List[WasmtimeVal]) raises -> Float64:
        """Call an exported WASM function and return a single f64 result.

        Args:
            name: The export function name.
            args: Arguments as WasmtimeVal values.

        Returns:
            The f64 result.

        Raises:
            Error: If the function is not found or the call fails/traps.
        """
        var func = self.get_func(name)
        return func_call_f64(self._store.context(), func, args)


# ---------------------------------------------------------------------------
# Convenience: build argument lists
# ---------------------------------------------------------------------------


fn args_i32(a: Int32) -> List[WasmtimeVal]:
    """Build a single-i32 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i32(a))
    return v


fn args_i32_i32(a: Int32, b: Int32) -> List[WasmtimeVal]:
    """Build a two-i32 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i32(b))
    return v


fn args_i64(a: Int64) -> List[WasmtimeVal]:
    """Build a single-i64 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(a))
    return v


fn args_i64_i64(a: Int64, b: Int64) -> List[WasmtimeVal]:
    """Build a two-i64 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(a))
    v.append(WasmtimeVal.from_i64(b))
    return v


fn args_f32(a: Float32) -> List[WasmtimeVal]:
    """Build a single-f32 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_f32(a))
    return v


fn args_f32_f32(a: Float32, b: Float32) -> List[WasmtimeVal]:
    """Build a two-f32 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_f32(a))
    v.append(WasmtimeVal.from_f32(b))
    return v


fn args_f64(a: Float64) -> List[WasmtimeVal]:
    """Build a single-f64 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_f64(a))
    return v


fn args_f64_f64(a: Float64, b: Float64) -> List[WasmtimeVal]:
    """Build a two-f64 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_f64(a))
    v.append(WasmtimeVal.from_f64(b))
    return v


fn args_i32_i32_i32(a: Int32, b: Int32, c: Int32) -> List[WasmtimeVal]:
    """Build a three-i32 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i32(b))
    v.append(WasmtimeVal.from_i32(c))
    return v


fn args_f64_f64_f64(a: Float64, b: Float64, c: Float64) -> List[WasmtimeVal]:
    """Build a three-f64 argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_f64(a))
    v.append(WasmtimeVal.from_f64(b))
    v.append(WasmtimeVal.from_f64(c))
    return v


fn args_ptr(ptr: Int) -> List[WasmtimeVal]:
    """Build a single-pointer (i64) argument list from an Int address."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    return v


fn args_ptr_ptr(a: Int, b: Int) -> List[WasmtimeVal]:
    """Build a two-pointer (i64, i64) argument list from Int addresses."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(a)))
    v.append(WasmtimeVal.from_i64(Int64(b)))
    return v


fn args_ptr_i32(ptr: Int, val: Int32) -> List[WasmtimeVal]:
    """Build a (ptr, i32) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(val))
    return v


fn args_ptr_i32_i32(ptr: Int, a: Int32, b: Int32) -> List[WasmtimeVal]:
    """Build a (ptr, i32, i32) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i32(b))
    return v


fn args_ptr_i32_i32_i32(
    ptr: Int, a: Int32, b: Int32, c: Int32
) -> List[WasmtimeVal]:
    """Build a (ptr, i32, i32, i32) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i32(b))
    v.append(WasmtimeVal.from_i32(c))
    return v


fn args_ptr_i32_i32_i32_ptr(
    ptr: Int, a: Int32, b: Int32, c: Int32, ptr2: Int
) -> List[WasmtimeVal]:
    """Build a (ptr, i32, i32, i32, ptr) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i32(b))
    v.append(WasmtimeVal.from_i32(c))
    v.append(WasmtimeVal.from_i64(Int64(ptr2)))
    return v


fn args_ptr_i32_ptr(ptr: Int, val: Int32, ptr2: Int) -> List[WasmtimeVal]:
    """Build a (ptr, i32, ptr) argument list — i64, i32, i64."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(val))
    v.append(WasmtimeVal.from_i64(Int64(ptr2)))
    return v


fn args_ptr_i64_ptr(a: Int, b: Int64, c: Int) -> List[WasmtimeVal]:
    """Build a (ptr, i64, ptr) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(a)))
    v.append(WasmtimeVal.from_i64(b))
    v.append(WasmtimeVal.from_i64(Int64(c)))
    return v


fn args_ptr_ptr_ptr(a: Int, b: Int, c: Int) -> List[WasmtimeVal]:
    """Build a three-pointer (i64, i64, i64) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(a)))
    v.append(WasmtimeVal.from_i64(Int64(b)))
    v.append(WasmtimeVal.from_i64(Int64(c)))
    return v


fn args_ptr_ptr_i32(a: Int, b: Int, c: Int32) -> List[WasmtimeVal]:
    """Build a (ptr, ptr, i32) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(a)))
    v.append(WasmtimeVal.from_i64(Int64(b)))
    v.append(WasmtimeVal.from_i32(c))
    return v


fn args_ptr_ptr_ptr_ptr(a: Int, b: Int, c: Int, d: Int) -> List[WasmtimeVal]:
    """Build a four-pointer (i64, i64, i64, i64) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(a)))
    v.append(WasmtimeVal.from_i64(Int64(b)))
    v.append(WasmtimeVal.from_i64(Int64(c)))
    v.append(WasmtimeVal.from_i64(Int64(d)))
    return v


fn args_ptr_ptr_ptr_ptr_i32(
    a: Int, b: Int, c: Int, d: Int, e: Int32
) -> List[WasmtimeVal]:
    """Build a (ptr, ptr, ptr, ptr, i32) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(a)))
    v.append(WasmtimeVal.from_i64(Int64(b)))
    v.append(WasmtimeVal.from_i64(Int64(c)))
    v.append(WasmtimeVal.from_i64(Int64(d)))
    v.append(WasmtimeVal.from_i32(e))
    return v


fn args_ptr_ptr_ptr_ptr_i32_i32(
    a: Int, b: Int, c: Int, d: Int, e: Int32, f: Int32
) -> List[WasmtimeVal]:
    """Build a (ptr, ptr, ptr, ptr, i32, i32) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(a)))
    v.append(WasmtimeVal.from_i64(Int64(b)))
    v.append(WasmtimeVal.from_i64(Int64(c)))
    v.append(WasmtimeVal.from_i64(Int64(d)))
    v.append(WasmtimeVal.from_i32(e))
    v.append(WasmtimeVal.from_i32(f))
    return v


fn args_ptr_i32_i32_ptr(
    ptr: Int, a: Int32, b: Int32, ptr2: Int
) -> List[WasmtimeVal]:
    """Build a (ptr, i32, i32, ptr) argument list — i64, i32, i32, i64."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i32(b))
    v.append(WasmtimeVal.from_i64(Int64(ptr2)))
    return v


fn args_ptr_i32_i32_i32_i32(
    ptr: Int, a: Int32, b: Int32, c: Int32, d: Int32
) -> List[WasmtimeVal]:
    """Build a (ptr, i32, i32, i32, i32) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i32(b))
    v.append(WasmtimeVal.from_i32(c))
    v.append(WasmtimeVal.from_i32(d))
    return v


fn args_ptr_i32_i32_i32_ptr_ptr(
    ptr: Int, a: Int32, b: Int32, c: Int32, ptr2: Int, ptr3: Int
) -> List[WasmtimeVal]:
    """Build a (ptr, i32, i32, i32, ptr, ptr) argument list."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i32(b))
    v.append(WasmtimeVal.from_i32(c))
    v.append(WasmtimeVal.from_i64(Int64(ptr2)))
    v.append(WasmtimeVal.from_i64(Int64(ptr3)))
    return v


fn args_ptr_i32_ptr_ptr(
    ptr: Int, val: Int32, ptr2: Int, ptr3: Int
) -> List[WasmtimeVal]:
    """Build a (ptr, i32, ptr, ptr) argument list — i64, i32, i64, i64."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(val))
    v.append(WasmtimeVal.from_i64(Int64(ptr2)))
    v.append(WasmtimeVal.from_i64(Int64(ptr3)))
    return v


fn args_ptr_i32_ptr_i32(
    ptr: Int, a: Int32, ptr2: Int, b: Int32
) -> List[WasmtimeVal]:
    """Build a (ptr, i32, ptr, i32) argument list — i64, i32, i64, i32."""
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i64(Int64(ptr2)))
    v.append(WasmtimeVal.from_i32(b))
    return v


fn args_ptr_i32_ptr_i32_i32(
    ptr: Int, a: Int32, ptr2: Int, b: Int32, c: Int32
) -> List[WasmtimeVal]:
    """Build a (ptr, i32, ptr, i32, i32) argument list — i64, i32, i64, i32, i32.
    """
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i64(Int64(ptr2)))
    v.append(WasmtimeVal.from_i32(b))
    v.append(WasmtimeVal.from_i32(c))
    return v


fn args_ptr_i32_ptr_ptr_i32(
    ptr: Int, a: Int32, ptr2: Int, ptr3: Int, b: Int32
) -> List[WasmtimeVal]:
    """Build a (ptr, i32, ptr, ptr, i32) argument list — i64, i32, i64, i64, i32.
    """
    var v = List[WasmtimeVal]()
    v.append(WasmtimeVal.from_i64(Int64(ptr)))
    v.append(WasmtimeVal.from_i32(a))
    v.append(WasmtimeVal.from_i64(Int64(ptr2)))
    v.append(WasmtimeVal.from_i64(Int64(ptr3)))
    v.append(WasmtimeVal.from_i32(b))
    return v


fn no_args() -> List[WasmtimeVal]:
    """Build an empty argument list."""
    return List[WasmtimeVal]()


# ---------------------------------------------------------------------------
# Singleton instance — reuse a single instance across all Mojo test
# invocations within a process, matching the Python harness behavior.
# ---------------------------------------------------------------------------

var _cached_instance: UnsafePointer[WasmInstance] = UnsafePointer[
    WasmInstance
]()
var _cached_initialized: Bool = False


fn get_instance() raises -> UnsafePointer[WasmInstance]:
    """Return a pointer to a cached WasmInstance, creating it on first call.

    The instance is allocated on the heap and lives for the duration of
    the process.  All test functions share the same instance.

    Returns:
        UnsafePointer to the singleton WasmInstance.

    Raises:
        Error: If the WASM binary cannot be loaded or instantiated.
    """
    if not _cached_initialized:
        _cached_instance = UnsafePointer[WasmInstance].alloc(1)
        _cached_instance[] = WasmInstance(WASM_PATH)
        _cached_initialized = True
    return _cached_instance
