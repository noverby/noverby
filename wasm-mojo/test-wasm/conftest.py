"""
WASM test harness using wasmtime-py.

Provides fixtures for loading the Mojo WASM binary and interacting with
its exported functions, including string struct read/write operations
that mirror the TypeScript runtime/strings.ts and runtime/memory.ts.

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

import ctypes
import math
import struct
from pathlib import Path

import pytest
import wasmtime

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

STRING_STRUCT_SIZE = 24
STRING_STRUCT_ALIGN = 8

SSO_FLAG = 0x8000_0000_0000_0000
SSO_LEN_MASK = 0x1F00_0000_0000_0000

WASM_PATH = Path(__file__).resolve().parent.parent / "build" / "out.wasm"


# ---------------------------------------------------------------------------
# Bump allocator (mirrors runtime/memory.ts)
# ---------------------------------------------------------------------------


class BumpAllocator:
    """Simple bump allocator operating on WASM linear memory."""

    def __init__(self, heap_base: int):
        self._ptr = heap_base

    def aligned_alloc(self, align: int, size: int) -> int:
        remainder = self._ptr % align
        if remainder != 0:
            self._ptr += align - remainder
        ptr = self._ptr
        self._ptr += size
        return ptr

    def aligned_free(self, _ptr: int) -> int:
        return 1


# ---------------------------------------------------------------------------
# WASM instance wrapper
# ---------------------------------------------------------------------------


class WasmInstance:
    """Wraps a wasmtime instance with helper methods mirroring the TS runtime."""

    def __init__(
        self,
        store: wasmtime.Store,
        instance: wasmtime.Instance,
        alloc: BumpAllocator,
    ):
        self._store = store
        self._instance = instance
        self._alloc = alloc

        # Cache the memory export
        mem = instance.exports(store).get("memory")
        assert isinstance(mem, wasmtime.Memory), "WASM module must export 'memory'"
        self._memory = mem

        # Captured stdout (populated by the write import)
        self.captured_stdout: list[str] = []

    # -- raw memory helpers ------------------------------------------------

    @property
    def store(self) -> wasmtime.Store:
        return self._store

    @property
    def memory(self) -> wasmtime.Memory:
        return self._memory

    def _mem_buf(self):
        """Return a mutable ctypes array over the full WASM linear memory."""
        ptr = self._memory.data_ptr(self._store)
        size = self._memory.data_len(self._store)
        addr = ctypes.cast(ptr, ctypes.c_void_p).value
        return (ctypes.c_ubyte * size).from_address(addr)

    def read_bytes(self, ptr: int, length: int) -> bytes:
        buf = self._mem_buf()
        return bytes(buf[ptr : ptr + length])

    def write_bytes(self, ptr: int, data: bytes) -> None:
        buf = self._mem_buf()
        for i, b in enumerate(data):
            buf[ptr + i] = b

    def read_i64_le(self, ptr: int) -> int:
        raw = self.read_bytes(ptr, 8)
        return struct.unpack("<q", raw)[0]

    def read_u64_le(self, ptr: int) -> int:
        raw = self.read_bytes(ptr, 8)
        return struct.unpack("<Q", raw)[0]

    def write_i64_le(self, ptr: int, value: int) -> None:
        self.write_bytes(ptr, struct.pack("<q", value))

    # -- bump allocator wrappers ------------------------------------------

    def aligned_alloc(self, align: int, size: int) -> int:
        return self._alloc.aligned_alloc(align, size)

    # -- string struct operations (mirrors runtime/strings.ts) ------------

    def write_string_struct(self, s: str) -> int:
        """Allocate a Mojo String struct in WASM memory, populated with *s*."""
        encoded = s.encode("utf-8")
        data_len = len(encoded)

        # Allocate data buffer (with null terminator)
        data_ptr = self._alloc.aligned_alloc(1, data_len + 1)
        self.write_bytes(data_ptr, encoded + b"\x00")

        # Allocate 24-byte String struct
        struct_ptr = self._alloc.aligned_alloc(STRING_STRUCT_ALIGN, STRING_STRUCT_SIZE)
        self.write_i64_le(struct_ptr, data_ptr)  # data_ptr
        self.write_i64_le(struct_ptr + 8, data_len)  # len
        self.write_i64_le(struct_ptr + 16, data_len + 1)  # capacity

        return struct_ptr

    def alloc_string_struct(self) -> int:
        """Allocate a zero-initialized 24-byte Mojo String struct."""
        struct_ptr = self._alloc.aligned_alloc(STRING_STRUCT_ALIGN, STRING_STRUCT_SIZE)
        self.write_bytes(struct_ptr, b"\x00" * STRING_STRUCT_SIZE)
        return struct_ptr

    def read_string_struct(self, struct_ptr: int) -> str:
        """Read a Mojo String struct back into a Python str."""
        capacity = self.read_u64_le(struct_ptr + 16)

        if capacity & SSO_FLAG:
            # SSO: data inline at struct_ptr, length encoded in capacity
            data_ptr = struct_ptr
            length = (capacity & SSO_LEN_MASK) >> 56
        else:
            data_ptr = self.read_i64_le(struct_ptr)
            length = self.read_i64_le(struct_ptr + 8)

        if length <= 0:
            return ""

        raw = self.read_bytes(data_ptr, length)
        return raw.decode("utf-8")

    # -- calling WASM exports ---------------------------------------------

    def call(self, name: str, *args):
        """Call an exported WASM function by name."""
        fn = self._instance.exports(self._store).get(name)
        assert fn is not None, f"WASM export '{name}' not found"
        assert isinstance(fn, wasmtime.Func), f"'{name}' is not a function"
        result = fn(self._store, *args)
        return result

    def __getattr__(self, name: str):
        """Allow ``w.add_int32(1, 2)`` as shorthand for ``w.call('add_int32', 1, 2)``."""

        def _call(*args):
            return self.call(name, *args)

        _call.__name__ = name
        return _call


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _to_signed_64(v: int) -> int:
    """Convert an unsigned 64-bit int to signed (for struct packing)."""
    if v >= (1 << 63):
        v -= 1 << 64
    return v


def _f32(x: float) -> float:
    """Truncate to float32 precision."""
    return struct.unpack("f", struct.pack("f", x))[0]


# ---------------------------------------------------------------------------
# wasmtime ValType shortcuts
# ---------------------------------------------------------------------------

_I32 = wasmtime.ValType.i32()
_I64 = wasmtime.ValType.i64()
_F32 = wasmtime.ValType.f32()
_F64 = wasmtime.ValType.f64()


# ---------------------------------------------------------------------------
# Instance creation
# ---------------------------------------------------------------------------


def _create_instance() -> WasmInstance:
    """Load the WASM module and return a WasmInstance."""
    assert WASM_PATH.exists(), (
        f"WASM binary not found at {WASM_PATH}. Run `just build` first."
    )

    engine = wasmtime.Engine()
    store = wasmtime.Store(engine)
    linker = wasmtime.Linker(engine)

    # -- Shared state used by import callbacks -----------------------------

    alloc = BumpAllocator(0)  # updated after instantiation
    captured_stdout: list[str] = []
    _inst_wrapper: list[WasmInstance | None] = [None]

    # NOTE: The WASM module defines its own memory (not imported).
    # We do NOT call linker.define for "env" "memory".

    # ======================================================================
    # Import definitions — signatures MUST match the WASM binary exactly.
    # ======================================================================

    # -- func[0] KGEN_CompilerRT_AlignedAlloc: (i64, i64) -> i64 ----------

    def aligned_alloc_fn(align, size):
        return alloc.aligned_alloc(align, size)

    linker.define(
        store,
        "env",
        "KGEN_CompilerRT_AlignedAlloc",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I64, _I64], [_I64]),
            aligned_alloc_fn,
        ),
    )

    # -- func[1] KGEN_CompilerRT_AlignedFree: (i64) -> nil -----------------

    def aligned_free_fn(_ptr):
        pass  # bump allocator never reclaims

    linker.define(
        store,
        "env",
        "KGEN_CompilerRT_AlignedFree",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I64], []),
            aligned_free_fn,
        ),
    )

    # -- func[2] fmaf: (f32, f32, f32) -> f32 -----------------------------

    def fmaf_fn(x, y, z):
        return _f32(x * y + z)

    linker.define(
        store,
        "env",
        "fmaf",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_F32, _F32, _F32], [_F32]),
            fmaf_fn,
        ),
    )

    # -- func[3] fminf: (f32, f32) -> f32 ---------------------------------

    def fminf_fn(x, y):
        return y if x > y else x

    linker.define(
        store,
        "env",
        "fminf",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_F32, _F32], [_F32]),
            fminf_fn,
        ),
    )

    # -- func[4] fmaxf: (f32, f32) -> f32 ---------------------------------

    def fmaxf_fn(x, y):
        return x if x > y else y

    linker.define(
        store,
        "env",
        "fmaxf",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_F32, _F32], [_F32]),
            fmaxf_fn,
        ),
    )

    # -- func[5] fma: (f64, f64, f64) -> f64 ------------------------------

    def fma_fn(x, y, z):
        return x * y + z

    linker.define(
        store,
        "env",
        "fma",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_F64, _F64, _F64], [_F64]),
            fma_fn,
        ),
    )

    # -- func[6] fmin: (f64, f64) -> f64 ----------------------------------

    def fmin_fn(x, y):
        return y if x > y else x

    linker.define(
        store,
        "env",
        "fmin",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_F64, _F64], [_F64]),
            fmin_fn,
        ),
    )

    # -- func[7] fmax: (f64, f64) -> f64 ----------------------------------

    def fmax_fn(x, y):
        return x if x > y else y

    linker.define(
        store,
        "env",
        "fmax",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_F64, _F64], [_F64]),
            fmax_fn,
        ),
    )

    # -- func[8] KGEN_CompilerRT_GetStackTrace: (i64, i64) -> i64 ---------

    def get_stack_trace(_buf, _max_frames):
        return 0

    linker.define(
        store,
        "env",
        "KGEN_CompilerRT_GetStackTrace",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I64, _I64], [_I64]),
            get_stack_trace,
        ),
    )

    # -- func[9] free: (i64) -> nil ----------------------------------------

    def free_fn(_ptr):
        pass  # bump allocator never reclaims

    linker.define(
        store,
        "env",
        "free",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I64], []),
            free_fn,
        ),
    )

    # -- func[10] dup: (i32) -> i32 ----------------------------------------

    def dup_fn(_fd):
        return 1

    linker.define(
        store,
        "env",
        "dup",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I32], [_I32]),
            dup_fn,
        ),
    )

    # -- func[11] fdopen: (i32, i64) -> i64 --------------------------------

    def fdopen_fn(_fd, _mode):
        return 1

    linker.define(
        store,
        "env",
        "fdopen",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I32, _I64], [_I64]),
            fdopen_fn,
        ),
    )

    # -- func[12] fflush: (i64) -> i32 ------------------------------------

    def fflush_fn(_stream):
        return 1

    linker.define(
        store,
        "env",
        "fflush",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I64], [_I32]),
            fflush_fn,
        ),
    )

    # -- func[13] fclose: (i64) -> i32 ------------------------------------

    def fclose_fn(_stream):
        return 1

    linker.define(
        store,
        "env",
        "fclose",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I64], [_I32]),
            fclose_fn,
        ),
    )

    # -- func[14] KGEN_CompilerRT_fprintf: (i64, i64, i64) -> i32 ---------

    def fprintf_fn(_stream, _fmt, *_args):
        return 0

    linker.define(
        store,
        "env",
        "KGEN_CompilerRT_fprintf",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I64, _I64, _I64], [_I32]),
            fprintf_fn,
        ),
    )

    # -- func[15] write: (i64, i64, i64) -> i32 ---------------------------

    def write_fn(fd, ptr, length):
        if length == 0:
            return 0
        try:
            inst = _inst_wrapper[0]
            if inst is None:
                return -1
            data = inst.read_bytes(ptr, length)
            text = data.decode("utf-8", errors="replace")
            if fd == 1:
                captured_stdout.append(text)
                return length
            elif fd == 2:
                import sys

                sys.stderr.write(text)
                return length
        except Exception:
            pass
        return -1

    linker.define(
        store,
        "env",
        "write",
        wasmtime.Func(
            store,
            wasmtime.FuncType([_I64, _I64, _I64], [_I32]),
            write_fn,
        ),
    )

    # ======================================================================
    # Instantiate
    # ======================================================================

    wasm_bytes = WASM_PATH.read_bytes()
    module = wasmtime.Module(engine, wasm_bytes)

    instance = linker.instantiate(store, module)

    # Read __heap_base global to initialise the bump allocator
    heap_base_global = instance.exports(store).get("__heap_base")
    assert isinstance(heap_base_global, wasmtime.Global), "__heap_base not found"
    heap_base = heap_base_global.value(store)
    alloc._ptr = heap_base

    wrapper = WasmInstance(store, instance, alloc)
    wrapper.captured_stdout = captured_stdout
    _inst_wrapper[0] = wrapper

    return wrapper


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


# Session-scoped fixture: a single WASM instance shared across ALL tests
# in the session. This is efficient since instantiation is expensive.
@pytest.fixture(scope="session")
def w() -> WasmInstance:
    """Session-scoped WASM instance fixture."""
    return _create_instance()
