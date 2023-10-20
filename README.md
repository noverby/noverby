## Homepage
[![Niclas Overby's Homepage](homepage.png)](https://niclas.overby.me/)

## Projects
* [My Nix Files](https://github.com/noverby/nixfiles)
* [RadikalWiki](https://github.com/RadikalWiki/radikalwiki)

## Github Stats
[![GitHub Streak](https://streak-stats.demolab.com/?user=noverby&theme=dark)](https://git.io/streak-stats)

## Stack Migration
| Status | Component | Successor | Compat | Legacy |
|:-:|-|-|-|-|
| 🚫 | Compiler Framework | [Cranelift](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift), [Zig](https://github.com/ziglang/zig) | N/A | [LLVM](https://github.com/llvm/llvm-project) |
| ✅ | System Language | [Zig](https://github.com/ziglang/zig), [Rust](https://github.com/rust-lang/rust) | [cxx](https://github.com/dtolnay/cxx), [bindgen](https://github.com/rust-lang/rust-bindgen) | [Clang](https://github.com/llvm/llvm-project) |
| ✅ | Shell | [Nushell](https://github.com/nushell/nushell)| ?| [Bash](https://git.savannah.gnu.org/cgit/bash.git) |
| ✅ | Shell Utilities | [Nushell Builtins](https://github.com/nushell/nushell) | [uutils](https://github.com/uutils/coreutils) | [GNU Coreutils](https://git.savannah.gnu.org/cgit/coreutils.git) |
| 🚫 | Build Script| [Just](https://github.com/casey/just) | ?| [GNU Make](https://git.savannah.gnu.org/cgit/make.git) |
| 🚫 | Scripting Language | [Roc](https://github.com/roc-lang/roc)| [RustPython](https://github.com/RustPython/RustPython), [WASI](https://github.com/WebAssembly/WASI) | [Python](https://github.com/python/cpython), [TypeScript](https://github.com/microsoft/TypeScript) |
| 🚧 | Config Language | [Nickel](https://github.com/tweag/nickel) | N/A | [Nix](https://github.com/NixOS/nix)|
| 🚧 | Package Manager | [Tvix](https://github.com/tvlfyi/tvix) | [Flatpak](https://github.com/flatpak/flatpak) | [Nix](https://github.com/NixOS/nix) |
| 🚧 | 2D Toolkit | [Iced](https://github.com/iced-rs/iced) | [Cosmic Gtk Theme](https://github.com/pop-os/gtk-theme) | [GTK](https://gitlab.gnome.org/GNOME/gtk), [Qt](https://github.com/qt/qt5) |
| 🚧 | Desktop Environment | [Cosmic Epoch](https://github.com/pop-os/cosmic-epoch) | [Cosmic](https://github.com/pop-os/cosmic) | [Gnome Shell](https://gitlab.gnome.org/GNOME/gnome-shell)|
| ❓ | IDE | ? | [LSP](https://github.com/microsoft/language-server-protocol), [BSP](https://github.com/build-server-protocol/build-server-protocol) | [VS Codium](https://github.com/VSCodium/vscodium) |
| 🚫 | Web Browser | [Servo](https://github.com/servo/servo) | [Chrome Extension API](https://developer.chrome.com/docs/extensions/reference) | [Firefox](https://github.com/mozilla/gecko-dev)|
| ✅ | Distro | [NixOS](https://github.com/NixOS/nixpkgs) | [OCI](https://github.com/opencontainers/runtime-spec), [Distrobox](https://github.com/89luca89/distrobox) | [Fedora Silverblue](https://fedoraproject.org/silverblue/) |
| 🚫 | Container Runtime | [youki](https://github.com/containers/youki) | [OCI](https://github.com/opencontainers/runtime-spec) | [runc](https://github.com/opencontainers/runc) |

## Wish List

### Bun
* [Implement Node-API](https://github.com/oven-sh/bun/issues/158)

### Flatpak
* [Webextensions Portal](https://github.com/flatpak/xdg-desktop-portal/pull/705)

### Zig
* [Divorce from LLVM](https://github.com/ziglang/zig/issues/16270)
* [Comptime Interfaces](https://github.com/ziglang/zig/issues/1268)

### Roc
* [Language Server](https://github.com/ayazhafiz/roc/tree/langsrv)

### Matrix
* [Discord Forum Support](https://github.com/mautrix/discord/issues/101)

### Nix
* [fromYAML builtin](https://github.com/NixOS/nix/pull/7340)
* [Allow derivations to hardlink](https://github.com/NixOS/nix/issues/1272)

### ECMAScript 
* [Pattern Matching](https://github.com/tc39/proposal-pattern-matching):
  * [Extractors](https://github.com/tc39/proposal-extractors)
* [Pipeline Operator](https://github.com/tc39/proposal-pipeline-operator):
  * [Call This](https://github.com/tc39/proposal-call-this)
* [Type Annotations](https://github.com/tc39/proposal-type-annotations)
* [Record & Tuple](https://github.com/tc39/proposal-record-tuple)
* [ADT Enum](https://github.com/Jack-Works/proposal-enum)
* [Do Expressions](https://github.com/tc39/proposal-do-expressions)
* [Operator Overloading](https://github.com/tc39/proposal-operator-overloading)
* [Array Grouping](https://github.com/tc39/proposal-array-grouping)

### JS/TS Toolchain
* [Hegel: Static JS Type Checker](https://github.com/JSMonk/hegel)
* [Stc: Low-level TS Type Checker](https://github.com/dudykr/stc)

### React/JSX
* [JSX Props Pruning](https://github.com/facebook/jsx/issues/23)
* [React Native Promise](https://github.com/acdlite/rfcs/blob/first-class-promises/text/0000-first-class-support-for-promises.md)

### Mastodon
* [View Remote Followers](https://github.com/mastodon/mastodon/issues/20533)
* [View Old Posts](https://github.com/mastodon/mastodon/issues/17213)
* [Make Financial Supporters Visible](https://github.com/mastodon/mastodon/issues/5380)

