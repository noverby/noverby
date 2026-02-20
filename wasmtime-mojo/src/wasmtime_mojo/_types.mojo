"""C struct type definitions for the wasmtime C API.

These structs mirror the exact memory layout of their C counterparts
so they can be passed directly through FFI boundaries.

Layout reference (64-bit Linux):
  wasmtime_val_t:    32 bytes (kind:1 + pad:7 + union:24)
  wasmtime_extern_t: 32 bytes (kind:1 + pad:7 + union:24)
  wasmtime_func_t:   16 bytes (store_id:8 + __private:8)
  wasmtime_instance_t: 16 bytes (store_id:8 + __private:8)
  wasmtime_global_t: 24 bytes (store_id:8 + 3×u32:12 + pad:4)
  wasmtime_memory_t: 24 bytes (store_id:8 + u32:4 + pad:4 + u32:4 + pad:4)
  wasm_valtype_vec_t: 16 bytes (size:8 + data:8)
  wasm_byte_vec_t:   16 bytes (size:8 + data:8)
"""

from memory import UnsafePointer, memset_zero, memcpy
from sys.info import sizeof

# ---------------------------------------------------------------------------
# Wasmtime val kind constants (wasmtime_valkind_t)
# ---------------------------------------------------------------------------

alias WASMTIME_I32: UInt8 = 0
alias WASMTIME_I64: UInt8 = 1
alias WASMTIME_F32: UInt8 = 2
alias WASMTIME_F64: UInt8 = 3
alias WASMTIME_V128: UInt8 = 4
alias WASMTIME_FUNCREF: UInt8 = 5
alias WASMTIME_EXTERNREF: UInt8 = 6

# ---------------------------------------------------------------------------
# WASM val kind constants (wasm_valkind_t) — used by wasm_valtype_new
# ---------------------------------------------------------------------------

alias WASM_I32: UInt8 = 0
alias WASM_I64: UInt8 = 1
alias WASM_F32: UInt8 = 2
alias WASM_F64: UInt8 = 3
alias WASM_EXTERNREF: UInt8 = 128
alias WASM_FUNCREF: UInt8 = 129

# ---------------------------------------------------------------------------
# Wasmtime extern kind constants (wasmtime_extern_kind_t)
# ---------------------------------------------------------------------------

alias WASMTIME_EXTERN_FUNC: UInt8 = 0
alias WASMTIME_EXTERN_GLOBAL: UInt8 = 1
alias WASMTIME_EXTERN_TABLE: UInt8 = 2
alias WASMTIME_EXTERN_MEMORY: UInt8 = 3
alias WASMTIME_EXTERN_SHAREDMEMORY: UInt8 = 4

# ---------------------------------------------------------------------------
# Sizes of C structs (bytes)
# ---------------------------------------------------------------------------

alias WASMTIME_VAL_SIZE = 32
alias WASMTIME_EXTERN_SIZE = 32
alias WASMTIME_FUNC_SIZE = 16
alias WASMTIME_INSTANCE_SIZE = 16
alias WASMTIME_GLOBAL_SIZE = 24
alias WASMTIME_MEMORY_SIZE = 24
alias WASMTIME_TABLE_SIZE = 24
alias WASM_VALTYPE_VEC_SIZE = 16
alias WASM_BYTE_VEC_SIZE = 16


# ---------------------------------------------------------------------------
# Opaque pointer aliases — these wrap C types we never inspect directly
# ---------------------------------------------------------------------------

alias EnginePtr = UnsafePointer[NoneType]
alias StorePtr = UnsafePointer[NoneType]
alias ContextPtr = UnsafePointer[NoneType]
alias ModulePtr = UnsafePointer[NoneType]
alias LinkerPtr = UnsafePointer[NoneType]
alias ErrorPtr = UnsafePointer[NoneType]
alias TrapPtr = UnsafePointer[NoneType]
alias FuncTypePtr = UnsafePointer[NoneType]
alias ValTypePtr = UnsafePointer[NoneType]
alias CallerPtr = UnsafePointer[NoneType]
alias GlobalTypePtr = UnsafePointer[NoneType]
alias ExternTypePtr = UnsafePointer[NoneType]


# ---------------------------------------------------------------------------
# wasmtime_val_t — 32-byte tagged union for WASM values
#
# C layout:
#   struct wasmtime_val_t {
#       uint8_t kind;          // offset 0
#       // 7 bytes padding
#       wasmtime_valunion_t of; // offset 8, 24 bytes
#   };
#
# The union's largest members are anyref/externref at 24 bytes.
# For our purposes we only need i32/i64/f32/f64 access.
# ---------------------------------------------------------------------------


@register_passable("trivial")
struct WasmtimeVal:
    """Mirrors wasmtime_val_t (32 bytes)."""

    var _storage: SIMD[DType.uint8, WASMTIME_VAL_SIZE]

    @always_inline
    fn __init__(out self):
        self._storage = SIMD[DType.uint8, WASMTIME_VAL_SIZE](0)

    # -- Kind accessor (offset 0) --

    @always_inline
    fn get_kind(self) -> UInt8:
        return self._storage[0]

    @always_inline
    fn set_kind(mut self, kind: UInt8):
        self._storage[0] = kind

    # -- i32 accessor (offset 8, 4 bytes, little-endian) --

    @always_inline
    fn get_i32(self) -> Int32:
        var ptr = UnsafePointer(to=self._storage).bitcast[UInt8]()
        return (ptr + 8).bitcast[Int32]()[]

    @always_inline
    fn set_i32(mut self, value: Int32):
        var ptr = UnsafePointer(to=self._storage).bitcast[UInt8]()
        (ptr + 8).bitcast[Int32]()[] = value

    # -- i64 accessor (offset 8, 8 bytes) --

    @always_inline
    fn get_i64(self) -> Int64:
        var ptr = UnsafePointer(to=self._storage).bitcast[UInt8]()
        return (ptr + 8).bitcast[Int64]()[]

    @always_inline
    fn set_i64(mut self, value: Int64):
        var ptr = UnsafePointer(to=self._storage).bitcast[UInt8]()
        (ptr + 8).bitcast[Int64]()[] = value

    # -- f32 accessor (offset 8, 4 bytes) --

    @always_inline
    fn get_f32(self) -> Float32:
        var ptr = UnsafePointer(to=self._storage).bitcast[UInt8]()
        return (ptr + 8).bitcast[Float32]()[]

    @always_inline
    fn set_f32(mut self, value: Float32):
        var ptr = UnsafePointer(to=self._storage).bitcast[UInt8]()
        (ptr + 8).bitcast[Float32]()[] = value

    # -- f64 accessor (offset 8, 8 bytes) --

    @always_inline
    fn get_f64(self) -> Float64:
        var ptr = UnsafePointer(to=self._storage).bitcast[UInt8]()
        return (ptr + 8).bitcast[Float64]()[]

    @always_inline
    fn set_f64(mut self, value: Float64):
        var ptr = UnsafePointer(to=self._storage).bitcast[UInt8]()
        (ptr + 8).bitcast[Float64]()[] = value

    # -- Convenience constructors --

    @staticmethod
    fn from_i32(value: Int32) -> WasmtimeVal:
        var v = WasmtimeVal()
        v.set_kind(WASMTIME_I32)
        v.set_i32(value)
        return v

    @staticmethod
    fn from_i64(value: Int64) -> WasmtimeVal:
        var v = WasmtimeVal()
        v.set_kind(WASMTIME_I64)
        v.set_i64(value)
        return v

    @staticmethod
    fn from_f32(value: Float32) -> WasmtimeVal:
        var v = WasmtimeVal()
        v.set_kind(WASMTIME_F32)
        v.set_f32(value)
        return v

    @staticmethod
    fn from_f64(value: Float64) -> WasmtimeVal:
        var v = WasmtimeVal()
        v.set_kind(WASMTIME_F64)
        v.set_f64(value)
        return v


# ---------------------------------------------------------------------------
# wasmtime_func_t — 16 bytes
#
# C layout:
#   struct wasmtime_func_t {
#       uint64_t store_id;   // offset 0
#       size_t   __private;  // offset 8
#   };
# ---------------------------------------------------------------------------


@register_passable("trivial")
struct WasmtimeFunc:
    """Mirrors wasmtime_func_t (16 bytes)."""

    var _storage: SIMD[DType.uint8, WASMTIME_FUNC_SIZE]

    @always_inline
    fn __init__(out self):
        self._storage = SIMD[DType.uint8, WASMTIME_FUNC_SIZE](0)


# ---------------------------------------------------------------------------
# wasmtime_instance_t — 16 bytes
#
# C layout:
#   struct wasmtime_instance_t {
#       uint64_t store_id;   // offset 0
#       size_t   __private;  // offset 8
#   };
# ---------------------------------------------------------------------------


@register_passable("trivial")
struct WasmtimeInstance:
    """Mirrors wasmtime_instance_t (16 bytes)."""

    var _storage: SIMD[DType.uint8, WASMTIME_INSTANCE_SIZE]

    @always_inline
    fn __init__(out self):
        self._storage = SIMD[DType.uint8, WASMTIME_INSTANCE_SIZE](0)


# ---------------------------------------------------------------------------
# wasmtime_global_t — 24 bytes
#
# C layout:
#   struct wasmtime_global_t {
#       uint64_t store_id;      // offset 0
#       uint32_t __private1;    // offset 8
#       uint32_t __private2;    // offset 12
#       uint32_t __private3;    // offset 16
#       // 4 bytes padding to align to 8
#   };
# ---------------------------------------------------------------------------


@register_passable("trivial")
struct WasmtimeGlobal:
    """Mirrors wasmtime_global_t (24 bytes)."""

    var _storage: SIMD[DType.uint8, WASMTIME_GLOBAL_SIZE]

    @always_inline
    fn __init__(out self):
        self._storage = SIMD[DType.uint8, WASMTIME_GLOBAL_SIZE](0)


# ---------------------------------------------------------------------------
# wasmtime_memory_t — 24 bytes
#
# C layout (from Python):
#   struct _anon { uint64_t store_id; uint32_t __private1; };
#   struct wasmtime_memory_t { _anon; uint32_t __private2; };
#
# Total: 8 + 4 + (4 pad) + 4 + (4 pad) = 24 bytes
# ---------------------------------------------------------------------------


@register_passable("trivial")
struct WasmtimeMemory:
    """Mirrors wasmtime_memory_t (24 bytes)."""

    var _storage: SIMD[DType.uint8, WASMTIME_MEMORY_SIZE]

    @always_inline
    fn __init__(out self):
        self._storage = SIMD[DType.uint8, WASMTIME_MEMORY_SIZE](0)


# ---------------------------------------------------------------------------
# wasmtime_extern_t — 32-byte tagged union for exported items
#
# C layout:
#   struct wasmtime_extern_t {
#       uint8_t kind;               // offset 0
#       // 7 bytes padding
#       wasmtime_extern_union_t of; // offset 8, 24 bytes
#   };
#
# The union holds func/global/table/memory/sharedmemory.
# ---------------------------------------------------------------------------


@register_passable("trivial")
struct WasmtimeExtern:
    """Mirrors wasmtime_extern_t (32 bytes)."""

    var _storage: SIMD[DType.uint8, WASMTIME_EXTERN_SIZE]

    @always_inline
    fn __init__(out self):
        self._storage = SIMD[DType.uint8, WASMTIME_EXTERN_SIZE](0)

    @always_inline
    fn get_kind(self) -> UInt8:
        return self._storage[0]

    @always_inline
    fn get_func(self) -> WasmtimeFunc:
        """Extract the func from the union (valid when kind == WASMTIME_EXTERN_FUNC).
        """
        var f = WasmtimeFunc()
        var src = UnsafePointer(to=self._storage).bitcast[UInt8]() + 8
        var dst = UnsafePointer(to=f._storage).bitcast[UInt8]()
        memcpy(dst, src, WASMTIME_FUNC_SIZE)
        return f

    @always_inline
    fn get_global(self) -> WasmtimeGlobal:
        """Extract the global from the union (valid when kind == WASMTIME_EXTERN_GLOBAL).
        """
        var g = WasmtimeGlobal()
        var src = UnsafePointer(to=self._storage).bitcast[UInt8]() + 8
        var dst = UnsafePointer(to=g._storage).bitcast[UInt8]()
        memcpy(dst, src, WASMTIME_GLOBAL_SIZE)
        return g

    @always_inline
    fn get_memory(self) -> WasmtimeMemory:
        """Extract the memory from the union (valid when kind == WASMTIME_EXTERN_MEMORY).
        """
        var m = WasmtimeMemory()
        var src = UnsafePointer(to=self._storage).bitcast[UInt8]() + 8
        var dst = UnsafePointer(to=m._storage).bitcast[UInt8]()
        memcpy(dst, src, WASMTIME_MEMORY_SIZE)
        return m


# ---------------------------------------------------------------------------
# wasm_byte_vec_t — 16 bytes
#
# C layout:
#   struct wasm_byte_vec_t {
#       size_t size;
#       wasm_byte_t *data;  // uint8_t*
#   };
# ---------------------------------------------------------------------------


@register_passable("trivial")
struct WasmByteVec:
    """Mirrors wasm_byte_vec_t (16 bytes)."""

    var size: Int
    var data: UnsafePointer[UInt8]

    @always_inline
    fn __init__(out self):
        self.size = 0
        self.data = UnsafePointer[UInt8]()


# ---------------------------------------------------------------------------
# wasm_valtype_vec_t — 16 bytes
#
# C layout:
#   struct wasm_valtype_vec_t {
#       size_t size;
#       wasm_valtype_t **data;  // array of pointers
#   };
# ---------------------------------------------------------------------------


@register_passable("trivial")
struct WasmValtypeVec:
    """Mirrors wasm_valtype_vec_t (16 bytes)."""

    var size: Int
    var data: UnsafePointer[ValTypePtr]

    @always_inline
    fn __init__(out self):
        self.size = 0
        self.data = UnsafePointer[ValTypePtr]()


# ---------------------------------------------------------------------------
# Callback type aliases
#
# wasmtime_func_callback_t:
#   wasm_trap_t* (*)(
#       void *env,
#       wasmtime_caller_t *caller,
#       const wasmtime_val_t *args,
#       size_t nargs,
#       wasmtime_val_t *results,
#       size_t nresults
#   );
#
# We represent this as a raw function pointer type. The return value is
# a wasm_trap_t* where NULL means success.
# ---------------------------------------------------------------------------

alias WasmtimeCallback = fn (
    UnsafePointer[NoneType],  # env
    UnsafePointer[NoneType],  # caller
    UnsafePointer[WasmtimeVal],  # args
    Int,  # nargs
    UnsafePointer[WasmtimeVal],  # results
    Int,  # nresults
) -> UnsafePointer[NoneType]

# Finalizer callback: void (*)(void*)
alias FinalizerCallback = fn (UnsafePointer[NoneType]) -> None
