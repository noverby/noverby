from memory import UnsafePointer

# Tests for Phase 10.4 — AppShell (component abstraction).
#
# Validates:
#   - AppShell create/destroy lifecycle
#   - is_alive tracking
#   - Scope creation (root and child) via shell
#   - Signal creation, read, peek, write via shell
#   - begin_render / end_render scope lifecycle
#   - has_dirty / collect_dirty / next_dirty scheduler integration
#   - dispatch_event via shell
#   - mount() produces valid mutations
#   - diff() produces correct mutations on state change
#   - Pointer accessors (rt_ptr, store_ptr, eid_ptr)
#   - Double destroy safety

from testing import assert_equal, assert_true, assert_false
from wasm_harness import (
    WasmInstance,
    no_args,
    args_i32,
    args_ptr,
    args_ptr_i32,
    args_ptr_i32_i32,
    args_ptr_ptr,
    args_ptr_ptr_i32,
    args_ptr_ptr_i32_i32,
    args_ptr_ptr_i32_i32_i32,
    args_ptr_i32_ptr,
    args_ptr_i32_i32_i32_ptr,
)


fn _load() raises -> WasmInstance:
    return WasmInstance("build/out.wasm")


# ── AppShell lifecycle ───────────────────────────────────────────────────────


def test_shell_create_destroy(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    assert_true(shell != 0, "shell pointer should be non-zero")
    assert_equal(w[].call_i32("shell_is_alive", args_ptr(shell)), 1)
    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_is_alive_after_create(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    assert_equal(w[].call_i32("shell_is_alive", args_ptr(shell)), 1)
    w[].call_void("shell_destroy", args_ptr(shell))


# ── Pointer accessors ────────────────────────────────────────────────────────


def test_shell_rt_ptr_non_zero(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var rt = Int(w[].call_i64("shell_rt_ptr", args_ptr(shell)))
    assert_true(rt != 0, "runtime pointer should be non-zero")
    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_store_ptr_non_zero(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var store = Int(w[].call_i64("shell_store_ptr", args_ptr(shell)))
    assert_true(store != 0, "store pointer should be non-zero")
    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_eid_ptr_non_zero(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var eid = Int(w[].call_i64("shell_eid_ptr", args_ptr(shell)))
    assert_true(eid != 0, "eid_alloc pointer should be non-zero")
    w[].call_void("shell_destroy", args_ptr(shell))


# ── Scope creation ───────────────────────────────────────────────────────────


def test_shell_create_root_scope(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var s0 = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    assert_true(s0 >= 0, "root scope id should be non-negative")

    # Verify the scope exists in the underlying runtime
    var rt = Int(w[].call_i64("shell_rt_ptr", args_ptr(shell)))
    assert_equal(w[].call_i32("scope_contains", args_ptr_i32(rt, s0)), 1)
    assert_equal(w[].call_i32("scope_height", args_ptr_i32(rt, s0)), 0)

    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_create_child_scope(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var parent = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var child = w[].call_i32(
        "shell_create_child_scope", args_ptr_i32(shell, parent)
    )
    assert_true(child != parent, "child id should differ from parent")

    # Verify parent/child relationship in the runtime
    var rt = Int(w[].call_i64("shell_rt_ptr", args_ptr(shell)))
    assert_equal(w[].call_i32("scope_height", args_ptr_i32(rt, child)), 1)
    assert_equal(w[].call_i32("scope_parent", args_ptr_i32(rt, child)), parent)

    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_create_multiple_root_scopes(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var s0 = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var s1 = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var s2 = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    assert_true(s0 != s1, "scopes should have unique ids")
    assert_true(s1 != s2, "scopes should have unique ids")
    assert_true(s0 != s2, "scopes should have unique ids")
    w[].call_void("shell_destroy", args_ptr(shell))


# ── Signal operations ────────────────────────────────────────────────────────


def test_shell_create_signal_i32(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var key = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 42))
    assert_true(key >= 0, "signal key should be non-negative")

    # Peek should return the initial value
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, key)), 42
    )
    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_write_and_peek_signal(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var key = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 0))

    w[].call_void("shell_write_signal_i32", args_ptr_i32_i32(shell, key, 99))
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, key)), 99
    )

    w[].call_void("shell_write_signal_i32", args_ptr_i32_i32(shell, key, -7))
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, key)), -7
    )

    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_read_signal_with_context(w: UnsafePointer[WasmInstance]):
    """read_signal subscribes the current scope to the signal."""
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var scope = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var sig = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 10))

    # Begin render to activate the scope as reactive context
    _ = w[].call_i32("shell_begin_render", args_ptr_i32(shell, scope))
    var val = w[].call_i32("shell_read_signal_i32", args_ptr_i32(shell, sig))
    assert_equal(val, 10)
    w[].call_void("shell_end_render", args_ptr_i32(shell, -1))

    # Writing should make the scope dirty (subscribed via read)
    w[].call_void("shell_write_signal_i32", args_ptr_i32_i32(shell, sig, 20))
    assert_equal(w[].call_i32("shell_has_dirty", args_ptr(shell)), 1)

    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_peek_does_not_subscribe(w: UnsafePointer[WasmInstance]):
    """peek_signal does NOT subscribe the scope."""
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var scope = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var sig = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 5))

    # Begin render but only peek (not read)
    _ = w[].call_i32("shell_begin_render", args_ptr_i32(shell, scope))
    var val = w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, sig))
    assert_equal(val, 5)
    w[].call_void("shell_end_render", args_ptr_i32(shell, -1))

    # Writing should NOT make the scope dirty (not subscribed)
    w[].call_void("shell_write_signal_i32", args_ptr_i32_i32(shell, sig, 99))
    assert_equal(w[].call_i32("shell_has_dirty", args_ptr(shell)), 0)

    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_multiple_signals(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var sig_a = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 1))
    var sig_b = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 2))
    var sig_c = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 3))

    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, sig_a)), 1
    )
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, sig_b)), 2
    )
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, sig_c)), 3
    )

    w[].call_void("shell_write_signal_i32", args_ptr_i32_i32(shell, sig_b, 200))
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, sig_b)), 200
    )
    # Others unchanged
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, sig_a)), 1
    )
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, sig_c)), 3
    )

    w[].call_void("shell_destroy", args_ptr(shell))


# ── Render lifecycle ─────────────────────────────────────────────────────────


def test_shell_begin_end_render(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var scope = w[].call_i32("shell_create_root_scope", args_ptr(shell))

    # begin_render returns previous scope (-1 for first render)
    var prev = w[].call_i32("shell_begin_render", args_ptr_i32(shell, scope))
    assert_equal(prev, -1, "no previous scope on first render")

    w[].call_void("shell_end_render", args_ptr_i32(shell, prev))
    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_nested_render(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var parent_scope = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var child_scope = w[].call_i32(
        "shell_create_child_scope", args_ptr_i32(shell, parent_scope)
    )

    # Render parent
    var prev1 = w[].call_i32(
        "shell_begin_render", args_ptr_i32(shell, parent_scope)
    )
    assert_equal(prev1, -1)

    # Nested render child
    var prev2 = w[].call_i32(
        "shell_begin_render", args_ptr_i32(shell, child_scope)
    )

    # End child render
    w[].call_void("shell_end_render", args_ptr_i32(shell, prev2))
    # End parent render
    w[].call_void("shell_end_render", args_ptr_i32(shell, prev1))

    w[].call_void("shell_destroy", args_ptr(shell))


# ── Dirty / Scheduler integration ───────────────────────────────────────────


def test_shell_initially_not_dirty(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    assert_equal(w[].call_i32("shell_has_dirty", args_ptr(shell)), 0)
    assert_equal(w[].call_i32("shell_scheduler_empty", args_ptr(shell)), 1)
    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_dirty_after_signal_write(w: UnsafePointer[WasmInstance]):
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var scope = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var sig = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 0))

    # Subscribe scope to signal
    _ = w[].call_i32("shell_begin_render", args_ptr_i32(shell, scope))
    _ = w[].call_i32("shell_read_signal_i32", args_ptr_i32(shell, sig))
    w[].call_void("shell_end_render", args_ptr_i32(shell, -1))

    # Write → dirty
    w[].call_void("shell_write_signal_i32", args_ptr_i32_i32(shell, sig, 1))
    assert_equal(w[].call_i32("shell_has_dirty", args_ptr(shell)), 1)

    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_collect_and_drain_dirty(w: UnsafePointer[WasmInstance]):
    """collect_dirty + next_dirty yields dirty scopes in order."""
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var scope = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var sig = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 0))

    # Subscribe and write
    _ = w[].call_i32("shell_begin_render", args_ptr_i32(shell, scope))
    _ = w[].call_i32("shell_read_signal_i32", args_ptr_i32(shell, sig))
    w[].call_void("shell_end_render", args_ptr_i32(shell, -1))
    w[].call_void("shell_write_signal_i32", args_ptr_i32_i32(shell, sig, 42))

    # Collect dirty into scheduler
    w[].call_void("shell_collect_dirty", args_ptr(shell))
    assert_equal(w[].call_i32("shell_scheduler_empty", args_ptr(shell)), 0)

    # Runtime dirty queue should be drained
    assert_equal(w[].call_i32("shell_has_dirty", args_ptr(shell)), 0)

    # Drain from scheduler
    var dirty_scope = w[].call_i32("shell_next_dirty", args_ptr(shell))
    assert_equal(dirty_scope, scope)
    assert_equal(w[].call_i32("shell_scheduler_empty", args_ptr(shell)), 1)

    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_dirty_height_ordering(w: UnsafePointer[WasmInstance]):
    """Scheduler yields parent before child when both are dirty."""
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var parent = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var child = w[].call_i32(
        "shell_create_child_scope", args_ptr_i32(shell, parent)
    )
    var sig = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 0))

    # Subscribe both scopes to the same signal
    _ = w[].call_i32("shell_begin_render", args_ptr_i32(shell, parent))
    _ = w[].call_i32("shell_read_signal_i32", args_ptr_i32(shell, sig))
    w[].call_void("shell_end_render", args_ptr_i32(shell, -1))

    _ = w[].call_i32("shell_begin_render", args_ptr_i32(shell, child))
    _ = w[].call_i32("shell_read_signal_i32", args_ptr_i32(shell, sig))
    w[].call_void("shell_end_render", args_ptr_i32(shell, -1))

    # Write → both dirty
    w[].call_void("shell_write_signal_i32", args_ptr_i32_i32(shell, sig, 99))

    # Collect and verify ordering
    w[].call_void("shell_collect_dirty", args_ptr(shell))
    var first = w[].call_i32("shell_next_dirty", args_ptr(shell))
    assert_equal(first, parent, "parent (height 0) should come first")
    var second = w[].call_i32("shell_next_dirty", args_ptr(shell))
    assert_equal(second, child, "child (height 1) should come second")

    w[].call_void("shell_destroy", args_ptr(shell))


# ── Event dispatch ───────────────────────────────────────────────────────────


def test_shell_dispatch_event(w: UnsafePointer[WasmInstance]):
    """dispatch_event routes to the runtime's handler registry."""
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var rt = Int(w[].call_i64("shell_rt_ptr", args_ptr(shell)))
    var scope = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var sig = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 0))

    # Register a handler that adds 5 to the signal
    var click_str = w[].write_string_struct("click")
    var handler = w[].call_i32(
        "handler_register_signal_add",
        args_ptr_i32_i32_i32_ptr(rt, scope, sig, 5, click_str),
    )

    # Subscribe scope to signal for dirty tracking
    _ = w[].call_i32("shell_begin_render", args_ptr_i32(shell, scope))
    _ = w[].call_i32("shell_read_signal_i32", args_ptr_i32(shell, sig))
    w[].call_void("shell_end_render", args_ptr_i32(shell, -1))

    # Dispatch
    var executed = w[].call_i32(
        "shell_dispatch_event", args_ptr_i32_i32(shell, handler, 0)
    )
    assert_equal(executed, 1, "handler should execute")

    # Signal should be updated
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell, sig)), 5
    )

    # Scope should be dirty
    assert_equal(w[].call_i32("shell_has_dirty", args_ptr(shell)), 1)

    w[].call_void("shell_destroy", args_ptr(shell))


# ── Mount ────────────────────────────────────────────────────────────────────


def test_shell_mount_text_vnode(w: UnsafePointer[WasmInstance]):
    """shell_mount produces valid mutation bytes for a text VNode."""
    var shell = Int(w[].call_i64("shell_create", no_args()))

    # Create a text VNode in the store
    var store = Int(w[].call_i64("shell_store_ptr", args_ptr(shell)))
    var text_ptr = w[].write_string_struct("hello world")
    var vn = w[].call_i32("vnode_push_text", args_ptr_ptr(store, text_ptr))

    # Allocate mutation buffer
    var buf = Int(w[].call_i64("mutation_buf_alloc", args_i32(4096)))

    # Mount
    var byte_len = w[].call_i32(
        "shell_mount", args_ptr_ptr_i32_i32(shell, buf, 4096, vn)
    )
    assert_true(byte_len > 0, "mount should produce mutations")

    w[].call_void("mutation_buf_free", args_ptr(buf))
    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_mount_template_ref(w: UnsafePointer[WasmInstance]):
    """shell_mount produces mutations for a TemplateRef VNode."""
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var rt = Int(w[].call_i64("shell_rt_ptr", args_ptr(shell)))
    var store = Int(w[].call_i64("shell_store_ptr", args_ptr(shell)))

    # Build and register a simple template: div > text("hi")
    var tmpl_name_ptr = w[].write_string_struct("test-tmpl")
    var builder = Int(
        w[].call_i64("tmpl_builder_create", args_ptr(tmpl_name_ptr))
    )
    var div_idx = w[].call_i32(
        "tmpl_builder_push_element", args_ptr_i32_i32(builder, 0, -1)
    )
    var hi_ptr = w[].write_string_struct("hi")
    _ = w[].call_i32(
        "tmpl_builder_push_text",
        args_ptr_ptr_i32(builder, hi_ptr, div_idx),
    )
    var tmpl_id = w[].call_i32(
        "tmpl_builder_register", args_ptr_ptr(rt, builder)
    )
    w[].call_void("tmpl_builder_destroy", args_ptr(builder))

    # Create a TemplateRef VNode
    var vn = w[].call_i32(
        "vnode_push_template_ref", args_ptr_i32(store, tmpl_id)
    )

    # Mount
    var buf = Int(w[].call_i64("mutation_buf_alloc", args_i32(4096)))
    var byte_len = w[].call_i32(
        "shell_mount", args_ptr_ptr_i32_i32(shell, buf, 4096, vn)
    )
    assert_true(byte_len > 0, "mount should produce mutations")

    # VNode should be mounted
    assert_equal(w[].call_i32("vnode_is_mounted", args_ptr_i32(store, vn)), 1)

    w[].call_void("mutation_buf_free", args_ptr(buf))
    w[].call_void("shell_destroy", args_ptr(shell))


# ── Diff ─────────────────────────────────────────────────────────────────────


def test_shell_diff_same_text_zero_mutations(w: UnsafePointer[WasmInstance]):
    """Diffing identical text VNodes produces 0 mutation bytes (just End)."""
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var store = Int(w[].call_i64("shell_store_ptr", args_ptr(shell)))

    # Create old text VNode and mount it
    var same_ptr1 = w[].write_string_struct("same")
    var old_vn = w[].call_i32("vnode_push_text", args_ptr_ptr(store, same_ptr1))
    var buf = Int(w[].call_i64("mutation_buf_alloc", args_i32(4096)))
    _ = w[].call_i32(
        "shell_mount", args_ptr_ptr_i32_i32(shell, buf, 4096, old_vn)
    )

    # Create new text VNode with same content
    var same_ptr2 = w[].write_string_struct("same")
    var new_vn = w[].call_i32("vnode_push_text", args_ptr_ptr(store, same_ptr2))

    # Diff
    var diff_len = w[].call_i32(
        "shell_diff",
        args_ptr_ptr_i32_i32_i32(shell, buf, 4096, old_vn, new_vn),
    )
    # Only the End sentinel (1 byte) should be written
    assert_equal(diff_len, 1, "same text → only End sentinel")

    w[].call_void("mutation_buf_free", args_ptr(buf))
    w[].call_void("shell_destroy", args_ptr(shell))


def test_shell_diff_text_changed(w: UnsafePointer[WasmInstance]):
    """Diffing different text VNodes produces SetText mutations."""
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var store = Int(w[].call_i64("shell_store_ptr", args_ptr(shell)))

    # Create and mount old text VNode
    var before_ptr = w[].write_string_struct("before")
    var old_vn = w[].call_i32(
        "vnode_push_text", args_ptr_ptr(store, before_ptr)
    )
    var buf = Int(w[].call_i64("mutation_buf_alloc", args_i32(4096)))
    _ = w[].call_i32(
        "shell_mount", args_ptr_ptr_i32_i32(shell, buf, 4096, old_vn)
    )

    # Create new text VNode with different content
    var after_ptr = w[].write_string_struct("after")
    var new_vn = w[].call_i32("vnode_push_text", args_ptr_ptr(store, after_ptr))

    # Diff
    var diff_len = w[].call_i32(
        "shell_diff",
        args_ptr_ptr_i32_i32_i32(shell, buf, 4096, old_vn, new_vn),
    )
    # Should produce SetText + End = more than 1 byte
    assert_true(diff_len > 1, "text change should produce SetText mutation")

    w[].call_void("mutation_buf_free", args_ptr(buf))
    w[].call_void("shell_destroy", args_ptr(shell))


# ── Full mount → update cycle ────────────────────────────────────────────────


def test_shell_full_mount_update_cycle(w: UnsafePointer[WasmInstance]):
    """End-to-end: create shell, mount, write signal, collect dirty, diff."""
    var shell = Int(w[].call_i64("shell_create", no_args()))
    var rt = Int(w[].call_i64("shell_rt_ptr", args_ptr(shell)))
    var store = Int(w[].call_i64("shell_store_ptr", args_ptr(shell)))

    # 1. Create scope and signal
    var scope = w[].call_i32("shell_create_root_scope", args_ptr(shell))
    var sig = w[].call_i32("shell_create_signal_i32", args_ptr_i32(shell, 0))

    # 2. Register template: div > dynamic_text[0]
    var tmpl_name_ptr = w[].write_string_struct("cycle-tmpl")
    var builder = Int(
        w[].call_i64("tmpl_builder_create", args_ptr(tmpl_name_ptr))
    )
    var div_idx = w[].call_i32(
        "tmpl_builder_push_element", args_ptr_i32_i32(builder, 0, -1)
    )
    _ = w[].call_i32(
        "tmpl_builder_push_dynamic_text",
        args_ptr_i32_i32(builder, 0, div_idx),
    )
    var tmpl_id = w[].call_i32(
        "tmpl_builder_register", args_ptr_ptr(rt, builder)
    )
    w[].call_void("tmpl_builder_destroy", args_ptr(builder))

    # 3. Subscribe scope
    _ = w[].call_i32("shell_begin_render", args_ptr_i32(shell, scope))
    _ = w[].call_i32("shell_read_signal_i32", args_ptr_i32(shell, sig))
    w[].call_void("shell_end_render", args_ptr_i32(shell, -1))

    # 4. Build initial VNode and mount
    var v0 = w[].call_i32(
        "vnode_push_template_ref", args_ptr_i32(store, tmpl_id)
    )
    var count0_ptr = w[].write_string_struct("Count: 0")
    w[].call_void(
        "vnode_push_dynamic_text_node",
        args_ptr_i32_ptr(store, v0, count0_ptr),
    )

    var buf = Int(w[].call_i64("mutation_buf_alloc", args_i32(8192)))
    var mount_len = w[].call_i32(
        "shell_mount", args_ptr_ptr_i32_i32(shell, buf, 8192, v0)
    )
    assert_true(mount_len > 0, "mount should produce mutations")

    # 5. Write signal → scope dirty
    w[].call_void("shell_write_signal_i32", args_ptr_i32_i32(shell, sig, 1))
    assert_equal(w[].call_i32("shell_has_dirty", args_ptr(shell)), 1)

    # 6. Collect dirty into scheduler
    w[].call_void("shell_collect_dirty", args_ptr(shell))
    assert_equal(w[].call_i32("shell_scheduler_empty", args_ptr(shell)), 0)

    # 7. Drain scheduler
    var dirty = w[].call_i32("shell_next_dirty", args_ptr(shell))
    assert_equal(dirty, scope)

    # 8. Build new VNode with updated text
    var v1 = w[].call_i32(
        "vnode_push_template_ref", args_ptr_i32(store, tmpl_id)
    )
    var count1_ptr = w[].write_string_struct("Count: 1")
    w[].call_void(
        "vnode_push_dynamic_text_node",
        args_ptr_i32_ptr(store, v1, count1_ptr),
    )

    # 9. Diff old → new
    var diff_len = w[].call_i32(
        "shell_diff", args_ptr_ptr_i32_i32_i32(shell, buf, 8192, v0, v1)
    )
    assert_true(diff_len > 1, "diff should produce SetText mutation")

    # 10. No more dirty
    assert_equal(w[].call_i32("shell_has_dirty", args_ptr(shell)), 0)
    assert_equal(w[].call_i32("shell_scheduler_empty", args_ptr(shell)), 1)

    w[].call_void("mutation_buf_free", args_ptr(buf))
    w[].call_void("shell_destroy", args_ptr(shell))


# ── Subsystem isolation ──────────────────────────────────────────────────────


def test_shell_independent_instances(w: UnsafePointer[WasmInstance]):
    """Two AppShells are fully independent (no shared state)."""
    var shell_a = Int(w[].call_i64("shell_create", no_args()))
    var shell_b = Int(w[].call_i64("shell_create", no_args()))

    # Create signals in each
    var sig_a = w[].call_i32(
        "shell_create_signal_i32", args_ptr_i32(shell_a, 100)
    )
    var sig_b = w[].call_i32(
        "shell_create_signal_i32", args_ptr_i32(shell_b, 200)
    )

    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell_a, sig_a)), 100
    )
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell_b, sig_b)), 200
    )

    # Writing to one does not affect the other
    w[].call_void(
        "shell_write_signal_i32", args_ptr_i32_i32(shell_a, sig_a, 999)
    )
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell_a, sig_a)), 999
    )
    assert_equal(
        w[].call_i32("shell_peek_signal_i32", args_ptr_i32(shell_b, sig_b)), 200
    )

    w[].call_void("shell_destroy", args_ptr(shell_a))
    w[].call_void("shell_destroy", args_ptr(shell_b))
