# 🔥 Mojo UI Framework — Development Plan

A Dioxus-inspired, signal-based UI framework written in Mojo, compiled to WebAssembly, rendered by a thin JavaScript interpreter.

> **Codename:** `mojo-ui` (working title)
>
> **Inspiration:** [Dioxus](https://github.com/DioxusLabs/dioxus) — signals-based reactivity, stack-based mutations, template pre-compilation, fine-grained subscriber tracking.
>
> **Starting point:** The `wasm-mojo` proof-of-concept already demonstrates that Mojo compiles to WASM and that primitives, strings, and memory management work across the WASM↔JS boundary. This plan builds on that foundation.

---

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [How Dioxus Works (Reference Model)](#how-dioxus-works-reference-model)
- [rsx! vs Mojo Parameters — Compile-Time UI](#rsx-vs-mojo-parameters--compile-time-ui)
- [Ergonomics-First API Design](#ergonomics-first-api-design)
- [Design Principles](#design-principles)
- [Phase 0 — Foundation Hardening](#phase-0--foundation-hardening)
- [Phase 1 — Signals & Reactivity](#phase-1--signals--reactivity)
- [Phase 2 — Scopes & Components](#phase-2--scopes--components)
- [Phase 3 — Templates & VNodes](#phase-3--templates--vnodes)
- [Phase 4 — Mutations & Diffing](#phase-4--mutations--diffing)
- [Phase 5 — JS Interpreter (Renderer)](#phase-5--js-interpreter-renderer)
- [Phase 6 — Events](#phase-6--events)
- [Phase 7 — First App (End-to-End)](#phase-7--first-app-end-to-end)
- [Phase 8 — Advanced Features](#phase-8--advanced-features)
- [Phase 9 — Performance & Polish](#phase-9--performance--polish)
- [Test Strategy](#test-strategy)
- [Project Structure](#project-structure)
- [Open Questions](#open-questions)
- [Milestone Checklist](#milestone-checklist)

---

## Architecture Overview

```txt
┌─────────────────────────────────────────────────────────────────┐
│                         Mojo (WASM)                             │
│                                                                 │
│  ┌───────────┐    ┌──────────┐    ┌───────────┐                 │
│  │ Component  │───→│  Scope   │───→│  VNode /  │                │
│  │ Functions  │    │  State   │    │ Templates │                 │
│  └───────────┘    └────┬─────┘    └─────┬─────┘                 │
│                        │                │                       │
│  ┌───────────┐    ┌────┴─────┐    ┌─────┴──────┐               │
│  │ Signal[T] │←──→│ Reactive │    │   Diff /   │               │
│  │  read()   │    │ Context  │    │  Reconcile │               │
│  │  write()  │    │ Tracking │    └─────┬──────┘               │
│  └───────────┘    └──────────┘          │                       │
│                                   ┌─────┴──────┐               │
│       ┌──────────┐                │  Mutations  │               │
│       │  Memo[T] │                │  (stack     │               │
│       │  Effect  │                │   machine)  │               │
│       └──────────┘                └─────┬──────┘               │
│                                         │                       │
│                                         ↓                       │
├─────────────────────────────────────────┼───────────────────────┤
│              Shared Linear Memory       │                       │
│         ┌───────────────────────┐       │                       │
│         │   Mutation Buffer     │←──────┘                       │
│         │   (WASM → JS)        │                                │
│         ├───────────────────────┤                                │
│         │   Event Buffer        │───────→ dispatch_event()      │
│         │   (JS → WASM)        │                                │
│         └───────────────────────┘                                │
├─────────────────────────────────────────────────────────────────┤
│                      JavaScript Runtime                         │
│                                                                 │
│  ┌──────────────┐  ┌───────────────┐  ┌───────────────────┐    │
│  │  Interpreter │  │  Template     │  │  Event Delegation │    │
│  │  (apply      │  │  Cache        │  │  (capture → WASM) │    │
│  │   mutations) │  │  (DOM frags)  │  └───────────────────┘    │
│  └──────────────┘  └───────────────┘                            │
└─────────────────────────────────────────────────────────────────┘
```

**Split of responsibilities:**

| Layer | Language | Responsibility |
|---|---|---|
| Signals, reactivity, components, diffing | Mojo (WASM) | All framework logic — the "brain" |
| Mutation interpretation, DOM patching, event capture | TypeScript (JS) | Thin DOM interface — the "hands" |
| Communication | Shared linear memory | Stack-based mutation buffer + event buffer |

---

## How Dioxus Works (Reference Model)

Understanding Dioxus's architecture is essential since we're modeling after it. Here are the key concepts we're adapting:

### Signals (not setState)

Dioxus uses fine-grained reactivity inspired by Solid.js. A `Signal<T>` is a lightweight handle (Copy in Rust) that wraps a value with automatic subscriber tracking:

```rust
// Dioxus signal usage
let mut count = use_signal(|| 0);
// Reading subscribes the current scope automatically
let value = count.read();     // or count() for short
// Writing notifies all subscribers
count.write() += 1;           // or count += 1
```

When a signal is `.read()` inside a component, that component's scope is automatically registered as a subscriber. When `.write()` is called, all subscribed scopes are marked dirty and scheduled for re-render. **No dependency arrays, no explicit subscriptions.**

### Reactive Contexts

Every component runs inside a `ReactiveContext`. When a signal is read, the current reactive context is detected and subscribed. This is how the framework knows which components to re-render when state changes.

### Memo / Computed

`Memo<T>` is a derived signal that recomputes only when its input signals change:

```rust
let doubled = use_memo(move || count() * 2);
```

### Templates

Dioxus pre-compiles the static structure of UI at compile time. The `rsx!` macro splits markup into:

- **Static template** — the unchanging HTML structure (compiled once)
- **Dynamic slots** — holes where runtime values go

This means the framework never diffs static nodes — only dynamic values are compared.

### Stack-Based Mutations

Rather than producing a flat list of "create element, set attribute, append" instructions, Dioxus uses a **stack machine**. The `WriteMutations` trait defines operations like:

| Mutation | Description |
|---|---|
| `LoadTemplate(name, index, id)` | Clone a pre-compiled template, assign it an ElementId |
| `PushRoot(id)` | Push a node onto the stack |
| `AppendChildren(id, m)` | Pop m nodes from stack, append as children of id |
| `CreateTextNode(text, id)` | Create a text node with the given ID |
| `CreatePlaceholder(id)` | Create an empty placeholder for future content |
| `SetAttribute(name, ns, value, id)` | Set an attribute on a node |
| `SetText(value, id)` | Update text content |
| `NewEventListener(name, id)` | Attach an event listener |
| `RemoveEventListener(name, id)` | Remove an event listener |
| `Remove(id)` | Remove a node |
| `InsertAfter(id, m)` | Insert m stack nodes after id |
| `InsertBefore(id, m)` | Insert m stack nodes before id |
| `ReplaceWith(id, m)` | Replace node id with m stack nodes |
| `ReplacePlaceholder(path, m)` | Replace placeholder at path with m stack nodes |
| `AssignId(path, id)` | Assign an ElementId to a node at a path in the template |

### ElementId

Every dynamic DOM node gets a unique `ElementId(u32)`. This is the only identifier shared between WASM and JS. The JS side maintains a `Map<number, Node>` — the renderer never needs to understand the component tree.

### Scopes

Each component instance has a `Scope` that tracks:

- Its rendered output (previous VNodes)
- Its height in the tree (for ordering re-renders parent-first)
- Its dirty status
- Its subscriptions (which signals it depends on)

### The Render Loop

```text
Event arrives (click, timer, etc.)
  → Signal.write() called
  → All subscribed scopes marked dirty
  → Scheduler collects dirty scopes, sorts by height
  → For each dirty scope:
      → Re-run the component function
      → Diff new output against previous output
      → Emit mutations
  → Flush all mutations to renderer
  → Renderer applies mutations to real DOM
```

---

## rsx! vs Mojo Parameters — Compile-Time UI

Dioxus's power comes from its `rsx!` proc macro, which parses JSX-like syntax at compile time and extracts static templates from dynamic values. Mojo doesn't have proc macros, but its [parameter system](https://docs.modular.com/mojo/manual/parameters/) provides powerful compile-time metaprogramming that can achieve similar goals through different means.

### What `rsx!` Does in Dioxus

The `rsx!` macro does four things at compile time:

1. **Parses JSX-like syntax** into VNode construction code
2. **Extracts static templates** — unchanging HTML structure becomes a `Template` constant with `&'static str` tag names and static attributes
3. **Identifies dynamic slots** — any runtime expression (`{count}`) becomes a "dynamic node" or "dynamic attribute" placed into a numbered slot
4. **Generates a unique template name** — based on `file:line:col` for deduplication

The result: static structure is zero-cost at runtime. Only dynamic slot values are evaluated each render.

```rust
// What the developer writes:
rsx! {
    div { class: "counter",
        h1 { "Count: {count}" }
        button { onclick: move |_| count += 1, "Increment" }
    }
}

// What the macro generates (simplified):
static TEMPLATE: Template = Template {
    name: "src/app.rs:10:5",
    roots: &[
        TemplateNode::Element {
            tag: "div",
            attrs: &[TemplateAttribute::Static { name: "class", value: "counter" }],
            children: &[
                TemplateNode::Element {
                    tag: "h1",
                    children: &[TemplateNode::DynamicText { id: 0 }],
                },
                TemplateNode::Element {
                    tag: "button",
                    children: &[TemplateNode::Text { text: "Increment" }],
                },
            ],
        },
    ],
};

VNode {
    template: &TEMPLATE,
    dynamic_nodes: &[DynamicNode::Text(format!("Count: {count}"))],
    dynamic_attrs: &[Attribute::new("onclick", handler)],
}
```

### What Mojo's Parameter System Offers

Mojo's compile-time facilities are different from proc macros but surprisingly powerful:

| Mojo Feature | What It Does | Relevance to UI |
|---|---|---|
| `comptime` values | Compile-time constants (including types) | Template structures as compile-time constants |
| Parametric `comptime` | Compile-time functions returning constants/types | Template factory functions |
| `@parameter for` | Loop unrolling at compile time | Generate child creation code with known child count |
| `@parameter if` | Dead-branch elimination at compile time | Conditional template selection |
| `@parameter fn` | Parametric closures capturable as parameters | Event handlers as compile-time values |
| Variadic parameters `*params` | Variable compile-time argument count | Variable child counts |
| Parameter inference | Auto-deduce parameters from arguments | Ergonomic API without explicit type annotations |
| Automatic parameterization | Unbound types auto-create parameters | Generic component signatures |
| `comptime` struct members | Constants derived from struct parameters | Template metadata (child count, slot count) |

### What We CAN Do

#### 1. Templates as `comptime` Constants

The most direct analog to `rsx!`'s template extraction. Static UI structure becomes a compile-time constant:

```text
comptime counter_template = Template(
    name = "counter.mojo:10:5",
    roots = StaticArray[TemplateNode, 1](
        TemplateNode.Element(
            tag = "div",
            attrs = StaticArray[TemplateAttr, 1](
                TemplateAttr.Static(name="class", value="counter"),
            ),
            children = StaticArray[TemplateNode, 2](
                TemplateNode.Element(
                    tag = "h1",
                    children = StaticArray[TemplateNode, 1](
                        TemplateNode.DynamicText(index=0),
                    ),
                ),
                TemplateNode.Element(
                    tag = "button",
                    children = StaticArray[TemplateNode, 1](
                        TemplateNode.StaticText("Increment"),
                    ),
                ),
            ),
        ),
    ),
)
```

This is verbose but achieves exactly what `rsx!` generates: a zero-cost static template. The `comptime` keyword guarantees it's resolved at compile time.

#### 2. Parametric `comptime` Template Factories

Use parametric `comptime` values to create reusable template patterns:

```text
# A compile-time function that creates a template node
comptime div_node[N: Int](children: StaticArray[TemplateNode, N]) : TemplateNode =
    TemplateNode.Element(tag="div", children=children)

comptime text_slot[idx: Int]() : TemplateNode =
    TemplateNode.DynamicText(index=idx)

comptime static_text[s: StringLiteral]() : TemplateNode =
    TemplateNode.StaticText(s)

# Now template definition is more concise:
comptime my_template = Template(
    name = "my_component",
    roots = StaticArray(div_node(StaticArray(
        text_slot[0](),
        static_text["hello"](),
    ))),
)
```

#### 3. `@parameter for` for Compile-Time Child Generation

When the number of children is known at compile time, unroll the creation loop:

```text
fn create_static_list[N: Int](items: StaticArray[StringLiteral, N]) -> VNode:
    var children = DynArray[VNode](capacity=N)
    @parameter
    for i in range(N):
        # Each iteration is a separate compile-time-generated block
        children.push(li(text(items[i])))
    return ul_from(children)

# Usage — N inferred as 3:
var list = create_static_list(StaticArray("Apple", "Banana", "Cherry"))
```

The loop body is duplicated N times by the compiler, each with a different constant `i`. No runtime loop overhead.

#### 4. `@parameter if` for Conditional Templates

Select between entirely different templates at compile time — the dead branch is eliminated from the binary:

```text
fn themed_button[dark: Bool]() -> VNode:
    @parameter
    if dark:
        comptime tmpl = dark_button_template
    else:
        comptime tmpl = light_button_template
    return VNode.from_template(tmpl, dynamic_texts=[], dynamic_attrs=[])
```

Only one template is compiled in. This is Mojo's equivalent of Dioxus's `#[cfg()]` on templates.

#### 5. Parameterized Component Structs

Components can be parameterized on compile-time values, enabling zero-cost specialization:

```text
struct Counter[initial: Int]:
    comptime template = Template(
        name = "Counter",
        roots = StaticArray(div_node(StaticArray(
            TemplateNode.Element(tag="h1", children=StaticArray(text_slot[0]())),
            TemplateNode.Element(tag="button", children=StaticArray(static_text["+"]())),
        ))),
    )

    fn render(self) -> VNode:
        var count = use_signal(fn() -> Int: return initial)
        return VNode.from_template[Self.template](
            dynamic_texts = DynArray(str(count.read())),
            dynamic_attrs = DynArray(on_click(fn(_: EventData):
                count.write(count.peek() + 1)
            )),
        )
```

The `Counter[0]` and `Counter[100]` are different compile-time specializations, each with their own template baked in.

#### 6. `@parameter fn` Closures as Event Handlers

Parametric closures can capture values and be passed as compile-time parameters — critical for event handler registration:

```text
fn make_button(label: StringLiteral, handler: fn(EventData) capturing [_]) -> VNode:
    return button(on_click=handler, text(label))

fn counter_view() -> VNode:
    var count = use_signal(fn() -> Int: return 0)

    @parameter
    fn increment(event: EventData):
        count.write(count.peek() + 1)

    return div(
        h1(text("Count: " + str(count.read()))),
        make_button("+", increment),
    )
```

#### 7. Variadic Parameters for Flexible Children

Tag helper functions can accept a variable number of compile-time children:

```text
fn div[*child_count: Int](children: StaticArray[VNode, child_count]) -> VNode:
    # child_count is known at compile time
    var dyn_children = DynArray[VNode](capacity=child_count)
    @parameter
    for i in range(child_count):
        dyn_children.push(children[i])
    return VNode.Element(tag="div", children=dyn_children)
```

Note: variadic parameters must be homogeneous in Mojo (all the same type), which works here because all children are `VNode`.

### What We CANNOT Replicate

| `rsx!` Feature | Why Not in Mojo | Workaround |
|---|---|---|
| Custom JSX-like syntax | No proc macros / token stream manipulation | Builder API: `div(h1(text("Hello")))` |
| Automatic static/dynamic separation | No AST analysis at compile time | Manual: developer defines `comptime` templates explicitly, or uses Tier 1 builder which does it at runtime |
| Inline format strings `"Count: {count}"` | Mojo doesn't have string interpolation in this context | Explicit: `"Count: " + str(count.read())` |
| Compile-time HTML validation | No compile-time string parsing | Runtime validation or external linter |
| Heterogeneous variadic children | Variadic params must be same type | All children are `VNode` (a variant type), so this works |

### Three-Tier Strategy

We adopt a graduated approach. Each tier is usable on its own, and we build them in order:

#### Tier 1: Runtime Builder (Phase 3 — implement first)

Fully dynamic. No compile-time template extraction. Simple, correct, easy to implement. Uses the ergonomic API from [Ergonomics-First API Design](#ergonomics-first-api-design).

```text
def app() -> Element:
    var count = signal(0)
    div(class_="counter",
        h1("Count: ", count),
        button(onclick=fn(_): count += 1, "+"),
        button(onclick=fn(_): count -= 1, "-"),
    )
```

Tag helpers construct Elements at runtime. Templates are created dynamically on first render and cached by the runtime. Performance is good but not optimal — we diff the entire tree each render.

**This is our MVP.** It's how the framework works for Phase 3 through Phase 7.

#### Tier 2: Compile-Time Templates (Phase 9 optimization)

Templates are `comptime` constants. The builder API is enhanced to extract static structure at compile time when possible. **The ergonomic Tier 1 API remains the recommended way to write components** — Tier 2 is for hot paths only.

```text
def app() -> Element:
    comptime tmpl = Template("app", StaticArray(
        div_node(StaticArray(
            h1_node(StaticArray(text_slot[0]())),
            button_node(StaticArray(static_text["+"]())),
        )),
    ))

    var count = signal(0)
    Element.from_template[tmpl](
        dynamic_texts = DynArray(str(count())),
        dynamic_attrs = DynArray(EventAttr("onclick", fn(_): count += 1)),
    )
```

This is more verbose than the Tier 1 builder, but achieves `rsx!`-level performance: static structure compiled once, only dynamic slots evaluated per render. The diff engine only compares dynamic slots, not the full tree.

#### Tier 3: External Codegen (Future / Out of Scope)

A preprocessor or Mojo compiler plugin that transforms a DSL file into Tier 2 code:

```text
# counter.mojui (hypothetical DSL)
component app:
    signal count: Int = 0

    <div>
        <h1>Count: {count}</h1>
        <button @click={count += 1}>Increment</button>
    </div>
```

↓ generates Tier 2 Mojo code ↓

This is out of scope for the initial framework but is the natural evolution if Mojo adds macro support or if we build an external tool.

### Tests for Compile-Time Features

**Tests (`test/comptime/templates.test.ts`):**

- `comptime` template has correct structure (verified via exported introspection functions)
- Two components using the same `comptime` template → share template ID
- `@parameter for` with N=0, 1, 5, 100 children → correct VNode child count
- `@parameter if` with true/false → only one branch's template present
- Parametric component `Counter[0]` vs `Counter[100]` → different initial values, same template structure

**Tests (`test/comptime/builder.test.ts`):**

- Tier 1 builder produces valid VNode trees
- Tier 2 `VNode.from_template` fills dynamic slots correctly
- Template with 0 dynamic slots → all static, no per-render allocation
- Template with only dynamic slots → all values from runtime
- Mixed static + dynamic → static parts unchanged across re-renders

---

## Ergonomics-First API Design

The framework must appeal to web developers. If the API is more verbose than React or Svelte, adoption is dead on arrival. Every design decision must be evaluated against: **"would a web developer find this annoying?"**

### Target: Dioxus-Level Conciseness Without Proc Macros

Here is the gold standard we're aiming for — our counter app compared to the competition:

**mojo-ui (our goal):**

```text
fn counter() -> Element:
    var count = signal(0)

    div(class_="counter",
        h1("Count: ", count),
        button(onclick=fn(_): count += 1, "+"),
        button(onclick=fn(_): count -= 1, "-"),
    )
```

**Dioxus (Rust):**

```text
fn counter() -> Element {
    let mut count = use_signal(|| 0);
    rsx! {
        div { class: "counter",
            h1 { "Count: {count}" }
            button { onclick: move |_| count += 1, "+" }
            button { onclick: move |_| count -= 1, "-" }
        }
    }
}
```

**React (JavaScript):**

```text
function Counter() {
    const [count, setCount] = useState(0);
    return (
        <div className="counter">
            <h1>Count: {count}</h1>
            <button onClick={() => setCount(c => c + 1)}>+</button>
            <button onClick={() => setCount(c => c - 1)}>-</button>
        </div>
    );
}
```

**Svelte:**

```text
<script>
let count = 0;
</script>
<div class="counter">
    <h1>Count: {count}</h1>
    <button on:click={() => count++}>+</button>
    <button on:click={() => count--}>-</button>
</div>
```

Our Mojo API is **competitive with all of these** — no more verbose than Dioxus, shorter than React, and only slightly longer than Svelte (which has custom compiler magic).

### Ergonomic API Rules

Every API in the framework must follow these rules:

| Rule | Bad (verbose) | Good (concise) |
|---|---|---|
| Short signal creation | `use_signal(fn() -> Int: return 0)` | `signal(0)` |
| Callable signals | `count.read()` | `count()` |
| Operator overloading | `count.write(count.peek() + 1)` | `count += 1` |
| Implicit text nodes | `text("hello")` | `"hello"` (bare string is a child) |
| Inline signal display | `text(str(count.read()))` | `count` as a child (auto-converts to text) |
| Keyword attributes | `attrs(class_("x"))` | `class_="x"` |
| Inline event handlers | `on_click(fn(_: EventData): ...)` | `onclick=fn(_): ...` |
| Short type alias | `VNode` | `Element` |
| Implicit return | `return div(...)` | `div(...)` (last expression in `def`) |
| No explicit `text()` wrapper | `h1(text("Hello"))` | `h1("Hello")` |

### Mojo Features That Enable Conciseness

#### 1. `signal(value)` — Short Signal Creation

```text
fn signal[T](initial: T) -> Signal[T]:
    # Wraps use_signal with a simpler API
    return use_signal(fn() -> T: return initial)
```

Parameter inference deduces `T` from the argument. `signal(0)` creates a `Signal[Int]`, `signal("hello")` creates a `Signal[String]`. No type annotation needed.

#### 2. `Signal.__call__()` — Callable Read

```text
struct Signal[T]:
    fn __call__(self) -> T:
        # Subscribes current reactive context, returns value
        return self.read()
```

This means `count()` is the same as `count.read()`. For even shorter syntax when used as a child node, `Signal` also conforms to `Renderable` (see below).

#### 3. `Signal.__iadd__()` etc. — Operator Overloading

```text
struct Signal[T]:
    fn __iadd__(inout self, rhs: T):
        self.write(self.peek() + rhs)

    fn __isub__(inout self, rhs: T):
        self.write(self.peek() - rhs)

    fn __imul__(inout self, rhs: T):
        self.write(self.peek() * rhs)
```

Now `count += 1` compiles to a signal read-modify-write. This is the single biggest ergonomic win — event handlers become trivially short.

#### 4. `Renderable` Trait — Bare Strings and Signals as Children

The key insight: tag functions like `div()`, `h1()`, `button()` accept `*children` of any type that conforms to `Renderable`:

```text
trait Renderable:
    fn into_node(self) -> VNode

# String literals become text nodes
fn Renderable.into_node(self: StringLiteral) -> VNode:
    return VNode.Text(String(self))

# Strings become text nodes
fn Renderable.into_node(self: String) -> VNode:
    return VNode.Text(self)

# Signals auto-display as reactive text
fn Renderable.into_node[T: Stringable](self: Signal[T]) -> VNode:
    return VNode.Text(str(self.read()))

# VNodes pass through
fn Renderable.into_node(self: VNode) -> VNode:
    return self
```

This means **all of these work as children** without wrappers:

```text
div(
    "Hello, ",                  # StringLiteral → text node
    username,                   # Signal[String] → reactive text node
    " you have ",
    count,                      # Signal[Int] → reactive text node (auto str())
    " messages",
    p("A paragraph"),           # VNode → passed through
)
```

No `text()` wrapper needed. Ever.

#### 5. Keyword Arguments for Attributes and Events

Mojo supports keyword arguments natively. Tag functions use `**kwargs` or explicit keyword params:

```text
fn div(*children: Renderable, class_: String = "", id_: String = "",
       style: String = "", onclick: Optional[EventHandler] = None,
       ...) -> Element:
    ...
```

Usage: `div(class_="container", onclick=handler, "child text")`

The trailing underscore convention (`class_`, `id_`, `for_`, `type_`) avoids Mojo keyword conflicts, matching Python convention that web developers from Django/Flask already know.

#### 6. `Element` Type Alias

```text
comptime Element = VNode
```

Shorter, matches Dioxus naming, familiar to web developers.

#### 7. `def` for Implicit Return

Components can use `def` instead of `fn` to get implicit last-expression return:

```text
def counter() -> Element:
    var count = signal(0)
    div(class_="counter",
        h1("Count: ", count),
        button(onclick=fn(_): count += 1, "+"),
    )
    # ↑ last expression is implicitly returned
```

### More Complete Examples

**Todo list:**

```text
def todo_app() -> Element:
    var items = signal(List[TodoItem]())
    var input_text = signal("")

    div(class_="todo-app",
        h1("Todo List"),
        form(onsubmit=fn(_):
            items().append(TodoItem(input_text()))
            input_text.set("")
        ,
            input(type_="text", value=input_text(),
                  oninput=fn(e): input_text.set(e.value)),
            button(type_="submit", "Add"),
        ),
        ul(
            each(items(), fn(item) -> Element:
                li(key=str(item.id),
                    class_="done" if item.done() else "",
                    onclick=fn(_): item.done.toggle(),
                    item.text,
                )
            ),
        ),
    )
```

**Conditional rendering:**

```text
def greeting(logged_in: Signal[Bool]) -> Element:
    div(
        if logged_in(): h1("Welcome back!")
        else: h1("Please log in"),
        p("Content goes here"),
    )
```

**Component with props:**

```text
def user_card(name: String, avatar_url: String) -> Element:
    div(class_="user-card",
        img(src=avatar_url, alt=name),
        h2(name),
    )

# Usage in parent:
def app() -> Element:
    div(
        user_card("Alice", "/alice.png"),
        user_card("Bob", "/bob.png"),
    )
```

### Line Count Comparison

| Pattern | React JSX | Dioxus rsx! | mojo-ui |
|---|---|---|---|
| Signal/state creation | 1 line | 1 line | 1 line |
| Display signal value | `{count}` | `{count}` | `count` (as child) |
| Click handler | `onClick={() => set(c+1)}` | `onclick: move \|_\| c += 1` | `onclick=fn(_): c += 1` |
| Text node | `"text"` | `"text"` | `"text"` |
| Attribute | `className="x"` | `class: "x"` | `class_="x"` |
| Conditional | `{cond && <X/>}` | `if cond { X {} }` | `if cond: X()` |
| List | `{items.map(i => <X/>)}` | `for i in items { X {} }` | `each(items, fn(i): X())` |

---

## Design Principles

1. **Ergonomics first.** The API must be no more verbose than Dioxus. Bare strings as children, operator overloading on signals, keyword attributes, implicit return. If a web developer would find it annoying, redesign it. See [Ergonomics-First API Design](#ergonomics-first-api-design).
2. **Signal-first reactivity.** Fine-grained tracking like Dioxus/Solid, not coarse setState like React.
3. **Mojo-idiomatic.** Use `struct` value types, `@parameter` for compile-time work, traits for extensibility — don't fight the language.
4. **Template compilation.** Static UI structure should be determined at compile time. Only dynamic parts cross the WASM↔JS boundary at runtime.
5. **Stack-based mutations.** Match Dioxus's mutation protocol — it's proven and efficient.
6. **Thin JS interpreter.** The JS side is a dumb mutation applier. All logic lives in Mojo.
7. **Testable in layers.** Signals, diffing, and mutations must be testable in pure Mojo without a DOM. The JS interpreter must be testable with hand-crafted mutation buffers without Mojo.
8. **Incremental milestones.** Each phase produces a working, testable artifact.
9. **Three-tier UI construction.** Start with runtime builders (Tier 1), optimize to compile-time templates (Tier 2), leave DSL codegen as future work (Tier 3). See [rsx! vs Mojo Parameters](#rsx-vs-mojo-parameters--compile-time-ui).

---

## Phase 0 — Foundation Hardening

> **Goal:** Upgrade the runtime so it can support a framework's allocation, collection, and ID management needs.
>
> **Depends on:** Current `wasm-mojo` working state

### 0.1 Arena Allocator

The current bump allocator never frees memory. A framework that creates and destroys component trees every frame needs actual deallocation.

**Mojo side (`src/alloc/`):**

- `Arena` — region-based allocator. Allocate many objects, free them all at once.
- `Pool[T]` — fixed-size object pool for hot-path structs (signals, scopes, VNodes).
- Export: `arena_create`, `arena_alloc`, `arena_reset`, `arena_destroy`

**JS side (`runtime/memory.ts`):**

- Augment with arena-aware allocation tracking
- Memory high-water mark diagnostics

**Tests (`test/alloc.test.ts`):**

- Allocate N blocks from arena, verify no overlap
- Reset arena, verify subsequent allocations reuse memory
- Allocate mixed sizes, verify alignment invariants
- Stress: allocate/reset 10,000 cycles, verify memory bounded
- Pool: alloc/free interleaved, verify reuse

### 0.2 Collections

**Mojo side (`src/collections/`):**

- `DynArray[T]` — growable array (children lists, mutation buffers)
- `SmallVec[T, N]` — stack-allocated up to N, heap after (for props, small children lists)
- `HashMap[K, V]` — open-addressing (for keyed children reconciliation)
- `HashSet[T]` — for subscriber sets
- `Slab[T]` — indexed arena with stable IDs and O(1) insert/remove (for scopes, signals)

**Tests (`test/collections.test.ts`):**

- DynArray: push, pop, get, set, length, grow, clear, iterate
- SmallVec: stack-allocated for N items, transitions to heap at N+1
- HashMap: insert, get, delete, contains, collision handling, resize
- HashSet: insert, contains, remove, iterate
- Slab: insert returns ID, get by ID, remove by ID, insert reuses freed slots
- All: stress with 10,000+ elements

### 0.3 ElementId Allocator

**Mojo side (`src/arena/element_id.mojo`):**

- `ElementId` — a `u32` wrapper
- `ElementIdAllocator` — backed by a slab. `alloc() -> ElementId`, `free(id)`
- Recycling: freed IDs are reused

**Tests (`test/element_id.test.ts`):**

- Alloc IDs are sequential
- Free and re-alloc reuses slot
- Double-free is detected or handled gracefully
- Alloc 10,000 IDs, free half, alloc 5,000 more — all unique at any given time

### 0.4 Mutation Buffer Protocol

Design the binary encoding that WASM writes and JS reads.

**Encoding (matches Dioxus `WriteMutations` trait):**

```txt
Each mutation is a variable-length record:

┌──────────┬─────────────────────────────────────────────┐
│ op: u8   │ payload (variable, depends on op)           │
├──────────┼─────────────────────────────────────────────┤
│ 0x01     │ AppendChildren    { id: u32, m: u32 }       │
│ 0x02     │ AssignId          { path_len: u8, path: [u8], id: u32 } │
│ 0x03     │ CreatePlaceholder { id: u32 }               │
│ 0x04     │ CreateTextNode    { id: u32, len: u32, text: [u8] } │
│ 0x05     │ LoadTemplate      { tmpl_id: u32, index: u32, id: u32 } │
│ 0x06     │ ReplaceWith       { id: u32, m: u32 }       │
│ 0x07     │ ReplacePlaceholder { path_len: u8, path: [u8], m: u32 } │
│ 0x08     │ InsertAfter       { id: u32, m: u32 }       │
│ 0x09     │ InsertBefore      { id: u32, m: u32 }       │
│ 0x0A     │ SetAttribute      { id: u32, ns: u8, name_len: u16, name: [u8], val_len: u32, val: [u8] } │
│ 0x0B     │ SetText           { id: u32, len: u32, text: [u8] } │
│ 0x0C     │ NewEventListener  { id: u32, name_len: u16, name: [u8] } │
│ 0x0D     │ RemoveEventListener { id: u32, name_len: u16, name: [u8] } │
│ 0x0E     │ Remove            { id: u32 }               │
│ 0x0F     │ PushRoot          { id: u32 }               │
│ 0x00     │ End               (sentinel)                │
└──────────┴─────────────────────────────────────────────┘
```

**Event buffer (JS → WASM):**

```txt
┌──────────────┬──────────┬──────────────────────────────┐
│ element_id   │ evt_type │ payload                      │
│ u32          │ u8       │ (type-dependent)             │
├──────────────┼──────────┼──────────────────────────────┤
│ CLICK        │ 0x01     │ client_x: f64, client_y: f64 │
│ INPUT        │ 0x02     │ len: u32, value: [u8]        │
│ KEY_DOWN     │ 0x03     │ key_code: u32, len: u16, key: [u8] │
│ KEY_UP       │ 0x04     │ key_code: u32, len: u16, key: [u8] │
│ MOUSE_MOVE   │ 0x05     │ client_x: f64, client_y: f64 │
│ FOCUS        │ 0x06     │ (none)                       │
│ BLUR         │ 0x07     │ (none)                       │
│ SUBMIT       │ 0x08     │ (none)                       │
│ CHANGE       │ 0x09     │ len: u32, value: [u8]        │
└──────────────┴──────────┴──────────────────────────────┘
```

**Files:**

- `src/bridge/protocol.mojo` — opcodes, `MutationWriter` struct
- `runtime/protocol.ts` — opcodes, `MutationReader` class

**Tests (`test/protocol.test.ts`):**

- Write each opcode in Mojo, read in JS, verify all fields match
- Round-trip every opcode with edge-case payloads (empty strings, max u32, zero IDs)
- Multiple mutations in sequence
- Empty buffer → 0 mutations
- Buffer with only End sentinel → 0 mutations

---

## Phase 1 — Signals & Reactivity

> **Goal:** Implement the core reactive primitive. Signals are the foundation of everything.
>
> **Depends on:** Phase 0.2 (collections — HashSet for subscribers, Slab for storage)

### 1.1 Signal[T]

A `Signal[T]` is a lightweight, copyable handle to a reactive value. The API is designed for **maximum conciseness** — see [Ergonomics-First API Design](#ergonomics-first-api-design).

**Mojo side (`src/signals/signal.mojo`):**

```text
struct Signal[T: Copyable & Movable](Renderable):
    var id: UInt32  # index into global signal slab

    # --- Core API ---

    fn read(self) -> T:
        # 1. Get current reactive context (if any)
        # 2. Subscribe that context to this signal
        # 3. Return the value
        ...

    fn peek(self) -> T:
        # Read without subscribing (for avoiding re-render loops)
        ...

    fn write(inout self, value: T):
        # 1. Store new value
        # 2. Notify all subscribers (mark their scopes dirty)
        ...

    fn set(inout self, value: T):
        # Alias for write() — familiar to web devs
        self.write(value)

    fn modify(inout self, f: fn(inout T)):
        # Read-modify-write with single notification
        ...

    # --- Ergonomic sugar ---

    fn __call__(self) -> T:
        # count() is shorthand for count.read()
        return self.read()

    fn __iadd__[T: Addable](inout self, rhs: T):
        self.write(self.peek() + rhs)

    fn __isub__[T: Subtractable](inout self, rhs: T):
        self.write(self.peek() - rhs)

    fn __imul__[T: Multipliable](inout self, rhs: T):
        self.write(self.peek() * rhs)

    fn into_node(self) -> VNode:
        # Signal used as a child auto-converts to reactive text
        return VNode.Text(str(self.read()))

    fn toggle(inout self):
        # For Signal[Bool] — convenience method
        self.write(not self.peek())
```

**Short creation function (`src/signals/signal.mojo`):**

```text
fn signal[T](initial: T) -> Signal[T]:
    # T is inferred from the argument — signal(0), signal("hello"), signal(True)
    return use_signal(fn() -> T: return initial)
```

**Mojo side (`src/signals/store.mojo`):**

```text
struct SignalStore:
    # Global slab of signal data
    # Each entry: { value: T (type-erased), subscribers: HashSet[ReactiveContextId] }
    ...
```

**Export functions for testing:**

- `signal_create_i32(initial: i32) -> u32` — returns signal ID
- `signal_read_i32(id: u32) -> i32`
- `signal_write_i32(id: u32, value: i32)`
- `signal_subscriber_count(id: u32) -> u32`
- `signal_iadd_i32(id: u32, rhs: i32)` — test `+=` operator
- `signal_call_i32(id: u32) -> i32` — test `__call__` shorthand

**Tests (`test/signals/signal.test.ts`):**

*Basic:*

- `signal(0)` creates signal, read returns 0
- `signal("hello")` — type inference works for String
- `signal(True)` — type inference works for Bool
- Write new value, read returns new value
- Create 100 signals, each holds independent value

*Ergonomic API:*

- `count()` returns same value as `count.read()` (callable shorthand)
- `count += 1` increments value (operator overloading)
- `count -= 1` decrements value
- `count.set(42)` is alias for `count.write(42)`
- `flag.toggle()` flips a `Signal[Bool]`
- Signal used as `Renderable` child → produces text VNode with string value

*Subscription tracking:*

- Read signal inside a reactive context → context is subscribed
- `count()` inside reactive context → context is subscribed (same as read)
- Read signal outside any context → no subscription (no crash)
- Write signal → all subscribed contexts marked dirty
- `count += 1` → same subscribers notified as `count.write()`
- Peek signal → context is NOT subscribed
- Unsubscribe context → write no longer marks it dirty

*Edge cases:*

- Read after write in same turn → returns new value
- Multiple reads → only one subscription (idempotent)
- `count += 1` then `count()` in same turn → returns incremented value
- Signal with large value (1KB string)
- Drop signal → subscribers cleaned up

### 1.2 Reactive Context

The reactive context tracks which signals are read during a scope's execution.

**Mojo side (`src/signals/context.mojo`):**

```text
struct ReactiveContext:
    var id: UInt32
    var subscriptions: HashSet[UInt32]  # signal IDs we're subscribed to

    fn subscribe(inout self, signal_subscribers: inout HashSet[UInt32]):
        signal_subscribers.insert(self.id)
        self.subscriptions.insert(signal_id)

    fn mark_dirty(self) -> Bool:
        # Mark the owning scope as dirty, return True if still alive
        ...
```

**Mojo side (`src/signals/runtime.mojo`):**

```text
# Thread-local (well, WASM is single-threaded) current context
var current_context: Optional[ReactiveContextId] = None

fn with_context[T](ctx: ReactiveContextId, f: fn() -> T) -> T:
    let prev = current_context
    current_context = ctx
    let result = f()
    current_context = prev
    return result
```

**Tests (`test/signals/context.test.ts`):**

- Set current context, read signal → context subscribed
- No current context, read signal → no subscription
- Nested contexts → inner context subscribes, not outer
- Context cleanup: destroy context → unsubscribed from all signals

### 1.3 Memo (Computed Values)

A `Memo[T]` is a derived signal that lazily recomputes when its input signals change.

**Mojo side (`src/signals/memo.mojo`):**

```text
struct Memo[T]:
    var signal: Signal[T]          # the cached output
    var context: ReactiveContext    # tracks input signals
    var compute: fn() -> T         # recomputation function
    var dirty: Bool

    fn read(self) -> T:
        if self.dirty:
            self.recompute()
        return self.signal.peek()  # peek to avoid double-subscription
```

**Export functions:**

- `memo_create_i32(compute_fn_id: u32) -> u32` — returns memo ID
- `memo_read_i32(id: u32) -> i32`

**Tests (`test/signals/memo.test.ts`):**

- Create memo that doubles a signal → reads correct initial value
- Update input signal → memo recomputes
- Read memo twice without change → compute runs only once (cached)
- Chain: signal → memo1 → memo2 → correct propagation
- Diamond dependency: two memos read same signal → no double-compute of downstream

### 1.4 Effect

An effect is a side-effectful callback that runs after rendering when its signal dependencies change.

**Mojo side (`src/signals/effect.mojo`):**

```text
struct Effect:
    var context: ReactiveContext
    var callback: fn()
    var scope: ScopeId
```

**Tests (`test/signals/effect.test.ts`):**

- Create effect → runs immediately once
- Update signal → effect runs again
- Effect reads two signals → runs when either changes
- Effect runs AFTER scope re-render (not during)
- Destroy scope → effect cleaned up, no further runs

---

## Phase 2 — Scopes & Components

> **Goal:** Components are functions that return VNodes. Each instance gets a scope.
>
> **Depends on:** Phase 1 (signals for state), Phase 0.2 (Slab for scope storage)

### 2.1 ScopeId & Scope State

**Mojo side (`src/scope/scope.mojo`):**

```text
alias ScopeId = UInt32

struct ScopeState:
    var id: ScopeId
    var height: UInt32                  # depth in component tree
    var parent: Optional[ScopeId]
    var context: ReactiveContext         # for signal subscription tracking
    var dirty: Bool
    var last_rendered: Optional[VNode]   # previous render output
    var hooks: DynArray[AnyHookState]    # hook storage (for use_signal, use_memo, etc.)
```

**Mojo side (`src/scope/slab.mojo`):**

```text
struct ScopeArena:
    var scopes: Slab[ScopeState]

    fn new_scope(parent: Optional[ScopeId], component: ComponentFn) -> ScopeId: ...
    fn get(id: ScopeId) -> ref ScopeState: ...
    fn remove(id: ScopeId): ...
```

**Tests (`test/scope/scope.test.ts`):**

- Create scope, verify ID allocated
- Scope tracks height correctly (root = 0, child = 1, grandchild = 2)
- Destroy scope → freed from slab
- Scope dirty flag: initially false, set to true, reset after render

### 2.2 Component Functions

Components are just functions that return `Element`. No decorator, no class, no trait — just a function. Using `def` enables implicit return for the last expression.

**Mojo side (`src/component/component.mojo`):**

```text
# Type alias for conciseness
comptime Element = VNode

# A component is a function pointer with type-erased props
alias ComponentFn = fn() -> Element

struct VComponent:
    var name: StringLiteral
    var render_fn: ComponentFn
    var props: AnyProps
```

**Example components:**

```text
# Minimal component — just a function
def hello() -> Element:
    h1("Hello, World!")

# Component with state
def counter() -> Element:
    var count = signal(0)
    div(
        h1("Count: ", count),
        button(onclick=fn(_): count += 1, "+"),
    )

# Component with props — props are just function arguments
def greeting(name: String) -> Element:
    h1("Hello, ", name, "!")
```

**Tests (`test/component/component.test.ts`):**

- Register a component, invoke its render function, get an Element back
- Component with props: pass different props, get different Elements
- Component returning nested elements
- `def` component with implicit return works correctly

### 2.3 Hooks (signal, memo, effect)

Hooks are how components create and access reactive state. They use positional indexing within the scope's hook array (same pattern as React/Dioxus). **Names are kept as short as possible** — `signal()` not `use_signal()`, `memo()` not `use_memo()`.

**Mojo side (`src/hooks/`):**

```text
fn signal[T](initial: T) -> Signal[T]:
    # Short form — T inferred, initial value passed directly
    # First render: create signal, store in scope hooks
    # Subsequent renders: return existing signal from hooks
    ...

fn use_signal[T](initial: fn() -> T) -> Signal[T]:
    # Long form — for lazy initialization
    ...

fn memo[T](compute: fn() -> T) -> Memo[T]:
    # Derived value, recomputes when dependencies change
    ...

fn effect(callback: fn()):
    # Side effect, runs when dependencies change
    ...
```

**Tests (`test/hooks/hooks.test.ts`):**

- `signal(0)` returns same signal across re-renders (stable identity)
- `signal(0)` initial value only evaluated once
- `signal(0)` — short form works identically to `use_signal(fn(): return 0)`
- Two `signal()` calls in same component → two independent signals
- `memo(fn(): count() * 2)` recomputes when count changes
- `memo()` returns cached value when dependencies unchanged
- `memo()` stable across re-renders

---

## Phase 3 — Templates & VNodes

> **Goal:** Define how UI structure is represented, with static templates for performance.
>
> **Depends on:** Phase 2 (components produce VNodes)

### 3.1 Template Definition

A template is the static structure of a piece of UI, compiled once, instantiated many times. In Tier 1, templates are created at runtime by the builder and cached. In Tier 2, templates are `comptime` constants (see [rsx! vs Mojo Parameters](#rsx-vs-mojo-parameters--compile-time-ui)).

**Mojo side (`src/vdom/template.mojo`):**

```text
struct Template:
    var name: StringLiteral         # unique ID (e.g. file:line:col)
    var roots: DynArray[TemplateNode]

# A node in a template (static structure)
variant TemplateNode:
    Element:
        tag: StringSlice
        namespace: Optional[StringSlice]
        attrs: DynArray[TemplateAttribute]
        children: DynArray[TemplateNode]
    Text:
        text: StringSlice
    Dynamic:
        index: UInt32               # index into VNode's dynamic_nodes array
    DynamicText:
        index: UInt32               # index into VNode's dynamic_text array
```

**Mojo side (`src/vdom/template_registry.mojo`):**

```text
struct TemplateRegistry:
    var templates: HashMap[StringSlice, Template]
    var next_id: UInt32

    fn register(template: Template) -> UInt32: ...
    fn get(id: UInt32) -> ref Template: ...
```

**Compile-time template helpers (`src/vdom/comptime_templates.mojo`):**

Parametric `comptime` values for Tier 2 template construction:

```text
# Compile-time template node factories
comptime div_node[N: Int](children: StaticArray[TemplateNode, N]) : TemplateNode =
    TemplateNode.Element(tag="div", children=children)

comptime h1_node[N: Int](children: StaticArray[TemplateNode, N]) : TemplateNode =
    TemplateNode.Element(tag="h1", children=children)

comptime button_node[N: Int](children: StaticArray[TemplateNode, N]) : TemplateNode =
    TemplateNode.Element(tag="button", children=children)

# ... one per HTML tag

comptime text_slot[idx: Int]() : TemplateNode =
    TemplateNode.DynamicText(index=idx)

comptime node_slot[idx: Int]() : TemplateNode =
    TemplateNode.Dynamic(index=idx)

comptime static_text[s: StringLiteral]() : TemplateNode =
    TemplateNode.Text(text=s)

comptime static_attr[name: StringLiteral, value: StringLiteral]() : TemplateAttribute =
    TemplateAttribute.Static(name=name, value=value)

comptime dynamic_attr[idx: Int]() : TemplateAttribute =
    TemplateAttribute.Dynamic(index=idx)
```

These are the building blocks for Tier 2's compile-time template definition. They compose naturally:

```text
# A complete compile-time template
comptime counter_tmpl = Template(
    name = "counter",
    roots = StaticArray(div_node(StaticArray(
        h1_node(StaticArray(text_slot[0]())),
        button_node(StaticArray(static_text["Increment"]())),
    ))),
)
```

**Tests (`test/vdom/template.test.ts`):**

*Tier 1 (runtime):*

- Register a template, retrieve by ID
- Template with static elements only
- Template with dynamic slots (placeholders)
- Template with mixed static and dynamic children
- Same builder call twice → template cached and reused (same ID)

*Tier 2 (compile-time):*

- `comptime` template has correct structure (export introspection functions)
- `comptime` template node factories produce correct tag names
- `text_slot[0]` and `text_slot[1]` produce different indices
- Nested `comptime` template construction: div containing ul containing li items
- Two components using same `comptime` template → share template ID in registry

### 3.2 VNode (Rendered Output)

A VNode is the output of a component's render function. It references a template and fills in the dynamic parts.

**Mojo side (`src/vdom/vnode.mojo`):**

```text
variant VNode:
    # Static template instantiation with dynamic values
    TemplateRef:
        template_id: UInt32
        dynamic_nodes: DynArray[DynamicNode]
        dynamic_attrs: DynArray[DynamicAttr]
        key: Optional[StringSlice]

    # Plain text
    Text:
        content: String

    # A placeholder for conditional/suspended content
    Placeholder:
        id: ElementId

    # A nested component
    Component:
        component: VComponent
        scope: Optional[ScopeId]     # filled in after mount

    # Multiple adjacent nodes (fragment)
    Fragment:
        children: DynArray[VNode]

variant DynamicNode:
    Text: { value: String }
    Component: { component: VComponent }
    Fragment: { children: DynArray[VNode] }
    Placeholder: {}

struct DynamicAttr:
    var name: StringSlice
    var namespace: Optional[StringSlice]
    var value: AttributeValue
    var element_id: ElementId        # which element in the template this attr belongs to

variant AttributeValue:
    Text: String
    Int: Int64
    Float: Float64
    Bool: Bool
    EventHandler: { handler_id: UInt32 }
    None
```

**Tests (`test/vdom/vnode.test.ts`):**

- Create a TemplateRef VNode, verify template_id and dynamic counts
- Create a Text VNode, verify content
- Create nested VNodes (component containing elements)
- VNode with key
- VNode with event handler attribute
- VNode with mixed attribute types (string, bool, int)

### 3.3 Builder API (DSL)

Since Mojo doesn't have proc macros like Rust's `rsx!`, we provide an ergonomic builder API. This is the **Tier 1** approach — fully dynamic, simple, correct. The API follows the [Ergonomics-First API Design](#ergonomics-first-api-design) rules: bare strings as children, keyword attributes, operator overloading on signals.

**Mojo side (`src/vdom/builder.mojo` + `src/vdom/tags.mojo`):**

```text
# What developers write day-to-day — concise, no wrappers
def counter_view() -> Element:
    var count = signal(0)
    div(class_="counter",
        h1("Count: ", count),
        button(onclick=fn(_): count += 1, "+"),
        button(onclick=fn(_): count -= 1, "-"),
    )
```

**Tag functions** accept `*children: Renderable` (bare strings, signals, or elements) and keyword attributes:

```text
# Signature pattern for all tag functions
fn div(*children: Renderable, class_: String = "", id_: String = "",
       style: String = "", onclick: Optional[EventHandler] = None,
       ...) -> Element:
    ...
```

Tag helpers: `div`, `span`, `p`, `h1`–`h6`, `ul`, `li`, `button`, `input`, `form`, `a`, `img`, `table`, `tr`, `td`, `th`, `section`, `header`, `footer`, `nav`, `main_`

Attributes are keyword arguments (underscore suffix for reserved words): `class_`, `id_`, `style`, `href`, `src`, `type_`, `value`, `placeholder`, `disabled`, `checked`, `for_`, `name`

Events are keyword arguments: `onclick`, `oninput`, `onchange`, `onsubmit`, `onkeydown`, `onkeyup`, `onfocus`, `onblur`, `onmousemove`

**Tier 2 alternative (`src/vdom/builder.mojo`):**

For performance-critical components, developers can opt into compile-time templates. The ergonomic API stays the same — only the template definition is explicit:

```text
# Tier 2: Compile-time template + runtime dynamic values
def counter_view_optimized() -> Element:
    comptime tmpl = Template("counter", StaticArray(
        div_node(StaticArray(
            h1_node(StaticArray(text_slot[0]())),
            button_node(StaticArray(static_text["+"]())),
        )),
    ))

    var count = signal(0)
    Element.from_template[tmpl](
        dynamic_texts = DynArray(str(count())),
        dynamic_attrs = DynArray(EventAttr("onclick", fn(_): count += 1)),
    )
```

Both tiers produce the same Element types and work with the same diff engine. Tier 2 simply skips runtime template construction and enables the diff engine to skip static node comparison.

**Compile-time children with `@parameter for`:**

```text
fn static_nav[N: Int](labels: StaticArray[StringLiteral, N]) -> Element:
    var items = DynArray[Element](capacity=N)
    @parameter
    for i in range(N):
        items.push(li(labels[i]))
    return ul_from(items)

# Usage — loop unrolled at compile time for N=3:
var nav = static_nav(StaticArray("Home", "About", "Contact"))
```

**Tests (`test/vdom/builder.test.ts`):**

*Renderable trait:*

- Bare `StringLiteral` "hello" → text VNode
- Bare `String` → text VNode
- `Signal[Int]` as child → reactive text VNode with string value
- `Signal[String]` as child → reactive text VNode
- `Element` as child → passed through unchanged
- Mixed children: `div("text", count, p("nested"))` → correct VNode tree

*Tag helpers:*

- Each tag helper produces correct tag name
- `div(class_="x")` → element with class attribute
- `input(type_="text", value="hello")` → correct attributes
- `button(onclick=handler, "Click")` → event + text child
- Nesting: `div(ul(li("A"), li("B")))` → correct tree
- Builder produces valid structures (smoke test all 20+ tags)

*Ergonomic patterns:*

- `h1("Count: ", count)` with Signal[Int] → two children (text + reactive text)
- `div(class_="x", id_="y", "child")` → attributes + child work together
- Keyword-only attributes don't conflict with positional children
- `@parameter for` with known N produces correct child count
- Tier 1 and Tier 2 builder produce semantically equivalent Elements

---

## Phase 4 — Mutations & Diffing

> **Goal:** Diff two VNode trees and emit stack-based mutations.
>
> **Depends on:** Phase 3 (VNodes), Phase 0.4 (mutation buffer protocol)

### 4.1 MutationWriter

The Mojo equivalent of Dioxus's `WriteMutations` trait. Writes mutations to the shared buffer.

**Mojo side (`src/mutations/writer.mojo`):**

```text
struct MutationWriter:
    var buffer: DynArray[UInt8]

    fn append_children(inout self, id: ElementId, m: UInt32): ...
    fn assign_id(inout self, path: DynArray[UInt8], id: ElementId): ...
    fn create_placeholder(inout self, id: ElementId): ...
    fn create_text_node(inout self, text: StringSlice, id: ElementId): ...
    fn load_template(inout self, tmpl_id: UInt32, index: UInt32, id: ElementId): ...
    fn replace_with(inout self, id: ElementId, m: UInt32): ...
    fn replace_placeholder(inout self, path: DynArray[UInt8], m: UInt32): ...
    fn insert_after(inout self, id: ElementId, m: UInt32): ...
    fn insert_before(inout self, id: ElementId, m: UInt32): ...
    fn set_attribute(inout self, name: StringSlice, ns: Optional[StringSlice], value: AttributeValue, id: ElementId): ...
    fn set_text(inout self, text: StringSlice, id: ElementId): ...
    fn new_event_listener(inout self, name: StringSlice, id: ElementId): ...
    fn remove_event_listener(inout self, name: StringSlice, id: ElementId): ...
    fn remove_node(inout self, id: ElementId): ...
    fn push_root(inout self, id: ElementId): ...
    fn finalize(inout self): ...  # write End sentinel
```

**Tests (`test/mutations/writer.test.ts`):**

- Write each mutation type → read from JS → verify binary matches
- Write sequence of mutations → all decoded correctly in order
- Finalize adds End sentinel
- Empty buffer (just End) → 0 mutations on JS side

### 4.2 Diff Algorithm

The diff engine compares the previous and new VNode outputs of a scope and emits mutations.

**Mojo side (`src/mutations/diff.mojo`):**

Following Dioxus's approach:

1. **Same template** → diff dynamic nodes and dynamic attributes only
2. **Different template** → replace entirely
3. **Component** → re-render component, diff its output recursively
4. **Text** → set_text if changed
5. **Children lists** → keyed reconciliation (LIS-based)

```text
fn diff_scope(scope_id: ScopeId, writer: inout MutationWriter):
    let old = scope.last_rendered
    let new = scope.render()
    diff_node(old, new, writer)
    scope.last_rendered = new

fn diff_node(old: Element, new: Element, writer: inout MutationWriter):
    match (old, new):
        (TemplateRef, TemplateRef) if same template:
            diff_dynamic_nodes(old.dynamic_nodes, new.dynamic_nodes, writer)
            diff_dynamic_attrs(old.dynamic_attrs, new.dynamic_attrs, writer)
        (Text, Text):
            if old.content != new.content:
                writer.set_text(new.content, old.element_id)
        _:
            replace(old, new, writer)

fn diff_children(old: DynArray[VNode], new: DynArray[VNode], writer: inout MutationWriter):
    # Keyed: use key to match, LIS for minimal moves
    # Unkeyed: pairwise diff, create/remove at boundaries
    ...
```

**Tests (`test/mutations/diff.test.ts`):**

*No change:*

- Same template, same dynamic values → 0 mutations
- Same text → 0 mutations

*Text updates:*

- Text "hello" → "world" → 1 SetText mutation
- Text "a" → "" → 1 SetText mutation
- Text "" → "hello" → 1 SetText mutation

*Attribute updates:*

- Attribute value changed → 1 SetAttribute mutation
- Attribute added → 1 SetAttribute mutation
- Attribute removed → 1 SetAttribute with None value

*Template swap:*

- div → span (different template) → ReplaceWith mutations

*Children (unkeyed):*

- Append child → LoadTemplate + AppendChildren
- Remove last child → Remove
- Same count, different values → SetText/SetAttribute only

*Children (keyed):*

- `[A, B, C]` → `[A, C]` — remove B
- `[A, B, C]` → `[A, D, B, C]` — insert D
- `[A, B, C]` → `[C, A, B]` — move only, no create/destroy
- `[]` → `[A, B, C]` — create all
- `[A, B, C]` → `[]` — remove all
- Reverse 100 keyed items → only move mutations

*Component replacement:*

- Component A → Component B at same position → teardown A, mount B

*Edge cases:*

- Empty → populated (full mount)
- Populated → empty (full teardown)
- Single child ↔ fragment of children
- Deep nesting (50 levels) → no stack overflow
- Wide tree (500 siblings) → linear performance

*Properties (property-based tests):*

- `apply(diff(A, B), A) == B` for randomly generated trees
- `diff(A, A)` always produces 0 meaningful mutations
- Mutation count bounded: `|diff(A, B)| <= |A| + |B|`

### 4.3 Initial Render (Create)

Initial mount is a special case of diff where there's no "old" tree.

**Mojo side (`src/mutations/create.mojo`):**

```text
fn create_scope(scope_id: ScopeId, writer: inout MutationWriter) -> UInt32:
    let el = scope.render()
    scope.last_rendered = el
    return create_node(el, writer)

fn create_node(node: Element, writer: inout MutationWriter) -> UInt32:
    # Emit LoadTemplate, SetAttribute, CreateTextNode, etc.
    # Returns number of roots created (for AppendChildren)
    ...
```

**Tests (`test/mutations/create.test.ts`):**

- Create single element → LoadTemplate + AssignId mutations
- Create element with attributes → SetAttribute mutations
- Create element with text child → CreateTextNode or dynamic text
- Create element with event listener → NewEventListener mutation
- Create nested tree → correct nesting of mutations
- Create component → recursively creates its output

---

## Phase 5 — JS Interpreter (Renderer)

> **Goal:** The JS side reads mutation buffers and applies them to a real DOM.
>
> **Depends on:** Phase 0.4 (protocol), Phase 4.1 (mutation writer)

### 5.1 Mutation Reader

**JS side (`runtime/interpreter.ts`):**

```ts
class MutationReader {
  private view: DataView;
  private offset: number;

  next(): Mutation | null { ... }  // reads one mutation, advances offset
  readAll(): Mutation[] { ... }    // reads until End sentinel
}
```

**Tests (`test/interpreter/reader.test.ts`):**

- Decode each opcode from hand-crafted binary buffers
- Decode sequence of mixed opcodes
- Empty buffer → null
- End sentinel → stops reading
- Malformed buffer → throws, not infinite loop

### 5.2 Template Cache

**JS side (`runtime/templates.ts`):**

```ts
class TemplateCache {
  private cache: Map<number, DocumentFragment>;

  register(id: number, html: string): void;     // parse and cache
  instantiate(id: number): DocumentFragment;     // clone from cache
}
```

The first time a template is used, the JS side receives the static HTML structure and caches a `DocumentFragment`. Subsequent uses clone the fragment (fast).

**Tests (`test/interpreter/templates.test.ts`):**

- Register template, instantiate → correct DOM structure
- Instantiate twice → two independent DOM fragments
- Template with nested elements
- Template with static attributes

### 5.3 DOM Interpreter

The stack machine that applies mutations to the real DOM.

**JS side (`runtime/interpreter.ts`):**

```ts
class Interpreter {
  private stack: Node[];                    // the virtual stack
  private nodes: Map<number, Node>;         // ElementId → DOM Node
  private templates: TemplateCache;
  private root: Element;

  applyMutations(buffer: ArrayBuffer): void {
    const reader = new MutationReader(buffer);
    let mutation: Mutation | null;
    while ((mutation = reader.next()) !== null) {
      this.apply(mutation);
    }
  }

  private apply(m: Mutation): void {
    switch (m.op) {
      case Op.PushRoot:
        this.stack.push(this.nodes.get(m.id)!);
        break;
      case Op.AppendChildren:
        const parent = this.nodes.get(m.id)!;
        for (let i = 0; i < m.count; i++) {
          parent.appendChild(this.stack.pop()!);
        }
        break;
      case Op.CreateTextNode:
        const text = document.createTextNode(m.text);
        this.nodes.set(m.id, text);
        this.stack.push(text);
        break;
      // ... etc
    }
  }
}
```

**Tests (`test/interpreter/interpreter.test.ts`):**

Using `deno-dom` or a mock DOM:

*Basic operations:*

- CreateTextNode → text node exists with correct content
- LoadTemplate → DOM fragment cloned and on stack
- SetAttribute → element has attribute
- SetText → text content updated
- Remove → node removed from parent
- PushRoot + AppendChildren → child attached to parent

*Sequences:*

- Full mount: LoadTemplate + AssignId + SetAttribute + CreateTextNode + AppendChildren → complete DOM tree
- Update: PushRoot + SetText → text updated in place
- Replace: PushRoot + CreateTextNode + ReplaceWith → old node replaced

*Stack correctness:*

- Multiple PushRoot + AppendChildren → correct parent-child relationships
- Nested templates → stack unwinds correctly

*Edge cases:*

- Remove node that has children → entire subtree removed
- SetAttribute with empty value
- CreateTextNode with empty string
- Apply same mutations twice → idempotent (no duplicates due to ID tracking)

---

## Phase 6 — Events

> **Goal:** DOM events flow from JS back to Mojo signal writes.
>
> **Depends on:** Phase 5 (interpreter), Phase 1 (signals)

### 6.1 Event Handler Registry

**Mojo side (`src/events/registry.mojo`):**

```text
struct HandlerRegistry:
    var handlers: Slab[EventHandler]

    fn register(handler: EventHandler) -> UInt32: ...
    fn dispatch(id: UInt32, event_data: Event): ...
    fn remove(id: UInt32): ...

struct EventHandler:
    var callback: fn(Event)
    var scope: ScopeId

# Short name — web devs write fn(e): e.value, not fn(e: EventData): e.value
variant Event:
    Click: { client_x: Float64, client_y: Float64 }
    Input: { value: String }
    KeyDown: { key: String, key_code: UInt32 }
    KeyUp: { key: String, key_code: UInt32 }
    MouseMove: { client_x: Float64, client_y: Float64 }
    Focus
    Blur
    Submit
    Change: { value: String }
```

**Export functions:**

- `dispatch_event(element_id: u32, event_type: u8, data_ptr: u64, data_len: u32)`
- Called by JS when a DOM event fires

**Tests (`test/events/registry.test.ts`):**

- Register handler → returns ID
- Dispatch to handler → callback invoked
- Remove handler → dispatch is no-op
- Register 100 handlers, remove 50, dispatch to remaining → correct

### 6.2 Event Delegation (JS Side)

**JS side (`runtime/events.ts`):**

```ts
class EventBridge {
  private root: Element;
  private interpreter: Interpreter;
  private wasmDispatch: (elementId: number, eventType: number, dataPtr: bigint, dataLen: number) => void;

  install(): void {
    // Single click listener on root
    this.root.addEventListener('click', (e) => {
      const target = e.target as Element;
      const elementId = this.findElementId(target);
      if (elementId !== null) {
        this.dispatchToWasm(elementId, EventType.Click, e);
      }
    }, true);  // capture phase for delegation
    // ... same for input, keydown, etc.
  }
}
```

**Tests (`test/events/delegation.test.ts`):**

- Click on element with handler → WASM dispatch called with correct element ID
- Click on element without handler → no dispatch, no error
- Click on nested child → bubbles to closest handler
- Input event → value string serialized correctly to event buffer
- Keyboard event → key and keyCode correct
- Register then unregister listener → no dispatch after removal

### 6.3 Full Event Flow

```text
User clicks button
  → JS EventBridge captures click
  → Writes event data to event buffer in shared memory
  → Calls WASM dispatch_event(element_id, CLICK, data_ptr, data_len)
  → Mojo runs: fn(_): count += 1
  → Signal.__iadd__ calls write() internally
  → Signal notifies subscribers → scopes marked dirty
  → JS calls WASM flush_updates(mutation_buffer_ptr) -> mutation_count
  → Dirty scopes re-rendered, mutations emitted
  → JS reads mutation buffer, applies to DOM
```

**Tests (`test/events/flow.test.ts`):**

- `onclick=fn(_): count += 1` → scope dirty → flush produces SetText mutation
- `oninput=fn(e): name.set(e.value)` → text node updates with input value
- Multiple events in sequence → each produces correct mutations
- Event handler that doesn't change state → flush produces 0 mutations

---

## Phase 7 — First App (End-to-End)

> **Goal:** A real counter app running in a browser.
>
> **Depends on:** Phases 0–6

### 7.1 App Shell

**Mojo side (`src/app.mojo`):**

```text
struct App:
    var vdom: VirtualDom
    var mutation_buffer: MutationWriter
    var element_ids: ElementIdAllocator

    fn init(root_component: ComponentFn) -> App: ...
    fn rebuild(inout self) -> (ptr: UInt64, len: UInt32): ...
    fn flush_updates(inout self) -> (ptr: UInt64, len: UInt32): ...
    fn handle_event(inout self, element_id: UInt32, event_type: UInt8, data_ptr: UInt64, data_len: UInt32): ...
```

**Export functions:**

- `app_init() -> u32` — initialize app, return app ID
- `app_rebuild(app_id: u32, buffer_ptr: u64) -> u32` — initial mount, returns mutation count
- `app_handle_event(app_id: u32, element_id: u32, event_type: u8, data_ptr: u64, data_len: u32)`
- `app_flush(app_id: u32, buffer_ptr: u64) -> u32` — flush pending updates, returns mutation count

**JS side (`runtime/app.ts`):**

```ts
export async function createApp(wasmPath: string, rootElement: Element) {
  const wasm = await instantiate(wasmPath);
  const interpreter = new Interpreter(rootElement);
  const events = new EventBridge(rootElement, wasm);

  // Initial mount
  const mutationCount = wasm.app_rebuild(appId, bufferPtr);
  interpreter.applyMutations(buffer);

  // Event loop
  events.onEvent((elementId, eventType, data) => {
    wasm.app_handle_event(appId, elementId, eventType, dataPtr, dataLen);
    const count = wasm.app_flush(appId, bufferPtr);
    if (count > 0) {
      interpreter.applyMutations(buffer);
    }
  });
}
```

**Files:**

- `examples/counter/app.mojo` — counter component
- `examples/counter/index.html` — minimal HTML host page
- `examples/counter/main.ts` — JS entry point

### 7.2 Counter App

```text
# examples/counter/app.mojo

def counter() -> Element:
    var count = signal(0)

    div(class_="counter",
        h1("Count: ", count),
        button(onclick=fn(_): count += 1, "+"),
        button(onclick=fn(_): count -= 1, "-"),
    )
```

That's it. 8 lines for a fully reactive counter. Compare to React (12 lines) or Dioxus (10 lines with `rsx!`).

**Tests (`test/app/counter.test.ts`):**

- Initial render → DOM has "Count: 0"
- Simulate click on increment → DOM has "Count: 1"
- Click 10 times → DOM has "Count: 10"
- Click decrement → DOM has "Count: 9"
- Verify minimal mutations per click (only text node change)

### 7.3 Todo List App

**Tests (`test/app/todolist.test.ts`):**

- Initial render → empty list
- Add item → list has 1 child with correct text
- Add 3 items → list has 3 children
- Toggle item complete → class changes on that item
- Remove item → correct item removed (keyed by ID)
- Clear all → list empty
- Verify keyed reconciliation: items maintain DOM identity through reorder

---

## Phase 8 — Advanced Features

> **Goal:** Feature completeness for a real framework.
>
> **Depends on:** Phase 7

### 8.1 Conditional Rendering

```text
def maybe_content(show: Signal[Bool]) -> Element:
    if show(): div("Visible!")
    else: placeholder()
```

**Tests:**

- Condition true → element present in DOM
- Condition false → placeholder in DOM (not absent — needed for diffing)
- Toggle true → false → element replaced with placeholder (1 Remove + 1 CreatePlaceholder)
- Toggle false → true → placeholder replaced with element
- Nested conditionals

### 8.2 Dynamic Lists with Keys

```text
def item_list(items: Signal[List[TodoItem]]) -> Element:
    ul(
        each(items(), fn(item) -> Element:
            li(key=str(item.id), item.text)
        ),
    )
```

**Tests:**

- Render list of 100 keyed items
- Append → 1 LoadTemplate + AppendChildren
- Prepend → LoadTemplate + InsertBefore
- Remove from middle → 1 Remove
- Reverse → Move mutations only, no create/destroy
- Shuffle → minimal mutation count (LIS optimization)
- Empty to populated and back

### 8.3 Context (Dependency Injection)

Provide values at a parent scope, consume in any descendant without prop drilling.

```text
def provide_theme(children: Element) -> Element:
    provide_context(Theme(dark=True))
    children

def themed_button() -> Element:
    var theme = context[Theme]()
    button(class_="btn-dark" if theme.dark else "btn-light", "Click me")
```

Note: `context[Theme]()` not `use_context[Theme]()` — shorter, same pattern as `signal()` vs `use_signal()`.

**Tests:**

- Provide at root → consume in deeply nested child
- Update provided value → all consumers re-render
- Multiple contexts → each consumer gets correct type
- Missing provider → panic or default

### 8.4 Error Boundaries

**Tests:**

- Child component panics → parent catches, renders fallback
- Recovery: error clears → child re-mounts
- Nested boundaries → innermost catches

### 8.5 Suspense

Async components that show a fallback while loading.

**Tests:**

- Suspense with pending async → shows fallback
- Async resolves → fallback replaced with content
- Nested suspense boundaries

---

## Phase 9 — Performance & Polish

> **Goal:** Prove competitive performance and production readiness.
>
> **Depends on:** Phase 8

### 9.1 js-framework-benchmark

Implement the standard [js-framework-benchmark](https://github.com/nicknisi/nicknisi.github.io) operations:

- Create 1,000 rows
- Create 10,000 rows
- Append 1,000 rows
- Update every 10th row
- Select row (highlight)
- Swap rows
- Remove row
- Clear all rows

**Targets:**

- Initial render: < 100ms for 1,000 rows
- Partial update: < 20ms
- Memory: < 10MB for 10,000 rows

### 9.2 Memory Management

- Audit all allocation paths for leaks
- Arena reset between render cycles
- Profile memory growth over 10,000 mount/unmount cycles
- Signal cleanup when scopes are destroyed

**Tests (`test/bench/memory.test.ts`):**

- Mount/unmount 1,000 cycles → memory bounded
- 10,000 signals created and destroyed → no leak
- Rapid state updates (1,000 in sequence) → memory stable

### 9.3 Tier 2 Compile-Time Template Optimization

Upgrade the builder to support `comptime` templates (see [rsx! vs Mojo Parameters](#rsx-vs-mojo-parameters--compile-time-ui)).

- Implement `VNode.from_template[comptime tmpl]()` path
- Diff engine: when both old and new VNodes reference the same `comptime` template, skip static structure comparison entirely — only diff dynamic slots
- Measure speedup: Tier 2 vs Tier 1 for 1,000-row table
- `@parameter for` loop unrolling for static child lists

**Tests (`test/bench/comptime.test.ts`):**

- Tier 2 template diff only checks dynamic slots (verify with mutation count)
- Tier 2 renders faster than Tier 1 for same component
- `comptime` template with 0 dynamic slots → 0 mutations on re-render
- `comptime` template with N dynamic slots → at most N mutations on re-render
- `@parameter for` unrolled list vs runtime list → same output, fewer allocations

### 9.4 Mutation Optimization

- Coalesce redundant mutations (double SetAttribute → keep last)
- Skip no-op mutations (SetText with same value)
- Batch signal notifications (multiple writes in one handler → single re-render)

**Tests:**

- Set same attribute twice in one handler → 1 mutation
- Write to signal twice → 1 re-render
- Measure mutation count for benchmark operations

### 9.5 Developer Experience

- Clear error messages for common mistakes (e.g., hook called conditionally)
- Debug mode: log VNode tree to console
- Debug mode: log mutation stream
- Performance profiling hooks

---

## Test Strategy

### Test Pyramid

```txt
           ╱╲
          ╱  ╲        E2E tests (counter, todo list — browser)
         ╱    ╲       ~10% of tests
        ╱──────╲
       ╱        ╲     Integration tests (Mojo → mutations → DOM)
      ╱          ╲    ~20% of tests
     ╱────────────╲
    ╱              ╲   Bridge tests (mutation encode/decode, events)
   ╱                ╲  ~20% of tests
  ╱──────────────────╲
 ╱                    ╲  Unit tests (signals, diff, collections)
╱                      ╲ ~50% of tests
╱────────────────────────╲
```

### Test Infrastructure

**Existing (keep):**

- `test/test_*.mojo` — primary WASM test suite (arithmetic, signals, templates, etc.)
- `test-js/harness.ts` — assert, assertClose, assertNaN, suite, summary
- `test-js/run.ts` — JS test runner entry point
- `test-js/` — JS runtime integration tests (DOM interpreter, counter app, protocol)

**New additions:**

- `test-js/helpers/dom.ts` — `deno-dom` based DOM creation and assertion
- `test-js/helpers/signals.ts` — helpers to create/read/write signals via WASM exports
- `test-js/helpers/mutations.ts` — helpers to read and assert on mutation buffers
- `test-js/helpers/events.ts` — helpers to simulate events and verify dispatch

### Test Directory Structure

```txt
test/
├── run.ts                          # Entry point (existing + new suites)
├── harness.ts                      # Test harness (existing)
│
│   # Existing low-level interop tests (unchanged)
├── arithmetic.test.ts
├── strings.test.ts
├── sso.test.ts
├── unicode.test.ts
├── ... (all existing tests)
│
│   # Helpers
├── helpers/
│   ├── dom.ts                      # DOM creation / assertion
│   ├── signals.ts                  # Signal WASM export helpers
│   ├── mutations.ts                # Mutation buffer helpers
│   └── events.ts                   # Event simulation helpers
│
│   # Phase 0: Foundation
├── alloc.test.ts
├── collections.test.ts
├── element-id.test.ts
├── protocol.test.ts
│
│   # Phase 1: Signals & Reactivity
├── signals/
│   ├── signal.test.ts              # Signal create/read/write
│   ├── context.test.ts             # Reactive context subscription
│   ├── memo.test.ts                # Computed/derived values
│   └── effect.test.ts              # Side effects
│
│   # Phase 2: Scopes & Components
├── scope/
│   ├── scope.test.ts               # Scope lifecycle
│   └── hooks.test.ts               # use_signal, use_memo, use_effect
│
│   # Phase 3: Templates & VNodes
├── vdom/
│   ├── template.test.ts            # Template registration (Tier 1 + Tier 2)
│   ├── vnode.test.ts               # VNode creation
│   └── builder.test.ts             # DSL / tag helpers (Tier 1 + Tier 2)
│
│   # Compile-time feature tests
├── comptime/
│   ├── templates.test.ts           # comptime template construction
│   └── builder.test.ts             # @parameter for, @parameter if, Tier 2 builder
│
│   # Phase 4: Mutations & Diffing
├── mutations/
│   ├── writer.test.ts              # MutationWriter encoding
│   ├── create.test.ts              # Initial mount mutations
│   └── diff.test.ts                # Diff algorithm
│
│   # Phase 5: JS Interpreter
├── interpreter/
│   ├── reader.test.ts              # MutationReader decoding
│   ├── templates.test.ts           # Template cache
│   └── interpreter.test.ts         # Full DOM interpreter
│
│   # Phase 6: Events
├── events/
│   ├── registry.test.ts            # Handler registration
│   ├── delegation.test.ts          # Event delegation
│   └── flow.test.ts                # Full event → update flow
│
│   # Phase 7: End-to-end
├── app/
│   ├── counter.test.ts             # Counter app
│   └── todolist.test.ts            # Todo list app
│
│   # Phase 8: Advanced features
├── advanced/
│   ├── conditional.test.ts         # Conditional rendering
│   ├── lists.test.ts               # Keyed list reconciliation
│   ├── context.test.ts             # Dependency injection
│   └── error-boundary.test.ts      # Error boundaries
│
│   # Phase 9: Performance
└── bench/
    ├── memory.test.ts              # Memory usage
    ├── render.bench.ts             # Render performance
    ├── diff.bench.ts               # Diff algorithm perf
    ├── signals.bench.ts            # Signal throughput
    └── comptime.test.ts            # Tier 2 vs Tier 1 comparison
```

---

## Project Structure

### Mojo Source

```txt
src/
├── main.mojo                       # Existing exports (keep for interop regression tests)
│
├── alloc/
│   ├── arena.mojo                  # Region-based allocator
│   └── pool.mojo                   # Fixed-size object pool
│
├── collections/
│   ├── dynarray.mojo               # Growable array
│   ├── smallvec.mojo               # Stack-allocated small vector
│   ├── hashmap.mojo                # Hash map
│   ├── hashset.mojo                # Hash set
│   └── slab.mojo                   # Indexed arena with stable IDs
│
├── arena/
│   └── element_id.mojo             # ElementId type and allocator
│
├── signals/
│   ├── signal.mojo                 # Signal[T] — core reactive primitive + operator overloads
│   ├── store.mojo                  # Global signal storage (Slab-backed)
│   ├── context.mojo                # ReactiveContext — subscriber tracking
│   ├── runtime.mojo                # Current context thread-local, with_context
│   ├── memo.mojo                   # Memo[T] — derived/computed signals
│   └── effect.mojo                 # Effect — reactive side effects
│
├── scope/
│   ├── scope.mojo                  # ScopeState — per-component instance
│   └── slab.mojo                   # ScopeArena — scope storage
│
├── hooks/
│   ├── signal.mojo                 # signal() and use_signal() hooks
│   ├── memo.mojo                   # memo() hook
│   ├── effect.mojo                 # effect() hook
│   └── context.mojo                # context() / provide_context()
│
├── vdom/
│   ├── template.mojo               # Template, TemplateNode (static structure)
│   ├── template_registry.mojo      # Template storage
│   ├── comptime_templates.mojo     # Parametric comptime template node factories (Tier 2)
│   ├── vnode.mojo                   # VNode / Element, DynamicNode, AttributeValue
│   ├── renderable.mojo             # Renderable trait (String, Signal, VNode → child)
│   ├── builder.mojo                # Builder API / DSL (Tier 1 runtime + Tier 2 from_template)
│   └── tags.mojo                   # HTML tag helpers (div, span, p, ...)
│
├── mutations/
│   ├── writer.mojo                 # MutationWriter — binary encoding
│   ├── diff.mojo                   # Diff algorithm
│   └── create.mojo                 # Initial mount (create from scratch)
│
├── events/
│   ├── registry.mojo               # Handler registry and dispatch
│   └── types.mojo                  # Event variant (short name, not EventData)
│
├── bridge/
│   └── protocol.mojo               # Opcode constants, shared with JS
│
├── component/
│   ├── component.mojo              # VComponent, ComponentFn, Element alias
│   └── lifecycle.mojo              # Mount/update/unmount hooks
│
├── scheduler/
│   └── scheduler.mojo              # Dirty scope queue, render ordering
│
└── app.mojo                        # App entry point, rebuild, flush, handle_event
```

### JavaScript Runtime

```txt
runtime/
├── mod.ts                          # Entry point (existing, extended)
├── types.ts                        # WASM exports interface (existing, extended)
├── memory.ts                       # Allocator (existing, extended)
├── env.ts                          # Environment imports (existing)
├── strings.ts                      # String ABI (existing)
│
├── protocol.ts                     # Mutation + event opcodes (must match Mojo)
├── interpreter.ts                  # MutationReader + DOM stack machine
├── templates.ts                    # Template cache (DocumentFragment pool)
├── events.ts                       # Event delegation bridge (DOM → WASM)
├── nodes.ts                        # ElementId ↔ DOM Node registry
├── scheduler.ts                    # requestAnimationFrame batching
└── app.ts                          # createApp() — high-level API
```

---

## Open Questions

| # | Question | Phase | Notes |
|---|---|---|---|
| 1 | Can Mojo function pointers be stored in collections and called dynamically? | 1 | Core to signal callbacks and event handlers. If not, use dispatch table with integer IDs. |
| 2 | Does Mojo support recursive/self-referential types? | 3 | VNode children contain VNodes. May need indirection via UnsafePointer. |
| 3 | How to implement "current reactive context" in single-threaded WASM? | 1 | Global mutable state works (WASM is single-threaded). Use a module-level `var`. |
| 4 | Can `@parameter` enable compile-time template generation? | 9 | Tier 2 depends on this. `comptime` values should work for `Template` if all fields are compile-time-known. `@parameter for` requires `_StridedRangeIterator` (only `Int` induction vars). Test early, optimize late. |
| 5 | What is the overhead of WASM↔JS calls vs shared memory reads? | 5 | Determines whether mutation buffer is worth the complexity vs direct JS calls. Benchmark. |
| 6 | How does Mojo handle type erasure for heterogeneous hook storage? | 2 | Hooks array stores different types. May need `AnyValue` wrapper or union types. |
| 7 | Should we support `wasm32` in addition to `wasm64`? | 0 | Currently using wasm64. wasm32 has wider browser support but 4GB limit. |
| 8 | How to handle Mojo's ownership model for VNode tree sharing? | 4 | Diffing needs old + new tree simultaneously. Arena allocation may sidestep ownership issues. |
| 9 | Can we implement a Mojo decorator/macro for component declaration? | 7+ | `@component` decorator that generates boilerplate. Depends on Mojo metaprogramming support. |
| 10 | Can `comptime` values contain `DynArray` or only `StaticArray`? | 3 | Templates with `comptime` require all fields to be compile-time-known. May need `StaticArray` for `comptime` templates, `DynArray` for runtime templates. |
| 11 | Does variadic parameter homogeneity block any builder patterns? | 3 | All children are `Renderable` (trait), so homogeneous variadics should work. Verify with mixed String/Signal/Element children. |
| 12 | Can `@parameter fn` closures capture `Signal[T]` handles? | 1 | Critical for `onclick=fn(_): count += 1`. Signals are small structs (just a `UInt32` ID), so capturing should work. Test early. |
| 13 | Does Mojo support `__iadd__` and other augmented assignment dunders on structs? | 1 | Critical for `count += 1` ergonomics. If not supported, fall back to `count.set(count() + 1)`. |
| 14 | Can Mojo variadic runtime args (`*children`) accept trait-conforming heterogeneous types? | 3 | `*children: Renderable` where String, Signal, and Element all conform. If runtime variadics must be homogeneous like parameter variadics, need overloads per arity or a `children(...)` helper. |
| 15 | Does `def` implicit return work with the last expression being a function call? | 2 | `def counter() -> Element:` with `div(...)` as last line. Must verify this returns the Element. |

---

## Phase 10 — Modularization & Next Steps

> **Goal:** Improve codebase structure, reduce monolithic coupling, and prepare for future feature work.
>
> **Status:** In progress.

### 10.1 Extract App Modules (✅ Done)

The monolithic `src/main.mojo` (4,249 lines) has been refactored:

- **`src/apps/counter.mojo`** — `CounterApp` struct + lifecycle functions (`counter_app_init`, `counter_app_destroy`, `counter_app_rebuild`, `counter_app_handle_event`, `counter_app_flush`)
- **`src/apps/todo.mojo`** — `TodoApp` + `TodoItem` structs + lifecycle functions (`todo_app_init`, `todo_app_destroy`, `todo_app_rebuild`, `todo_app_flush`)
- **`src/apps/bench.mojo`** — `BenchmarkApp` + `BenchRow` structs + label generation + lifecycle functions (`bench_app_init`, `bench_app_destroy`, `bench_app_rebuild`, `bench_app_flush`)
- **`src/apps/__init__.mojo`** — Package re-exports

`src/main.mojo` is now 2,930 lines of thin `@export` wrappers and PoC arithmetic/string functions. All 790 tests pass unchanged.

### 10.2 Extract PoC Exports (✅ Done)

The original proof-of-concept arithmetic, string, and algorithm implementations have been extracted into a `src/poc/` package:

- **`src/poc/arithmetic.mojo`** — `poc_add_*`, `poc_sub_*`, `poc_mul_*`, `poc_div_*`, `poc_mod_*`, `poc_pow_*`, `poc_neg_*`, `poc_abs_*`, `poc_min_*`, `poc_max_*`, `poc_clamp_*`
- **`src/poc/bitwise.mojo`** — `poc_bitand_int32`, `poc_bitor_int32`, `poc_bitxor_int32`, `poc_bitnot_int32`, `poc_shl_int32`, `poc_shr_int32`
- **`src/poc/comparison.mojo`** — `poc_eq_int32`, `poc_ne_int32`, `poc_lt_int32`, `poc_le_int32`, `poc_gt_int32`, `poc_ge_int32`, `poc_bool_and`, `poc_bool_or`, `poc_bool_not`
- **`src/poc/algorithms.mojo`** — `poc_fib_*`, `poc_factorial_*`, `poc_gcd_int32`
- **`src/poc/strings.mojo`** — `poc_identity_*`, `poc_print_*`, `poc_return_*_string`, `poc_string_length`, `poc_string_concat`, `poc_string_repeat`, `poc_string_eq`
- **`src/poc/__init__.mojo`** — Package re-exports

`src/main.mojo` PoC section is now thin `@export` wrappers calling into `poc/` modules. All 602 Mojo tests and 790 JS tests pass unchanged.

### 10.3 Shared JS Runtime Deduplication (✅ Done)

The counter, todo, and bench examples each inlined the full WASM environment, mutation reader, and interpreter (~300 lines each). Extracted into a shared `examples/lib/` package:

- **`examples/lib/env.js`** (84 lines) — WASM memory management, `alignedAlloc`, `env` import object, `loadWasm()` helper
- **`examples/lib/protocol.js`** (139 lines) — `Op` constants + `MutationReader` class
- **`examples/lib/interpreter.js`** (182 lines) — DOM `Interpreter` class
- **`examples/lib/strings.js`** (37 lines) — Mojo String ABI `writeStringStruct()` helper
- **`examples/lib/boot.js`** (46 lines) — Re-exports + convenience helpers: `createInterpreter()`, `allocBuffer()`, `applyMutations()`

Examples now import from `../lib/boot.js` and contain only app-specific logic:

- **`examples/counter/main.js`** — 81 lines (was ~337)
- **`examples/todo/main.js`** — 194 lines (was ~553)
- **`examples/bench/main.js`** — 160 lines (was ~490 inline in HTML)
- **`examples/bench/index.html`** — Now uses external `<script type="module" src="main.js">` instead of inline `<script>`

All 602 Mojo tests and 790 JS tests pass unchanged.

### 10.4 Component Abstraction (✅ Done)

Each app previously hand-rolled scope/signal/template wiring. Extracted common infrastructure into reusable packages:

- **`src/component/app_shell.mojo`** (242 lines) — `AppShell` struct bundling Runtime, VNodeStore, ElementIdAllocator, and Scheduler. Provides lifecycle methods: `setup()`, `destroy()`, `mount()`, `diff()`, `finalize()`, `collect_dirty()`, `next_dirty()`, `dispatch_event()`, plus scope/signal helpers.
- **`src/component/lifecycle.mojo`** (184 lines) — Reusable orchestration: `mount_vnode()`, `mount_vnode_to()`, `diff_and_finalize()`, `diff_no_finalize()`, `create_no_finalize()`.
- **`src/component/__init__.mojo`** — Package re-exports.
- **`src/scheduler/scheduler.mojo`** (169 lines) — `Scheduler` struct: height-ordered dirty scope queue with `collect()`, `collect_one()`, `next()`, deduplication, insertion-sort by scope height.
- **`src/scheduler/__init__.mojo`** — Package re-exports.

`main.mojo` now exposes `shell_*` WASM exports (create, destroy, mount, diff, signals, scopes, dirty tracking, event dispatch) and `scheduler_*` exports. Tests: `test_component.mojo` (26 tests) and `test_scheduler.mojo` (11 tests). All 641 Mojo + 790 JS tests pass.

### 10.5 Ergonomic Builder API (✅ Done)

Implemented the Tier 1 builder DSL from the plan's [Ergonomics-First API Design](#ergonomics-first-api-design) section:

- **`Node` tagged union** (`vdom/dsl.mojo`): 6-variant type (`NODE_TEXT`, `NODE_ELEMENT`, `NODE_DYN_TEXT`, `NODE_DYN_NODE`, `NODE_STATIC_ATTR`, `NODE_DYN_ATTR`) enabling declarative element tree composition.
- **Leaf constructors**: `text()`, `dyn_text()`, `dyn_node()`, `attr()`, `dyn_attr()` — concise helpers replacing verbose `TemplateBuilder` push calls.
- **40 tag helpers**: `el_div()`, `el_span()`, `el_h1()`–`el_h6()`, `el_button()`, `el_input()`, `el_form()`, `el_ul()`, `el_li()`, `el_table()`, `el_tr()`, `el_td()`, `el_a()`, `el_img()`, `el_br()`, `el_hr()`, `el_pre()`, `el_code()`, `el_strong()`, `el_em()`, etc. — each with empty and `List[Node]` overloads.
- **`to_template()` / `to_template_multi()`**: Converts `Node` trees to `Template` via 3-pass recursive walk (static attrs → dynamic attrs → children), producing identical output to manual `TemplateBuilder` calls.
- **`VNodeBuilder`**: Ergonomic VNode construction with `add_dyn_text()`, `add_dyn_event()`, `add_dyn_text_attr()`, `add_dyn_int_attr()`, `add_dyn_bool_attr()`, `add_dyn_float_attr()`, `add_dyn_none_attr()`, `add_dyn_placeholder()`. Supports keyed VNodes.
- **Utility functions**: `count_nodes()`, `count_all_items()`, `count_dynamic_text_slots()`, `count_dynamic_node_slots()`, `count_dynamic_attr_slots()`, `count_static_attr_nodes()`.
- **Template equivalence verified**: DSL-built counter template matches manually-built template node-for-node (kinds, tags, child counts, dynamic slot counts, attribute counts).
- **WASM exports**: `dsl_node_*` (Node lifecycle), `dsl_vb_*` (VNodeBuilder), `dsl_to_template()`, and 15 self-contained `dsl_test_*` functions.
- **33 new Mojo tests + 69 new JS tests**. All 674 Mojo + 859 JS tests pass.

### 10.6 DSL-Based App Rewrite (✅ Done)

Converted all three apps from verbose `TemplateBuilder`/`VNodeStore` API to the ergonomic DSL (M10.5):

- **Counter app** (`apps/counter.mojo`): Template construction replaced with `el_div`/`el_span`/`el_button`/`dyn_text`/`dyn_attr` + `to_template()`. VNode construction replaced with `VNodeBuilder`. Removed imports of `TemplateBuilder`, `create_builder`, `destroy_builder`, `DynamicNode`, `DynamicAttr`, `AttributeValue`, and tag constants. Net reduction: 276 → 250 lines.
- **Todo app** (`apps/todo.mojo`): Both templates (`todo-app` and `todo-item`) converted to DSL. `build_app_vnode()` and `build_item_vnode()` use `VNodeBuilder` with `add_dyn_text()`, `add_dyn_event()`, `add_dyn_text_attr()`, `add_dyn_placeholder()`. Static attributes use `attr()`. Net reduction: 538 → 486 lines.
- **Bench app** (`apps/bench.mojo`): Row template (`bench-row`) converted to DSL. `build_row_vnode()` uses `VNodeBuilder` with `add_dyn_text()`, `add_dyn_text_attr()`, `add_dyn_event()`. Net reduction: 538 → 502 lines.
- **Template equivalence verified**: All existing 674 Mojo + 859 JS tests pass unchanged, confirming DSL-built apps produce identical template structures, mutation sequences, and DOM output to the original manual builder code.
- **No new tests required**: The comprehensive existing test suites (counter: 650 lines, todo: 786 lines, bench: 769 lines of JS tests) exercise the full round-trip and implicitly validate the DSL conversion.

### 10.7 AppShell Integration (✅ Done)

Refactored all three apps to use the `AppShell` abstraction (M10.4) instead of manually managing `Runtime`, `VNodeStore`, and `ElementIdAllocator` pointers:

- **Counter app** (`apps/counter.mojo`): Replaced 3 `UnsafePointer` fields (`runtime`, `store`, `eid_alloc`) with single `shell: AppShell` field. Init uses `app_shell_create()` instead of manual alloc/init of 3 subsystems. Destroy uses `shell.destroy()` instead of 8 lines of manual teardown. Scope/signal lifecycle uses `shell.create_root_scope()`, `shell.begin_render()`, `shell.end_render()`, `shell.use_signal_i32()`, `shell.read_signal_i32()`, `shell.peek_signal_i32()`. Mount uses `shell.mount()` replacing manual CreateEngine + append + finalize. Flush uses `shell.diff()` + `shell.finalize()` replacing manual DiffEngine + finalize. Event dispatch uses `shell.dispatch_event()`. Net reduction: 250 → 217 lines.
- **Todo app** (`apps/todo.mojo`): Same subsystem consolidation. Init/destroy simplified via AppShell. Signal helpers (`_bump_version`, `build_item_vnode`) use `shell.peek_signal_i32()` / `shell.write_signal_i32()`. Handler registration via `shell.runtime[0]`. VNodeBuilder and fragment operations via `shell.store`. Complex flush logic (empty↔populated transitions) still uses direct `CreateEngine`/`DiffEngine` with `shell.eid_alloc`, `shell.runtime`, `shell.store`. Net reduction: 486 → 484 lines.
- **Bench app** (`apps/bench.mojo`): Same pattern as todo. Init/destroy via AppShell. Signal/scope helpers via shell methods. Row VNode building via `shell.store`. Complex flush transitions via direct engine access through shell's subsystem pointers. Net reduction: 502 → 504 lines (formatting changes offset savings).
- **WASM exports** (`main.mojo`): Updated 8 exports that directly accessed `app[0].runtime` to use `app[0].shell` equivalents (`shell.has_dirty()`, `shell.peek_signal_i32()`, `shell.runtime` for `counter_rt_ptr`).
- **Total net reduction**: 1,238 → 1,205 lines across all three apps (−33 lines).
- **All 674 Mojo + 859 JS tests pass unchanged**, confirming that AppShell integration produces identical behavior.

### 10.8 Fragment Lifecycle Helpers (✅ Done)

Extracted the repeated fragment-based list flush logic from todo and bench apps into a reusable `FragmentSlot` struct and `flush_fragment()` lifecycle helper in `component/lifecycle.mojo`:

- **`FragmentSlot` struct** (`component/lifecycle.mojo`): Bundles the three pieces of state every fragment-based dynamic list needs: `anchor_id` (ElementId of the placeholder when empty), `current_frag` (VNode index of the current Fragment), and `mounted` (whether items are in the DOM). Implements `Copyable` and `Movable`. Two constructors: default (uninitialized) and `(anchor_id, initial_frag)` for post-mount setup.
- **`flush_fragment()` function** (`component/lifecycle.mojo`): Handles all three fragment transitions generically:
  - **empty → populated**: CreateEngine creates fragment children, ReplaceWith the anchor placeholder.
  - **populated → populated**: DiffEngine diffs old fragment vs new fragment (keyed).
  - **populated → empty**: Creates new anchor placeholder, InsertBefore first old item, removes all old items via DiffEngine.
  - Does NOT finalize — caller controls when to write the End sentinel (enables batching).
  - Returns updated `FragmentSlot` with new state.
- **Todo app** (`apps/todo.mojo`): Replaced 3 fields (`current_frag`, `ul_placeholder_id`, `items_mounted`) with single `item_slot: FragmentSlot`. Removed ~90 lines of manual transition logic from `todo_app_flush()`, replaced with single `flush_fragment()` call. Removed unused `DiffEngine` import. Net reduction: 484 → 376 lines (−108 lines).
- **Bench app** (`apps/bench.mojo`): Replaced 3 fields (`current_frag`, `anchor_id`, `rows_mounted`) with single `row_slot: FragmentSlot`. Removed ~80 lines of manual transition logic from `bench_app_flush()`, replaced with single `flush_fragment()` call. Removed unused `CreateEngine`/`DiffEngine` imports. Net reduction: 504 → 420 lines (−84 lines).
- **Lifecycle module growth**: `component/lifecycle.mojo` grew from 184 → 364 lines (+180 lines) to house the shared `FragmentSlot` struct and `flush_fragment()` helper with full documentation.
- **Total app reduction**: 1,205 → 1,013 lines across all three apps (−192 lines). Net across all changed files: −10 lines (223 insertions, 233 deletions).
- **All 674 Mojo + 859 JS tests pass unchanged**, confirming that the extracted lifecycle helper produces identical mutation sequences and DOM output.

### 10.9 AppShell Flush Methods & Scheduler Integration (✅ Done)

Added convenience methods to `AppShell` so app flush functions no longer bypass the Scheduler or pass raw subsystem pointers:

- **`AppShell.consume_dirty()` method** (`component/app_shell.mojo`): Routes dirty scope processing through the height-ordered Scheduler instead of raw `runtime.drain_dirty()`. Calls `collect_dirty()` (which drains runtime → scheduler with deduplication and height sorting) then consumes all scheduled entries. Returns `True` if any scopes were dirty. For single-scope apps this is functionally equivalent to a raw drain, but correctly prepares for multi-scope support by ensuring parent-before-child render order.
- **`AppShell.flush_fragment()` method** (`component/app_shell.mojo`): Convenience wrapper around the lifecycle `flush_fragment()` helper that uses the shell's own `eid_alloc`, `runtime`, and `store` pointers. Eliminates the need for apps to pass 4 raw pointers individually. Same semantics: handles all three fragment transitions (empty→populated, populated→populated, populated→empty), does NOT finalize.
- **Counter app** (`apps/counter.mojo`): Replaced `shell.has_dirty()` + `runtime[0].drain_dirty()` (5 lines) with single `shell.consume_dirty()` call (2 lines). Flush path no longer accesses `runtime[0]` directly. Net reduction: 217 → 214 lines (−3 lines).
- **Todo app** (`apps/todo.mojo`): Replaced `shell.has_dirty()` + `runtime[0].drain_dirty()` with `shell.consume_dirty()`. Replaced 6-argument `flush_fragment(writer, eid, rt, store, slot, frag)` call with 3-argument `shell.flush_fragment(writer, slot, frag)`. Removed direct `flush_fragment` import from component package. Net reduction: 376 → 370 lines (−6 lines).
- **Bench app** (`apps/bench.mojo`): Same changes as todo. Replaced raw drain + raw flush_fragment with shell methods. Removed direct `flush_fragment` import. Net reduction: 420 → 414 lines (−6 lines).
- **AppShell growth**: `component/app_shell.mojo` grew from 242 → 304 lines (+62 lines) for the two new methods with full documentation.
- **Total app reduction**: 1,013 → 998 lines across all three apps (−15 lines). Net across all changed files: +47 lines (76 insertions, 29 deletions).
- **Key architectural improvement**: All app flush paths now route through the Scheduler (M10.4) instead of bypassing it with raw `drain_dirty()`. The Scheduler's height-ordered processing and deduplication are now active, preparing the codebase for multi-scope component trees.
- **All 674 Mojo + 859 JS tests pass unchanged**, confirming that the scheduler-routed flush produces identical mutation sequences and DOM output.

### 10.10 Precompiled Test Binary Infrastructure (✅ Done)

Implemented precompiled test binaries to reduce Mojo test suite execution from ~5–6 minutes to ~10 seconds for iterative development. No code generation — each test module has an inline `fn main()` that shares a single `WasmInstance` across all tests.

- **Inline `fn main()` per test module**: Every `test/test_*.mojo` file now ends with a `fn main() raises` block that creates one `WasmInstance` via `get_instance()` and calls all test functions in sequence. Adding a new test requires one additional line in `main()` — no external scripts or generated files. `mojo build test/test_scheduler.mojo` produces a standalone binary directly.
- **Shared `WasmInstance` parameter**: All test functions accept `w: UnsafePointer[WasmInstance]` instead of each creating their own instance. This eliminated per-function `_load()` calls (−591 lines net) and is what enables the single-process optimization.
- **`scripts/build_test_binaries.sh`**: Compiles every `test/test_*.mojo` that contains `fn main` into standalone binaries in `build/test-bin/` using `mojo build`. Runs compilations in parallel (up to `nproc` jobs, configurable via `-j`). Supports incremental builds: skips compilation when the binary is newer than its source file and `test/wasm_harness.mojo`. Supports `--force` flag for full rebuilds.
- **`scripts/run_test_binaries.sh`**: Launches all precompiled binaries in `build/test-bin/` concurrently from the project directory (so `build/out.wasm` / `build/out.cwasm` paths resolve correctly). Waits for each in order, captures stdout/stderr per binary, reports pass/fail with summary lines. Supports `-v` verbose mode. Displays elapsed wall-clock time.
- **Justfile targets**:
  - `test-build` — Depends on `precompile-if-changed` (ensures `.cwasm` exists for fast WASM loading), then runs `build_test_binaries.sh`.
  - `test-run` — Runs `run_test_binaries.sh` (precompiled binaries only, no compilation).
  - `test` — `test-build` + `test-run` (full incremental build-and-run cycle).
  - `test-all` — `test` + `test-js` (all Mojo + JS tests).
- **Removed codegen artifacts**: Deleted `scripts/gen_test_fast.sh`, `scripts/gen_test_runner.sh`, `test/fast/` directory, and `test/test_all.mojo`. The entire test infrastructure is now plain Mojo with no generated code.
- **Performance results** (16-core machine, 674 Mojo tests across 26 modules):

| Scenario | Time | Notes |
|---|---|---|
| Cold build (all 26 binaries) | ~92s | Parallel `mojo build` on 16 cores |
| Incremental build (nothing changed) | <0.1s | All 26 skipped via timestamp check |
| Incremental build (1 test file changed) | ~11s | Only changed module recompiled |
| Run precompiled binaries | ~10s | 26 binaries in parallel, wasmtime init |
| Full cycle (no code change) | ~11s | Skip build + run binaries |
| Full cycle + JS tests | ~22s | Mojo binaries + 859 JS tests |
| Previous `mojo test` approach | ~5–6 min | Each of 674 tests compiled separately |

- **Key design decisions**:
  - No code generation — `fn main()` lives directly in each test file, keeping test discovery and test execution in one place.
  - Each binary loads `build/out.cwasm` (pre-compiled WASM) for fast wasmtime instantiation (~70ms per binary).
  - Timestamp-based incremental checks compare against 2 dependencies: the test source file and `test/wasm_harness.mojo`.
  - `get_instance()` returns a heap-allocated `WasmInstance` pointer, shared across all test calls in a single process.
- **All 674 Mojo + 859 JS tests pass**, confirming precompiled binaries produce identical results.

---

### 10.11 README & Documentation Update (✅ Done)

Updated the README to accurately reflect the current state of the project after Phase 10 completion. The README had fallen behind — referencing 790 tests (now 1,533), missing several packages added during Phase 10, and lacking documentation for the precompiled test binary workflow.

- **Updated test counts**: 790 → 1,533 (674 Mojo + 859 JS) throughout the document.
- **Added new packages to project structure**: `src/component/` (AppShell, lifecycle), `src/scheduler/` (height-ordered dirty queue), `src/vdom/dsl.mojo` (ergonomic DSL), `src/poc/` (extracted PoC modules with all submodules listed).
- **Added `examples/lib/`**: Shared JS runtime (boot, env, interpreter, protocol, strings) that was extracted in M10.3.
- **Added `scripts/` directory**: `build_test_binaries.sh`, `run_test_binaries.sh`, `precompile.mojo`.
- **Added "Test infrastructure" section**: Documents the precompiled binary workflow (M10.10), including timing table (cold build ~92s, incremental <0.1s, run ~10s) and instructions for adding new tests.
- **Added "Ergonomic DSL" section**: Shows the `el_*` tag helpers and `VNodeBuilder` API (M10.5).
- **Updated features list**: Added ergonomic DSL, AppShell abstraction, and test count badge.
- **Updated reactive model**: Added Scheduler step (step 3: dirty scopes collected into height-ordered scheduler) and updated the flow diagram to include `scheduler`.
- **Updated build pipeline**: Added step 4 (wasmtime pre-compilation to `.cwasm`).
- **Updated test results section**: Added DSL, Component, and Scheduler test categories.
- **Updated prerequisites**: Added `wasmtime` to the Nix dev shell tools list.
- **Alphabetized `src/` directory listing**: Reorganized from ad-hoc order to consistent alphabetical order for easier scanning.
- **All 674 Mojo + 859 JS tests pass unchanged** (documentation-only change).

---

### 10.12 Test Filter Support (✅ Done)

Added substring filter arguments to `build_test_binaries.sh`, `run_test_binaries.sh`, and the Justfile so developers can target specific test modules during iterative work instead of building/running all 26 binaries.

- **`scripts/build_test_binaries.sh [FILTER...]`**: Positional arguments are matched as substrings against source file names. `bash scripts/build_test_binaries.sh signals` builds only `test_signals`; `bash scripts/build_test_binaries.sh signals mut` builds `test_signals` + `test_mutations`. When no filter is given, all modules are built (unchanged behavior). On no match, lists available test modules to stderr.
- **`scripts/run_test_binaries.sh [FILTER...]`**: Same substring matching against binary names. `bash scripts/run_test_binaries.sh dsl` runs only `test_dsl` (~88ms instead of ~10s). Flags (`-v`, `--verbose`) still work and can be mixed with filters (`-v dsl`). On no match, lists available binaries to stderr.
- **Justfile targets accept variadic filter**: `test-build`, `test-run`, and `test` now use `*FILTER` parameter syntax. `just test signals` builds + runs only `test_signals`. `just test signals mutations` targets both modules. `just test` (no args) runs everything as before.
- **Performance improvement for single-module iteration**: Running one module takes ~100ms vs ~10s for all 26. This makes the edit → test → fix cycle significantly faster when working on a single subsystem.
- **Help text updated**: Both scripts show filter usage examples in `--help` output.
- **All 674 Mojo + 859 JS tests pass unchanged** (infrastructure-only change).

---

### 10.13 Extract DSL Test Logic from main.mojo (✅ Done)

Moved the 19 self-contained `dsl_test_*` function bodies out of `main.mojo` into a dedicated `src/vdom/dsl_tests.mojo` module. The `@export` wrappers in `main.mojo` now delegate to the extracted functions, following the same thin-wrapper pattern used by the PoC and app sections.

- **`src/vdom/dsl_tests.mojo`** (761 lines): New module containing all 19 DSL test function implementations (`test_text_node`, `test_dyn_text_node`, `test_dyn_node_slot`, `test_static_attr`, `test_dyn_attr`, `test_empty_element`, `test_element_with_children`, `test_element_with_attrs`, `test_element_mixed`, `test_nested_elements`, `test_counter_template`, `test_to_template_simple`, `test_to_template_attrs`, `test_to_template_multi_root`, `test_vnode_builder`, `test_vnode_builder_keyed`, `test_all_tag_helpers`, `test_count_utilities`, `test_template_equivalence`). Each returns `Int32` (1 = pass, 0 = fail). Imports DSL constructors, tag constants, template types, and signal runtime directly from sibling modules.
- **`main.mojo` reduction**: 4,282 → 3,736 lines (−546 lines, −12.7%). The DSL self-contained test section shrank from ~667 lines of inline test logic to ~103 lines of thin `@export` wrappers. Each wrapper is a single `return _dsl_test_*()` call.
- **Import pattern**: `main.mojo` imports from `vdom.dsl_tests` using aliased names (`test_text_node as _dsl_test_text_node`, etc.) to avoid collisions with the `@export` wrapper names. The 19 imports add 21 lines to the import section.
- **No behavioral change**: The WASM export names (`dsl_test_text_node`, etc.) are unchanged, so all JS and Mojo test harnesses work without modification.
- **All 674 Mojo + 859 JS tests pass unchanged**, confirming that the extracted test module produces identical results.

### 10.14 Consolidate WASM ABI Helpers & Close Test Gaps (✅ Done)

Replaced 12 identical type-specific `_int_to_*_ptr` functions and 4 `_*_ptr_to_i64` functions with two generic helpers (`_as_ptr[T]`, `_to_i64[T]`), eliminating ~100 lines of repetitive boilerplate. Audited all 392 WASM exports for test coverage and closed the 5 identified gaps.

- **Generic pointer helpers**: `_as_ptr[T](addr: Int) -> UnsafePointer[T]` replaces `_int_to_runtime_ptr`, `_int_to_builder_ptr`, `_int_to_vnode_store_ptr`, `_int_to_eid_alloc_ptr`, `_int_to_writer_ptr`, `_int_to_counter_ptr`, `_int_to_todo_ptr`, `_int_to_bench_ptr`, `_int_to_scheduler_ptr`, `_int_to_shell_ptr`, `_int_to_node_ptr`, `_int_to_vb_ptr`, and the untyped `_int_to_ptr`. `_to_i64[T](ptr: UnsafePointer[T]) -> Int64` replaces `_ptr_to_i64`, `_runtime_ptr_to_i64`, `_builder_ptr_to_i64`, `_vnode_store_ptr_to_i64`. All 80+ call sites updated.
- **`main.mojo` reduction**: 3,736 → 3,601 lines (−135 lines, −3.6%).
- **Test coverage audit**: Identified 5 untested WASM exports (`bench_scope_id`, `debug_eid_alloc_capacity`, `dsl_node_count_all`, `dsl_node_count_dyn_node`, `dsl_node_count_static_attr`). All 5 now have tests.
- **New tests**: `test_debug_eid_alloc_capacity` in `test_element_id.mojo` (capacity query after allocations), `test_dsl_node_count_dyn_node_and_static_attr` in `test_dsl.mojo` (covers `dsl_node_count_all`, `dsl_node_count_dyn_node`, `dsl_node_count_static_attr`), `testBenchScopeId` in `bench.test.ts` (root scope ID validity).
- **All 676 Mojo + 860 JS tests pass** (674→676 Mojo, 859→860 JS; +3 total new tests).

### 10.15 Clean Unused Imports & Consolidate Writer Boilerplate (✅ Done)

Audited all ~300 lines of imports in `main.mojo` and removed 140 symbols that became unused after earlier extractions (M10.13 DSL test extraction, M10.6 DSL-based app rewrite, etc.). Consolidated the repeated `MutationWriter` heap-allocation/cleanup pattern into two shared helpers.

- **Unused import cleanup**: Removed 140 imported symbols across 8 import groups. Largest removals: 35 `TAG_*` constants, 38 `el_*` tag helpers, 18 `TNODE_`/`TATTR_`/`VNODE_`/`AVAL_`/`DNODE_` constants, 18 `EVT_`/`ACTION_` constants, 6 `NODE_*` constants, and 7 unused types/functions (`HandlerRegistry`, `SchedulerEntry`, `TodoItem`, `BenchRow`, `HOOK_SIGNAL`, `mount_vnode`, `mount_vnode_to`, `diff_and_finalize`, `diff_no_finalize`, `create_no_finalize`, `to_template_multi`, `el`). Import block reduced from ~300 → ~150 lines.
- **Writer boilerplate consolidation**: Added `_alloc_writer(buf_ptr, capacity)` and `_free_writer(ptr)` helpers. Replaced 8 identical 7-line `UnsafePointer[MutationWriter].alloc` / `init_pointee_move` / `destroy_pointee` / `free` blocks in `counter_rebuild`, `counter_flush`, `todo_rebuild`, `todo_flush`, `bench_rebuild`, `bench_flush`, `shell_mount`, and `shell_diff` with 2-line calls.
- **`main.mojo` reduction**: 3,601 → 3,425 lines (−176 lines, −4.9%).
- **No behavioral change**: All 397 WASM exports unchanged. All 676 Mojo + 860 JS tests pass unchanged.

### 10.16 Bool→Int32 Helper & Node Handle Consolidation (✅ Done)

Replaced 32 repetitive `if X: return 1; return 0` boolean-to-Int32 patterns with a single `_b2i(val: Bool) -> Int32` helper, and consolidated 6 Node heap-allocation + 3 Node heap-free patterns with `_alloc_node`/`_free_node` helpers.

- **`_b2i(val: Bool) -> Int32` helper**: Converts a Bool to Int32 (1 or 0) for WASM export return values. Replaced 32 identical 3-line `if X: return 1; return 0` blocks across all boolean-returning exports (`eid_is_alive`, `signal_contains`, `runtime_has_context`, `runtime_has_dirty`, `scope_contains`, `scope_is_dirty`, `scope_has_scope`, `scope_is_first_render`, `tmpl_contains_name`, `vnode_has_key`, `vnode_is_mounted`, `handler_contains`, `dispatch_event`, `dispatch_event_with_i32`, `ctx_remove`, `err_is_boundary`, `err_has_error`, `suspense_is_boundary`, `suspense_is_pending`, `counter_handle_event`, `counter_has_dirty`, `todo_item_completed_at`, `todo_has_dirty`, `bench_has_dirty`, `scheduler_is_empty`, `scheduler_has_scope`, `shell_is_alive`, `shell_has_dirty`, `shell_scheduler_empty`, `shell_dispatch_event`). Each call site reduced from 3 lines to 1 line.
- **`_alloc_node(var val: Node) -> Int64` helper**: Heap-allocates a Node and returns its address as Int64. Replaced 6 identical 3-line alloc/init/return blocks in `dsl_node_text`, `dsl_node_dyn_text`, `dsl_node_dyn_node`, `dsl_node_attr`, `dsl_node_dyn_attr`, and `dsl_node_element` with 1-line calls.
- **`_free_node(addr: Int64)` helper**: Destroys and frees a heap-allocated Node. Replaced 3 identical destroy/free patterns in `dsl_node_add_item`, `dsl_node_destroy`, and `dsl_to_template` with 1-line calls.
- **`main.mojo` reduction**: 3,425 → 3,378 lines (−47 lines, −1.4%).
- **No behavioral change**: All 397 WASM exports unchanged. All 676 Mojo + 860 JS tests pass unchanged.

### 10.17 Typed Pointer Accessors & Missed `_b2i` Fixes (✅ Done)

Added 5 typed `_get_*` pointer accessor helpers for the remaining subsystem types, and fixed 3 boolean-to-Int32 conversions missed in M10.16. Inlined all single-use pointer accesses to eliminate redundant `var` lines across 73 export functions.

- **5 new typed accessors**: `_get_counter(app_ptr) -> UnsafePointer[CounterApp]`, `_get_todo(app_ptr) -> UnsafePointer[TodoApp]`, `_get_bench(app_ptr) -> UnsafePointer[BenchmarkApp]`, `_get_shell(shell_ptr) -> UnsafePointer[AppShell]`, `_get_scheduler(sched_ptr) -> UnsafePointer[Scheduler]`. Consistent with existing `_get_runtime`, `_get_eid_alloc`, `_get_builder`, `_get_vnode_store`, `_get_node`, `_get_vb` helpers.
- **3 missed `_b2i` fixes**: `ctx_consume_found`, `ctx_has_local`, and `suspense_has_pending` still used manual `if X: return 1; return 0` patterns instead of `_b2i`. Now consistent with all other boolean-returning exports.
- **Inlined single-use pointer accesses**: 61 export functions that only used the pointer once had their `var app = _as_ptr[T](Int(ptr))` line eliminated, calling the getter inline instead (e.g., `return Int32(_get_counter(app_ptr)[0].template_id)`). Multi-access exports (rebuild/flush, destroy, diff) use `var` with the shorter getter call.
- **`main.mojo` reduction**: 3,378 → 3,335 lines (−43 lines, −1.3%).
- **No behavioral change**: All 397 WASM exports unchanged. All 676 Mojo + 860 JS tests pass unchanged.

---

## Milestone Checklist

- [x] **M0:** Arena allocator + collections + ElementId + protocol defined. All existing tests still pass.
- [x] **M1:** `Signal[Int32]` works end-to-end: create, read, write, subscribe, notify. Tested via WASM exports.
- [x] **M2:** Scopes created, components render VNodes, hooks work (use_signal returns stable signal across re-renders).
- [x] **M3:** Templates registered, Tier 1 VNode builder produces correct structures, tag helpers work.
- [x] **M4:** Diff algorithm produces correct mutations. Full mutation round-trip: Mojo diff → binary buffer → JS decode → verified.
- [x] **M5:** JS interpreter applies mutations to real DOM. Hand-crafted mutation buffers produce correct DOM trees.
- [x] **M6:** Events flow: click in DOM → JS → WASM → signal write → re-render → mutations → DOM update.
- [x] **M7:** Counter app works in a browser. Click increment, see number change. 🎉
- [x] **M8:** Todo list works. Conditional rendering, keyed lists, context, error boundaries, suspense.
- [x] **M9:** js-framework-benchmark competitive. Memory bounded. Tier 2 compile-time templates deferred (core template-aware diffing already in place; full `comptime` path awaits Mojo language maturation). Developer tools functional.
- [x] **M10.1:** App modules extracted (`apps/counter.mojo`, `apps/todo.mojo`, `apps/bench.mojo`). `main.mojo` reduced from 4,249 → 2,930 lines. All 790 tests pass.
- [x] **M10.2:** PoC exports extracted to `poc/` package (`poc/arithmetic.mojo`, `poc/bitwise.mojo`, `poc/comparison.mojo`, `poc/algorithms.mojo`, `poc/strings.mojo`). `main.mojo` is now pure `@export` wrappers. All 602 Mojo + 790 JS tests pass.
- [x] **M10.3:** Shared JS runtime extracted to `examples/lib/` (env, protocol, interpreter, strings, boot). Examples deduplicated: counter 81 lines, todo 194 lines, bench 160 lines. All 602 Mojo + 790 JS tests pass.
- [x] **M10.4:** Component abstraction. `AppShell` struct (`component/app_shell.mojo`), lifecycle helpers (`component/lifecycle.mojo`), height-ordered scheduler (`scheduler/scheduler.mojo`). `shell_*` and `scheduler_*` WASM exports. 37 new tests. All 641 Mojo + 790 JS tests pass.
- [x] **M10.5:** Ergonomic builder API. `Node` tagged union (`vdom/dsl.mojo`), 40 tag helpers (`el_div`, `el_h1`, …), `to_template()` conversion, `VNodeBuilder` for ergonomic VNode construction, count utilities. Template equivalence verified (DSL matches manual builder). 33 new Mojo + 69 new JS tests. All 674 Mojo + 859 JS tests pass.
- [x] **M10.6:** DSL-based app rewrite. Counter, todo, and bench apps converted from `TemplateBuilder`/manual VNode construction to `el_*`/`to_template`/`VNodeBuilder` DSL. All 674 Mojo + 859 JS tests pass unchanged.
- [x] **M10.7:** AppShell integration. All three apps refactored from manual `Runtime`/`VNodeStore`/`ElementIdAllocator` pointer management to `AppShell` struct. Counter fully uses shell lifecycle methods (`mount`, `diff`, `finalize`, `dispatch_event`). Todo and bench use shell for init/destroy/signals, direct subsystem access for complex flush logic. 8 WASM exports updated. All 674 Mojo + 859 JS tests pass unchanged.
- [x] **M10.8:** Fragment lifecycle helpers. `FragmentSlot` struct + `flush_fragment()` extracted to `component/lifecycle.mojo`. Todo and bench apps refactored from ~90/~80 lines of manual fragment transition logic to single `flush_fragment()` call each. Apps reduced by −192 lines total (todo: 484→376, bench: 504→420). All 674 Mojo + 859 JS tests pass unchanged.
- [x] **M10.9:** AppShell flush methods & scheduler integration. `consume_dirty()` routes dirty scope processing through the Scheduler instead of raw `drain_dirty()`. `flush_fragment()` method on AppShell wraps lifecycle helper using shell's own pointers (3 args instead of 6). All app flush paths simplified, no more raw subsystem pointer access in flush. Apps reduced by −15 lines (counter: 217→214, todo: 376→370, bench: 420→414). All 674 Mojo + 859 JS tests pass unchanged.
- [x] **M10.10:** Precompiled test binary infrastructure. Each `test/test_*.mojo` has an inline `fn main()` sharing one `WasmInstance` across all tests — no code generation. `build_test_binaries.sh` compiles them in parallel with incremental timestamp checks, `run_test_binaries.sh` executes all binaries concurrently. Test suite reduced from ~5–6 min to ~11s (`just test`, no code change) or ~10s (`just test-run`, binaries pre-built). All 674 Mojo + 859 JS tests pass.
- [x] **M10.11:** README & documentation update. Updated test counts (790 → 1,533), added missing packages (component, scheduler, DSL, poc, examples/lib, scripts), added "Test infrastructure" and "Ergonomic DSL" sections, updated reactive model with Scheduler step, alphabetized project structure. All 674 Mojo + 859 JS tests pass unchanged.
- [x] **M10.12:** Test filter support. Substring filter arguments added to `build_test_binaries.sh`, `run_test_binaries.sh`, and Justfile (`just test signals`, `just test-run dsl`, `just test signals mut`). Single-module runs take ~100ms vs ~10s for all 26 binaries. All 674 Mojo + 859 JS tests pass unchanged.
- [x] **M10.13:** Extract DSL test logic from main.mojo. 19 self-contained `dsl_test_*` function bodies moved to `src/vdom/dsl_tests.mojo` (761 lines). `main.mojo` reduced from 4,282 → 3,736 lines (−546 lines, −12.7%). Inline test logic replaced with thin `@export` wrappers. All 674 Mojo + 859 JS tests pass unchanged.
- [x] **M10.14:** Consolidate WASM ABI helpers & close test gaps. 16 type-specific pointer conversion functions replaced with 2 generic helpers (`_as_ptr[T]`, `_to_i64[T]`). `main.mojo` reduced from 3,736 → 3,601 lines (−135 lines, −3.6%). 5 untested WASM exports identified via audit; all now covered. All 676 Mojo + 860 JS tests pass.
- [x] **M10.15:** Clean unused imports & consolidate writer boilerplate. 140 unused import symbols removed (TAG_*, el_*, EVT_*, ACTION_*, TNODE_*, etc.). 8 identical MutationWriter alloc/free blocks replaced with `_alloc_writer`/`_free_writer` helpers. `main.mojo` reduced from 3,601 → 3,425 lines (−176 lines, −4.9%). All 676 Mojo + 860 JS tests pass unchanged.
- [x] **M10.16:** Bool→Int32 helper & Node handle consolidation. `_b2i(Bool) -> Int32` replaces 32 identical `if X: return 1; return 0` patterns across all boolean-returning exports. `_alloc_node`/`_free_node` consolidate 6 Node alloc + 3 Node free patterns in DSL exports. `main.mojo` reduced from 3,425 → 3,378 lines (−47 lines, −1.4%). All 676 Mojo + 860 JS tests pass unchanged.
- [x] **M10.17:** Typed pointer accessors & missed `_b2i` fixes. 5 new `_get_*` helpers (`_get_counter`, `_get_todo`, `_get_bench`, `_get_shell`, `_get_scheduler`) replace 73 `_as_ptr[T](Int(x))` call sites. 3 missed `_b2i` conversions fixed (`ctx_consume_found`, `ctx_has_local`, `suspense_has_pending`). 61 single-use pointer accesses inlined. `main.mojo` reduced from 3,378 → 3,335 lines (−43 lines, −1.3%). All 676 Mojo + 860 JS tests pass unchanged.