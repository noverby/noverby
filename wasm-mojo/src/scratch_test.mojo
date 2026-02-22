from memory import UnsafePointer, alloc


struct Foo(Movable):
    var x: Int

    fn __init__(out self):
        self.x = 42

    fn __moveinit__(out self, deinit other: Self):
        self.x = other.x


fn main():
    var p = alloc[Foo](1)
    p.init_pointee_move(Foo())
    print(p[].x)
    p.destroy_pointee()
    p.free()
