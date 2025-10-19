# Personal Monorepo

## Specifiers

### State
 * ✅: Good for now
 * 🚧: WIP
 * 🚫: Blocked
 * ❓: Undecided

 ### Control
 * 🌐: Managed by [Nonprofit Organization](https://en.wikipedia.org/wiki/Nonprofit_organization)
 * 🏛️: Managed by [Public Authority](https://en.wikipedia.org/wiki/Public_administration)
 * ⚖️: Managed by [Benefit Corporation](https://en.wikipedia.org/wiki/Benefit_corporation)
 * 📖: [Open Standard](https://en.wikipedia.org/wiki/Open_standard)
 * 🏡: Self-hosted
 * 🔒: [Proprietary](https://en.wiktionary.org/wiki/proprietary)
 * ⏳: [Business Source License](https://en.wikipedia.org/wiki/Business_Source_License)

### Compatibility
 * 🆗: Not needed
 * ⬅️: Backward compatible

 ### Language
 * 🔥: [Mojo](https://en.wikipedia.org/wiki/Mojo_(programming_language))
 * 🐍: Python
 * ❄️: Nix/[Nickel](https://github.com/tweag/nickel)
 * 🦀: Rust
 * 🐹: Go
 * 💣: C/C++
 * 🐒: [ECMAScript](https://en.wikipedia.org/wiki/ECMAScript)
 * 🐷: Java/Kotlin
 * 🌙: Lua
 * λ: Haskell

### Miscellaneous
* 🇽➡️🇾: Migrating from 🇽 to 🇾
* 👁️: [Sentientist](https://en.wikipedia.org/wiki/Sentientism)

## Repository

### Config
* [Home Manager Modules ❄️](https://tangled.org/@overby.me/overby.me/tree/main/modules/home-manager)
* [NixOS Modules ❄️](https://tangled.org/@overby.me/overby.me/tree/main/modules/nixos)
* [NixOS Devices ❄️](https://tangled.org/@overby.me/overby.me/tree/main/devices)
* [Devenv Shells ❄️](https://tangled.org/@overby.me/overby.me/tree/main/shells)

### Packages
* [Magic 🔥](https://tangled.org/@overby.me/overby.me/tree/main/packages/magic.nix): Mojo package manager
* [Mojo 🔥](https://tangled.org/@overby.me/overby.me/tree/main/packages/mojo.nix): Mojo development toolchain
* [Cavif-rs 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/cavif-rs/default.nix): AVIF image encoder CLI tool
* [Rcgen 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/rcgen.nix): X.509 certificate generation CLI tool
* [Hakoniwa 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/hakoniwa.nix): Process isolation CLI tool
* [Envy 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/envy.nix): Environment loader CLI tool
* [Nix-sweep 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/nix-sweep.nix): Nix garbage collector
* [Forkfs 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/forkfs.nix): Sandbox a process's changes to file system
* [Busd 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/busd.nix): A D-Bus bus implementation in Rust
* [Rustysd 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/rustysd.nix): A Systemd replacement in Rust
* [Lacy 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/lacy.nix): Fast magical cd alternative
* [Vibe 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/vibe.nix): A desktop audio visualizer
* [Sunsetc 🦀](https://tangled.org/@overby.me/overby.me/tree/main/packages/sunsetc.nix): SSH in Rust

### Projects
* [Homepage 🐒➡️🦀](https://tangled.org/@overby.me/overby.me/tree/main/projects/homepage): Personal website and portfolio
* [Wiki 🐒➡️🦀](https://tangled.org/@overby.me/overby.me/tree/main/projects/wiki): Platform for political conference & beyond
* [Mojo-Wasm 🔥](https://tangled.org/@overby.me/overby.me/tree/main/projects/mojo-wasm): WebAssembly interop layer for Mojo

## Stack

### Hardware

<details open>

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Architecture | [X86-64 🔒](https://en.wikipedia.org/wiki/X86-64) | [RISC-V 📖](https://en.wikipedia.org/wiki/RISC-V), [ARM 🔒](https://en.wikipedia.org/wiki/ARM_architecture_family) | |
| 🚫 | CPU | [AMD 🇺🇸](https://en.wikipedia.org/wiki/AMD) | | [Intel 🇺🇸](https://en.wikipedia.org/wiki/Intel) |
| 🚫 | GPU | [AMD 🇺🇸](https://en.wikipedia.org/wiki/AMD) | [Vortex 📖](https://github.com/vortexgpgpu/vortex) | [Intel 🇺🇸](https://en.wikipedia.org/wiki/Intel), [NVIDIA 🇺🇸](https://en.wikipedia.org/wiki/NVIDIA) |
| 🚫 | Laptop | [Thinkpad T14 Ryzen 7 Pro Gen 6](https://www.lenovo.com/dk/da/p/laptops/thinkpad/thinkpadt/lenovo-thinkpad-t14s-gen-6-14-inch-amd-laptop/len101t0109) | [Tuxedo ARM Laptop 🇪🇺](https://www.tuxedocomputers.com/en/TUXEDO-on-ARM-is-coming.tuxedo), [StarLabs Systems 🇬🇧](https://starlabs.systems) | [Framework 13 🇺🇸](https://frame.work/products/laptop-diy-13-gen-intel), [Dell XPS 13 Plus 9320 🇺🇸](https://www.dell.com/support/home/da-dk/product-support/product/xps-13-9320-laptop) |
| ✅ | Router | [Turris Omnia 🇪🇺](https://www.turris.com/en/products/omnia) | | |
| ✅ | Mobile | [Fairphone 4 🇪🇺](https://shop.fairphone.com/fairphone-4) | | [Google Pixel 7 Pro 🇺🇸](https://store.google.com/product/pixel_7_pro) |
| 🚫 | Watch | [Garmin Fenix 7 🔒🇺🇸](https://foundation.mozilla.org/en/privacynotincluded/garmin-fenix) | [Polar 🇬🇧](https://www.polar.com) | [PineTime 🇭🇰](https://www.pine64.org/pinetime) |
| 🚫 | AR Glasses | [XReal Air 2 Pro 🔒🇨🇳](https://us.shop.xreal.com/products/xreal-air-2-pro) | | [XReal Light 🔒🇨🇳](https://www.xreal.com/light) |
| ✅ | Earphones | [Hyphen Aria 🇨🇭](https://rollingsquare.com/products/hyphen%C2%AE-aria) | | [Shokz Openfit 🇬🇧](https://shokz.com/products/openfit) |
| ✅ | E-book Reader | [reMarkable 2 🔒🇳🇴](https://remarkable.com/store/remarkable-2) | [PineNote 🇭🇰](https://pine64.org/devices/pinenote) | [reMarkable 1 🔒🇳🇴](https://remarkable.com/store/remarkable) |

</details>

### Standards

#### Hardware

<details open>

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Firmware | [Thinkpad UEFI 🔒](https://en.wikipedia.org/wiki/UEFI) | [Coreboot 💣](https://coreboot.org), [Oreboot 🦀](https://github.com/oreboot/oreboot) | |
| ✅ | Internet of Things Connectivity | [Matter 📖](https://en.wikipedia.org/wiki/Matter_(standard)), [Zigbee 📖](https://en.wikipedia.org/wiki/Zigbee) | |
| 🚧 | Wireless Media | [Google ChromeCast 🔒](https://en.wikipedia.org/wiki/Chromecast) | [MatterCast 📖](https://en.wikipedia.org/wiki/Matter_(standard)) | [Miracast 📖](https://en.wikipedia.org/wiki/Miracast) |
| ✅ | Peripheral Interface | [USB4 📖](https://www.usb.org/usb4) | | [Thunderbolt 3 🔒](https://en.wikipedia.org/wiki/Thunderbolt_(interface)) |
| ✅ | Display Interface | [DisplayPort 📖](https://en.wikipedia.org/wiki/DisplayPort) | | [HDMI 2.1 🔒](https://en.wikipedia.org/wiki/HDMI) |
| 🚧 | Satellite Navigation | [Galileo 🇪🇺](https://www.euspa.europa.eu/eu-space-programme/galileo), [GPS 🏛️🇺🇸](https://www.gps.gov) | | |
| 🚧 | Satellite Internet | | [Iris² 🏛️🇪🇺](https://defence-industry-space.ec.europa.eu/eu-space-policy/iris2_en) | [Starlink 🔒🇺🇸](https://www.starlink.com) |

</details>

#### Interface

<details open>

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Application Binary Interface | [System V ABI 📖](https://wiki.osdev.org/System_V_ABI) | [CrABI 📖](https://github.com/rust-lang/rust/pull/105586) | |
| ✅ | GPU Compute | [Vulkan Compute 📖](https://www.vulkan.org) | | [OpenCL 📖](https://www.khronos.org/opencl) |
| ✅ | Graphics API | [Vulkan 📖](https://www.vulkan.org) | | [OpenGL 📖](https://www.opengl.org) |
| ✅ | Windowing | [Wayland 📖](https://wayland.freedesktop.org) | | [X11 📖](https://www.x.org) |
| ✅ | Heterogeneous Compute | [SYCL 📖](https://www.khronos.org/sycl) | | |
| 🚫 | Tensor Operations | | | | |
| 🚫 | AI Inference | | | | |

</details>

#### Encoding

<details open>

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| ✅ | Text | [UTF-8 📖](https://en.wikipedia.org/wiki/UTF-8) | | [UTF-16 📖](https://en.wikipedia.org/wiki/UTF-16) |
| ✅ | Object Notation | [JSON 📖](https://www.json.org) | [KDL](https://kdl.dev), [EON](https://github.com/emilk/eon) | |
| ✅ | Binary Object Notation | [CBOR 📖](https://cbor.io) | | |
| ✅ | Image Codec | [AVIF 📖](https://en.wikipedia.org/wiki/AVIF) | | [PNG 📖](https://en.wikipedia.org/wiki/Portable_Network_Graphics), [JPEG 📖](https://en.wikipedia.org/wiki/JPEG) |
| ✅ | Audio Codec | [Opus 📖](https://opus-codec.org) | | [AAC 🔒](https://en.wikipedia.org/wiki/Advanced_Audio_Coding) |
| ✅ | Video Codec | [AV1 📖](https://aomedia.org/av1-features/get-started) | | [H.264 🔒](https://en.wikipedia.org/wiki/Advanced_Video_Coding) |

</details>

#### Protocol

<details open>

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Network Transport | [TCP 📖](https://en.wikipedia.org/wiki/Transmission_Control_Protocol) | [QUIC 📖](https://www.chromium.org/quic/) | |
| 🚧 | Web Protocol | [HTTP/2 📖](https://en.wikipedia.org/wiki/HTTP/2) | [HTTP/3 📖](https://en.wikipedia.org/wiki/HTTP/3) | [HTTP/1.1 📖](https://en.wikipedia.org/wiki/HTTP/1.1) |

</details>

### System

#### Core

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Distro | [NixOS 🌐❄️](https://github.com/NixOS/nixpkgs) | | [OCI 📖](https://github.com/opencontainers/runtime-spec), [Distrobox](https://github.com/89luca89/distrobox) | [Fedora Silverblue](https://fedoraproject.org/silverblue) |
| ✅ | Kernel | [Zen Linux Kernel 🌐💣](https://github.com/zen-kernel/zen-kernel) | [Asterinas 🦀](https://github.com/asterinas/asterinas), [Redox OS 🦀](https://gitlab.redox-os.org/redox-os/redox) | [Rust For Linux 🦀](https://rust-for-linux.com/) | |
| 🚧 | Libc | [Glibc 💣](https://www.gnu.org/software/libc) | [Musl 💣](https://www.musl-libc.org), [Relibc 🦀](https://github.com/redox-os/relibc) | [Gcompat 💣](https://git.adelielinux.org/adelie/gcompat) | |
| 🚫 | Init System | [Systemd 💣](https://github.com/systemd/systemd) | [Redox Init 🦀](https://gitlab.redox-os.org/redox-os/init) [Rustysd 🦀](https://github.com/KillingSpark/rustysd) | ⬅️ | |
| 🚧 | Inter-process Communication | [Dbus 💣](https://gitlab.freedesktop.org/dbus/dbus) | [Busd 🦀](https://github.com/dbus2/busd) | ⬅️ | |
| ✅ | Filesystem | [Btrfs 📖💣](https://btrfs.wiki.kernel.org/index.php/Main_Page) | [Fxfs 🦀](https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/storage/fxfs) [Redoxfs 🦀](https://gitlab.redox-os.org/redox-os/redoxfs) | 🆗 | [Ext4 📖💣](https://ext4.wiki.kernel.org/index.php/Main_Page) |
| ✅ | Sandboxing | [Hakoniwa 🦀](https://github.com/souk4711/hakoniwa) | | | [Bubblewrap 💣](https://github.com/containers/bubblewrap) |

</details>

#### Libraries

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Compression | [Zlib-rs 🦀](https://github.com/memorysafety/zlib-rs) | | ⬅️ | [Zlib 💣](https://github.com/madler/zlib) |
| ✅ | TLS Protocol |  [Rustls 🦀](https://github.com/rustls/rustls) | | 🆗 | [Openssl 💣](https://github.com/openssl/openssl) |
| ✅ | HTTP Protocol | [Hyper 🦀](https://github.com/hyperium/hyper) | | 🆗 | [Nghttp2 💣](https://github.com/nghttp2/nghttp2), [Nghttp3 💣](https://github.com/ngtcp2/nghttp3) |
| ✅ | HTTP Client | [Reqwest 🦀](https://github.com/seanmonstar/reqwest) | | 🆗 | [Curl 💣](https://github.com/curl/curl) |
| ✅ | SSH Protocol | [Russh 🦀](https://github.com/warp-tech/russh) | | 🆗 | [OpenSSH 💣](https://github.com/openssh/openssh-portable) |
| ✅ | Font Rendering | [Cosmic-text 🦀](https://github.com/pop-os/cosmic-text) | | 🆗 | [HarfBuzz 💣](https://github.com/harfbuzz/harfbuzz), [FreeType 💣](https://github.com/freetype/freetype) |
| 🚧 | Browser Engine | [Gecko 🦀💣](https://en.wikipedia.org/wiki/Gecko_(software)) | [Servo 🦀](https://github.com/servo/servo) | ⬅️ | |
| 🚫 | ECMAScript Engine | [V8 💣](https://v8.dev) | [Boa 🦀](https://github.com/boa-dev/boa) | 🆗 | |

</details>

#### Nix

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Package Manager | [Nix 🌐💣](https://github.com/NixOS/nix) | [Snix 🦀](https://git.snix.dev/snix/snix) | ⬅️ | |
| 🚧 | Language | [Nix 🌐💣](https://github.com/NixOS/nix) | [Nickel 🦀](https://github.com/tweag/nickel), [Glistix 🦀](https://github.com/Glistix/glistix) | | |
| ✅ | Formatter | [Alejandra 🦀](https://github.com/kamadorueda/alejandra) | | | [Nixfmt λ](https://github.com/NixOS/nixfmt) |
| ✅ | Static Analyzer | [Statix 🦀](https://github.com/oppiliappan/statix), [Deadnix 🦀](https://github.com/astro/deadnix)  | | | |
| ✅ | Language Server | [Nil 🦀](https://github.com/oxalica/nil) | | | [Nixd 💣](https://github.com/nix-community/nixd) |
| 🚧 | Binary Cache | [Cachix 🔒λ](https://github.com/cachix/cachix) | | 🆗 | [Attic 🦀](https://github.com/zhaofengli/attic) |
| ✅ | Config Manager | [Home Manager 🌐❄️](https://github.com/nix-community/home-manager) | | | |
| ✅ | Secret Manager | [Agenix ❄️](https://github.com/ryantm/agenix) | [Ragenix 🦀❄️](github.com/yaxitech/ragenix) | | |
| ✅ | Deployment | [Colmena 🦀️❄️](https://github.com/zhaofengli/colmena) | | | |
| ✅ | Developer Environment | [Devenv 🦀️❄️](https://github.com/cachix/devenv) | [Organist ❄️](https://github.com/nickel-lang/organist) | 🆗 | |
| ✅ | Flake Framework | [Flakelight ❄️](https://github.com/nix-community/flakelight) | | | [Flake-parts ❄️](https://github.com/hercules-ci/flake-parts) |
| ✅ | File Locator | [Nix-index 🦀](https://github.com/nix-community/nix-index), [Comma 🦀](https://github.com/nix-community/comma) | | | |
| ✅ | Rust Integration | [Crate2nix 🦀❄️](https://github.com/nix-community/crate2nix) | | | [Crane ❄️](https://github.com/ipetkov/crane) |
| ✅ | Package Generation | [Nix-init 🦀](https://github.com/nix-community/nix-init) + [Nurl 🦀](https://github.com/nix-community/nurl) | | | |
| ✅ | Derivation Difference | [Nix-diff λ](https://github.com/Gabriella439/nix-diff) | | | |
| ✅ | Store Explorer | [Nix-du 🦀](https://github.com/symphorien/nix-du), [Nix-tree λ](https://github.com/utdemir/nix-tree) | | | |

</details>

### Tools

#### Command Line

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Shell | [Nushell 🦀](https://github.com/nushell/nushell) | | [Brush 🦀](github.com/reubeno/brush), [Rusty Bash 🦀](https://github.com/shellgei/rusty_bash) | [Bash 💣](https://git.savannah.gnu.org/cgit/bash.git) |
| ✅ | Core Utilities | [Nushell Builtins 🦀](https://github.com/nushell/nushell) | | [Uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Change Directory | [Zoxide 🦀](https://github.com/ajeetdsouza/zoxide) | [Lacy 🦀](https://github.com/timothebot/lacy) | ⬅️ | [Bash Cd 💣](https://git.savannah.gnu.org/cgit/bash.git) |
| ✅ | Directory Usage | [Dust 🦀](https://github.com/bootandy/dust) | | [Uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Superuser | [Sudo-rs 🦀](https://github.com/memorysafety/sudo-rs) | | ⬅️ | [Sudo 💣](https://www.sudo.ws/repos/sudo) |
| ✅ | Fortune | [Fortune-kind 🦀](https://github.com/cafkafk/fortune-kind) | | ⬅️ | [Fortune-mod 💣](https://github.com/shlomif/fortune-mod) |
| ✅ | List Processes | [Procs 🦀](https://github.com/dalance/procs) | | 🆗 | [Ps 💣](https://gitlab.com/procps-ng/procps) |
| ✅ | List Files | [Nushell Builtins 🦀](https://github.com/nushell/nushell) | [Eza 🦀](https://github.com/eza-community/eza) | 🆗 | [Ls 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Find Files | [Fd 🦀](https://github.com/sharkdp/fd) | | [Uutils Findutils 🦀](https://github.com/uutils/findutils) | [Findutils 💣](https://git.savannah.gnu.org/cgit/findutils.git) |
| ✅ | Find Patterns | [Ripgrep 🦀](https://github.com/BurntSushi/ripgrep) | | 🆗 | [Grep 💣](https://git.savannah.gnu.org/cgit/grep.git) |
| ✅ | Find & Replace | [Ast-grep 🦀](https://github.com/ast-grep/ast-grep) | | 🆗 | [Sed 💣](https://www.gnu.org/software/sed) |
| ✅ | File Differences | [Batdiff 🦀](https://github.com/eth-p/bat-extras) + [Delta 🦀](https://github.com/dandavison/delta) | [Difftastic 🦀](https://github.com/wilfred/difftastic) | [Uutils Diffutils 🦀](https://github.com/uutils/diffutils) | [Diffutils 💣](https://git.savannah.gnu.org/cgit/diffutils.git) |
| ✅ | Hex Viewer | [Hyxel 🦀](https://github.com/hyxel/hyxel) | | | [Util Linux Hexdump 💣](https://github.com/util-linux/util-linux) |
| ✅ | Terminal Workspace | [Zellij 🦀](https://github.com/zellij-org/zellij) | | 🆗 | [Tmux 💣](https://github.com/tmux/tmux) |
| ✅ | Parallel Processing | [Rust Parallel 🦀](https://github.com/aaronriekenberg/rust-parallel) | | 🆗 | [GNU Parallel 💣](https://git.savannah.gnu.org/cgit/parallel.git) |
| ✅ | Process Monitor | [Bottom 🦀](https://github.com/ClementTsang/bottom) | | 🆗 | [Top 💣](https://gitlab.com/procps-ng/procps) |
| ✅ | Fuzzy Finder | [Television 🦀](https://github.com/alexpasmantier/television) | | 🆗 | [Fzf 🐹](https://github.com/junegunn/fzf) |
| ✅ | Ping | [Gping 🦀](https://github.com/orf/gping) | | | [Ping 💣](https://git.savannah.gnu.org/cgit/inetutils.git) |
| ✅ | Benchmark | [Hyperfine 🦀](https://github.com/sharkdp/hyperfine) | | | [time 💣](https://www.gnu.org/software/time) |
| ✅ | Port Scanner | [RustScan 🦀](https://github.com/rustscan/rustscan) | | 🆗 | [Nmap 💣](https://github.com/nmap/nmap) |
| ✅ | Tree Viewer | [Tre 🦀](https://github.com/dduan/tre) | | 🆗 | [Tree 💣](https://oldmanprogrammer.net/source.php?dir=projects/tree) |
| 🚧 | PGP | [GnuPG 💣](https://gnupg.org) | [Sequoia-PGP 🦀](https://gitlab.com/sequoia-pgp/sequoia) | 🆗 | |
| 🚧 | SSH | [OpenSSH 💣](https://github.com/openssh/openssh-portable) | [Sunset 🦀](https://github.com/mkj/sunset) | 🆗 | |

</details>

#### Development

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Compiler Framework | [Mlir 💣](https://github.com/llvm/llvm-project/tree/main/mlir), [LLVM 💣](https://github.com/llvm/llvm-project) | [Cranelift 🦀](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift) | ⬅️ | |
| 🚧 | Linker | [Mold 💣](https://github.com/rui314/mold) | [Wild 🦀](https://github.com/davidlattimore/wild) | ⬅️ | [GNU ld 💣](https://sourceware.org/binutils) |
| ✅ | System Language | [Mojo 🔒🔥](https://github.com/modularml/mojo), [Rust 🦀](https://github.com/rust-lang/rust) | | [cxx 🦀](https://github.com/dtolnay/cxx), [bindgen 🦀](https://github.com/rust-lang/rust-bindgen) | |
| ✅ | Scripting Language | [Mojo 🔒🔥](https://github.com/modularml/mojo) | | [RustPython 🦀](https://github.com/RustPython/RustPython), [WASI 📖](https://github.com/WebAssembly/WASI), [Interface Types 📖](https://github.com/WebAssembly/interface-types/tree/main/proposals/interface-types) | [TypeScript 🐒🐹](https://github.com/microsoft/TypeScript) |
| 🚧 | Version Control | [Jujutsu 🦀](https://github.com/jj-vcs/jj), [Git 💣](https://github.com/git/git) | [Gitoxide 🦀](https://github.com/Byron/gitoxide) | [Gix 🦀](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md#gix) | |
| ✅ | Merger | [Mergiraf 🦀](https://codeberg.org/mergiraf/mergiraf) | | ⬅️ | |
| ✅ | Build Script | [Just 🦀](https://github.com/casey/just) | | Rusty Make ([Rusty Bash 🦀](https://github.com/shellgei/rusty_bash)) | [GNU Make 💣](https://git.savannah.gnu.org/cgit/make.git) |
| ✅ | Editor | [Evil Helix 🦀](https://github.com/usagi-flow/evil-helix) | | 🆗 | [Helix 🦀](https://github.com/helix-editor/helix), [Neovim 💣](https://github.com/neovim/neovim) |
| ✅ | IDE | [Zed 🦀](https://github.com/zed-industries/zed) | | [LSP](https://github.com/microsoft/language-server-protocol), [DAP](https://github.com/Microsoft/debug-adapter-protocol), [BSP](https://github.com/build-server-protocol/build-server-protocol) | [VS Codium 🐒💣](https://github.com/VSCodium/vscodium) |
| 🚧 | Pre-commit Manager | [Pre-commit 🐍](https://github.com/pre-commit/pre-commit) | [Prek 🦀](https://github.com/j178/prek) | | |
| ✅ | System Call Tracing | [Lurk 🦀](https://github.com/JakWai01/lurk), [Tracexec 🦀](https://github.com/kxxt/tracexec) | | 🆗 | [Strace 💣](https://github.com/strace/strace) |
| ✅ | Network Client | [Xh 🦀](https://github.com/ducaale/xh) | | 🆗 | [Curl 💣](https://github.com/curl/curl) |
| ✅ | Environment Loader | [Direnv 🐹](https://github.com/direnv/direnv) | [Envy 🦀](https://github.com/mre/envy) | ⬅️ | |
| ✅ | Pager | [Tailspin 🦀](https://github.com/bensadeh/tailspin) | | 🆗 | [Less 💣](https://github.com/gwsw/less) |
| ✅ | Performance Profiler | [Samply 🦀](https://github.com/mstange/samply) | | 🆗 | [Perf 💣](https://perf.wiki.kernel.org/) |
| 🚧 | Bundler | [Rsbuild 🦀](https://github.com/web-infra-dev/rsbuild), [Webpack 🐒](https://github.com/webpack/webpack), [Turbopack 🦀](https://github.com/vercel/turbo) | [Farm 🦀](https://github.com/farm-fe/farm) | 🆗 | |
| ✅ | Certificate Generation | [Rcgen 🦀](https://github.com/rustls/rcgen) | | 🆗 | [Mkcert 🐹](https://github.com/FiloSottile/mkcert) |
| ✅ | TCP Tunnel | [Bore 🦀](https://github.com/ekzhang/bore) | | 🆗 | |
| 🚧 | Monorepo | | [Josh 🦀](https://github.com/josh-project/josh), [Mega 🦀🐒](https://github.com/web3infra-foundation/mega), [Google Piper 🔒](https://en.wikipedia.org/wiki/Piper_(source_control_system)) | 🆗 | |


</details>

### Infrastructure

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | WebAssembly Runtime | [Wasmtime 🦀](https://github.com/bytecodealliance/wasmtime) | | [WASI 📖](https://wasi.dev) | |
| ✅ | ECMAScript Runtime | [Deno 🦀](https://github.com/denoland/deno) | | [Deno Node APIs](https://docs.deno.com/runtime/reference/node_apis) | [Node.js 💣](https://github.com/nodejs/node) |
| ✅ | Container Runtime | [Youki 🦀](https://github.com/containers/youki) | | [OCI 📖](https://github.com/opencontainers/runtime-spec) | [Runc 🐹](https://github.com/opencontainers/runc) |
| 🚧 | Virtualization | [QEMU 💣](https://github.com/qemu/qemu) | [Cloud Hypervisor 🦀](https://github.com/cloud-hypervisor/cloud-hypervisor) | | |
| 🚧 | Meta Database | [Hasura λ➡️🦀](https://github.com/hasura/graphql-engine) | [Surrealdb ⏳🦀](https://github.com/surrealdb/surrealdb) | [GraphQL](https://graphql.org) |
| 🚧 | Database | [Postgres 💣](https://github.com/postgres/postgres) | [Tikv 🦀](https://github.com/tikv/tikv) | 🆗 | |
| 🚧 | Storage Engine | | [Sled 🦀](https://github.com/spacejam/sled), [Fjall 🦀](https://github.com/fjall-rs/fjall) | 🆗 | [RocksDB 💣](https://github.com/facebook/rocksdb) |
| 🚫 | Web Server | [Nginx 💣](https://github.com/nginx/nginx) | [Moella 🦀](https://github.com/Icelk/moella) | | |
| ✅ | Virtual Private Network | [Tailscale 🐹](https://github.com/tailscale/tailscale) | [Innernet 🦀](https://github.com/tonarino/innernet) | | |

</details>

### Graphical User Interface

#### Desktop

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Color Scheme | [Catppuccin](https://github.com/catppuccin/catppuccin) | [Frosted Effect](https://github.com/pop-os/cosmic-epoch/issues/604) | 🆗 | [Adwaita](https://gitlab.gnome.org/GNOME/libadwaita) |
| ✅ | Wallpaper | [Nix-wallpaper ❄️](https://github.com/lunik1/nix-wallpaper) | | 🆗 | |
| ✅ | Design System | [Material You 🔒](https://m3.material.io) | | 🆗 | [Material Design 2 🔒](https://m2.material.io) |
| ✅ | 2D Renderer | [Wgpu 🦀](https://github.com/gfx-rs/wgpu) | | 🆗 | [Skia 💣](https://github.com/google/skia), [Cairo 💣](https://www.cairographics.org) |
| 🚧 | 2D Toolkit | [React 🐒](https://react.dev) | [WIP Toolkit 🔥](https://tangled.org/@overby.me/overby.me/tree/main/projects/mojo-wasm), [Dixous 🦀](https://github.com/dioxusLabs/dioxus) | [Web Component 📖](https://www.webcomponents.org/) | |
| 🚧 | 2D Component Library | [MUI 🐒](https://mui.com) | [Dioxus Components 🦀](https://github.com/DioxusLabs/components) | 🆗 | |
| ✅ | 2D Desktop Engine | [Smithay 🦀](https://github.com/Smithay/smithay) | | 🆗 | [Mutter 💣](https://gitlab.gnome.org/GNOME/mutter) |
| ✅ | 2D Desktop Environment | [Cosmic Epoch 🦀](https://github.com/pop-os/cosmic-epoch) | | 🆗 | [Gnome Shell 💣](https://gitlab.gnome.org/GNOME/gnome-shell) |
| 🚫 | 3D Toolkit | [Stereokit 💣](https://github.com/StereoKit/StereoKit) | | 🆗 | |
| 🚫 | 3D Desktop Engine | [Monado 💣](https://gitlab.freedesktop.org/monado/monado) | | [OpenXR 📖](https://www.khronos.org/openxr) | [Arcan 💣](https://github.com/letoram/arcan) |
| ✅ | 3D Desktop Environment | [Stardust XR 🦀](https://github.com/StardustXR/server/tree/dev) | [Breezy Desktop](https://github.com/wheaney/breezy-desktop) | 🆗 | [Safespaces 🌙](https://github.com/letoram/safespaces) |
| ✅ | File Manager | [Cosmic Files 🦀](https://github.com/pop-os/cosmic-files) | | 🆗 | [GNOME Files 💣](https://gitlab.gnome.org/GNOME/nautilus) |
| ✅ | Terminal | [Cosmic Term 🦀](https://github.com/pop-os/cosmic-term) | | 🆗 | [Wezterm 🦀](https://github.com/wez/wezterm), [GNOME Console 💣](https://gitlab.gnome.org/GNOME/console) |
| ✅ | Web Browser | [Zen Browser 💣🦀](https://zen-browser.app) | [Verso 🦀](https://github.com/versotile-org/verso) | [Chrome Extension API 🔒](https://developer.chrome.com/docs/extensions/reference) | [Firefox 💣🦀](https://github.com/mozilla/gecko-dev), [Unbraved Brave 💣🦀](https://github.com/MulesGaming/brave-debullshitinator) |
| ✅ | Application Store Frontend | [Cosmic Store 🦀](https://github.com/pop-os/cosmic-store) | | 🆗 | [GNOME Software 💣](https://gitlab.gnome.org/GNOME/gnome-software) |
| 🚫 | Application Store Backend | [Flatpak 💣](https://github.com/flatpak/flatpak) | | 🆗 | [Snap 🔒](https://github.com/canonical/snapd), [AppImage 💣](https://github.com/AppImage) |
| ✅ | Office Suite | [OnlyOffice 🐒](https://www.onlyoffice.com) | | [OpenDocument Format 📖](https://en.wikipedia.org/wiki/OpenDocument) | [LibreOffice 💣🐷](https://www.libreoffice.org) |
| ✅ | Remote Desktop | [Rustdesk 🦀](https://github.com/rustdesk/rustdesk) | | [VNC](https://en.wikipedia.org/wiki/VNC) | [GNOME Remote Desktop 💣](https://gitlab.gnome.org/GNOME/gnome-remote-desktop) |

</details>

#### Browser Extensions

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Keyboard Navigation | [Surfingkeys 🐒](https://github.com/brookhong/Surfingkeys) | | 🆗 | |
| ✅ | Advertising Blocker | [uBlock Origin 🐒](https://github.com/gorhill/uBlock) | | 🆗 | |
| ✅ | Grammar Checker | [Harper 🦀](https://github.com/Automattic/harper) | | 🆗 | [LanguageTools 🐷](https://github.com/languagetools) |

</details>

#### Media

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Media Player | [Cosmic Player 🦀](https://github.com/pop-os/cosmic-player) | | [FFMPEG 💣](https://github.com/FFmpeg/FFmpeg), [GStreamer 💣](https://gitlab.freedesktop.org/gstreamer) | [Mpv 💣](https://github.com/mpv-player/mpv) |
| 🚧 | Raster Graphics | [GIMP 💣](https://gitlab.gnome.org/GNOME/gimp) | [Graphite 🦀](https://github.com/GraphiteEditor/Graphite) | 🆗 | |
| ✅ | Vector Graphics | [Graphite 🦀](https://github.com/GraphiteEditor/Graphite) | | 🆗 | [Inkscape 💣](https://gitlab.com/inkscape/inkscape) |
| ✅ | Typesetter | [Typst 🦀](https://github.com/typst) | | 🆗 | [LaTeX 💣](https://github.com/latex3/latex3) |
| 🚧 | Image Optimizer | | [Cavif-rs 🦀](https://github.com/kornelski/cavif-rs) | 🆗 | [Oxipng 🦀](https://github.com/shssoichiro/oxipng), [Optipng 💣](https://optipng.sourceforge.net) |
| 🚧 | Image Processing | | [Wondermagick 🦀](https://github.com/Shnatsel/wondermagick) | 🆗 | [ImageMagick 💣](https://github.com/ImageMagick/ImageMagick) |
| ✅ | Screen Recorder | [Kooha 🦀](https://github.com/SeaDve/Kooha) | | 🆗 | [Mutter Built-in Recorder 💣](https://github.com/GNOME/mutter) |

</details>

#### Mobile

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | OS | [/e/OS 🌐🇪🇺](https://e.foundation/e-os) | | [MicroG 🌐🐷](https://microg.org), [Magisk 🦀💣🐷](https://github.com/topjohnwu/Magisk) | [GrapheneOS 🇨🇦](https://grapheneos.org) |
| ✅ | Launcher | [Olauncher 🐷](https://github.com/tanujnotes/Olauncher) | | 🆗 | [Minimalist Phone 🔒](https://www.minimalistphone.com) |
| ✅ | Keyboard | [Thumb-Key 🐷](https://github.com/dessalines/thumb-key) | | 🆗 | [OpenBoard 🐷](https://github.com/openboard-team/openboard) |
| ✅ | Alarm | [Chrono 🐷](https://github.com/vicolo-dev/chrono) | | 🆗 | [Sleep 🔒](https://sleep.urbandroid.org) |
| ✅ | Browser | [Fennec 💣🦀](https://f-droid.org/en/packages/org.mozilla.fennec_fdroid) | | 🆗 | [Mull 💣🦀](https://github.com/mull-project/mull) |
| ✅ | Maps | [CoMaps 💣](https://comaps.app) | | [Openstreetmap 🌐📖](https://www.openstreetmap.org) | [Organic Maps 💣](https://organicmaps.app), [Google Maps 🔒🇺🇸](https://maps.google.com)|
| ✅ | Distraction Blockers | [TimeLimit 🐷](https://codeberg.org/timelimit/timelimit-android), [LeechBlock NG](https://github.com/proginosko/LeechBlockNG), [Adguard DNS 🇪🇺](https://adguard.com) | | 🆗 | |
| ✅ | Authenticator | [Aegis 🐷](https://getaegis.app) | | [HOTP 📖](https://en.wikipedia.org/wiki/HMAC-based_One-time_Password_algorithm), [TOTP 📖](https://en.wikipedia.org/wiki/Time-based_One-time_Password_algorithm) | |
| ✅ | Music Recognition | [Audile 🐷](https://github.com/aleksey-saenko/MusicRecognizer) | | 🆗 | [Soundhound 🔒🇺🇸](https://www.soundhound.com) |
| ✅ | Malware Scanner | [Hypatia 🐷](https://github.com/MaintainTeam/Hypatia) | | 🆗 | |
| ✅ | Developer Environment | [Nix-on-droid ❄️🐍](https://github.com/nix-community/nix-on-droid) | | 🆗 | [Termux 🐷💣](https://github.com/termux/termux-app) |

</details>

### Platforms

#### Services

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Password Manager | [Bitwarden.eu 🇪🇺](https://bitwarden.eu) | | 🆗 | [Bitwarden.com 🇺🇸](https://bitwarden.com) |
| ✅ | Mail | [Tuta Mail 🇪🇺](https://tuta.com) | | [Mail Import](https://tuta.com/blog/tuta-release-update-february) | [Proton Mail 🌐🇨🇭➡️🇪🇺](https://proton.me/mail) |
| ✅ | Calendar | [Tuta Calendar 🇪🇺](https://tuta.com) | | [iCalendar 📖](https://en.wikipedia.org/wiki/ICalendar) | [Proton Calendar 🔒🌐🇨🇭➡️🇪🇺](https://proton.me/calendar) |
| ✅ | Storage | [Syncthing 🐹🏡🇪🇺](https://github.com/syncthing/syncthing) | [Tuta Drive 🇪🇺](https://tuta.com/blog/pqdrive-project) | 🆗 | [Proton Drive 🌐🇨🇭➡️🇪🇺](https://proton.me/drive) |
| ✅ | Virtual Private Network | [Adguard VPN 🇪🇺](https://adguard.com) | | 🆗 | [Proton VPN 🌐🇨🇭➡️🇪🇺](https://proton.me/vpn) |
| ✅ | Domain Name System | [Adguard DNS 🇪🇺](https://adguard.com) | | 🆗 | [NextDNS 🔒🇺🇸](https://nextdns.io) |
| ✅ | Search Engine | [StartPage 🔒🇪🇺](https://startpage.com) | [Stract 🦀🇪🇺](https://github.com/StractOrg/stract) | [Search Shortcuts](https://support.mozilla.org/en-US/kb/assign-shortcuts-search-engines) | [Kagi 🔒🇺🇸](https://kagi.com), [DuckDuckGo 🔒🇺🇸](https://duckduckgo.com) |
| ✅ | Large Language Model | [Codestral 🇪🇺](https://mistral.ai/news/codestral) | [EuroLLM 🇪🇺](https://eurollm.io) | | |
| ✅ | Large Language Model Provider | [Ollama 🐹🏡🇪🇺](https://github.com/ollama/ollama), [Mistral 🇪🇺](https://mistral.ai) | [Lumo 🇪🇺](https://lumo.proton.me) | 🆗 | [Claude 🔒🇺🇸](https://claude.ai), [OpenAI 🔒🇺🇸](https://openai.com) |
| ✅ | Version Control | [Codeberg 🇪🇺](https://codeberg.org/noverby), [Tangled 🇪🇺](https://tangled.sh/@overby.me) | | [Mirror](https://codeberg.org/Recommendations/Mirror_to_Codeberg) | [Microsoft GitHub 🔒🇺🇸](https://github.com/noverby), [GitLab 🇺🇸](https://gitlab.com/noverby) |
| ✅ | Music |  [Spotify 🇪🇺](https://spotify.com) | | 🆗 | [Deezer 🇪🇺](https://deezer.com) |
| ✅ | Audiobooks |  [LibreVox 🌐](https://librivox.org/) | | 🆗 | [Amazon Audible 🇺🇸](https://www.audible.com) |
| 🚧 | Payment | [MobilePay 🇪🇺](https://mobilepay.dk), [PayPal 🇺🇸](https://paypal.com) | [Wero 🇪🇺](https://wero-wallet.eu) | 🆗 | |
| 🚧 | Payment Medium | [Dankort 🔒🇪🇺](https://www.dankort.dk), [Visa 🔒🇺🇸](https://www.visa.com) | [Digital Euro 🏛️🇪🇺](https://www.ecb.europa.eu/paym/digital_euro/html/index.en.html), [GNU Taler 📖](https://taler.net) | |
| 🚧 | Donation | [Ko-fi 🇬🇧](https://ko-fi.com) | [Liberapay 🌐🇪🇺](https://liberapay.com) | [GNU Taler 📖🇪🇺](https://taler.net) | [Patreon 🔒🇺🇸](https://patreon.com) |
| ✅ | Translation | [DeepL 🔒🇪🇺](https://www.deepl.com) | | 🆗 | [Google Translate 🔒🇺🇸](https://translate.google.com) |

</details>

#### Social

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Messaging | [Etke.cc Matrix 🇪🇺](https://etke.cc)  | | [Matrix 🌐](https://matrix.org), [Matrix Bridges](https://matrix.org/ecosystem/bridges) | [Telegram 🔒🇦🇪](https://telegram.org), [Automattic Beeper 🔒🇺🇸](https://www.beeper.com), [Meta Messenger 🔒🇺🇸](https://messenger.com), [Meta WhatsApp 🔒🇺🇸](https://whatsapp.com) |
| ✅ | Event Hosting | [Smoke Signal Events 🌐](https://smokesignal.events), [Meetup 🔒🇪🇺](https://meetup.com) | | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub) | [Meta Facebook Events 🔒🇺🇸](https://facebook.com) |
| ✅ | Media Sharing | [Pixelfed 🇪🇺](https://pixelfed.social/niclasoverby) | [Flashes 🇪🇺](https://github.com/birdsongapps/Flashes) | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub) | [Meta Instagram 🔒🇺🇸](https://instagram.com) |
| ✅ | Discussion | [Lemmy World 🌐🇪🇺](https://lemmy.world) | | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub) | [Reddit 🔒🇺🇸](https://reddit.com), [Lemmy.ml 🌐🇪🇺](https://lemmy.world) |
| ✅ | Microblogging | [Mastodon 🌐🇪🇺](https://mas.to/niclasoverby), [Bluesky ⚖️🇺🇸](https://bsky.app/profile/overby.me) | | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub), [ATProtocol](https://atproto.com), [X-Cancel](https://xcancel.com) | [X-Twitter 🔒🇺🇸](https://x.com), [Meta Threads 🔒🇺🇸](https://www.threads.net) |
| ✅ | [Book Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [Bookwyrm 🐍🇪🇺](https://bookwyrm.social/user/niclasoverby) | [OpenLibrary 🌐📖](https://openlibrary.org) | [Amazon Goodreads 🔒🇺🇸](https://www.goodreads.com/niclasoverby) |
| ✅ | [Film Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Neodb 🐍](https://github.com/neodb-social/neodb) | | [OpenLibrary 🌐📖](https://openlibrary.org) | [Letterboxd 🔒🇳🇿](https://letterboxd.com/niclasoverby), [Amazon IMDB 🔒🇺🇸](https://www.imdb.com) |
| ✅ | [Music Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Spotify 🔒🇪🇺](https://open.spotify.com/user/1148979230) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [OpenLibrary 🌐📖](https://openlibrary.org) | |
| 🚫 | [Fitness Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Garmin Connect 🔒🇺🇸](https://connect.garmin.com) | [FitTrackee 🐍](https://github.com/SamR1/FitTrackee) | [GPX 📖](https://en.wikipedia.org/wiki/GPS_Exchange_Format) | [Strava 🔒🇺🇸](https://www.strava.com/athletes/116425039) |
| ✅ | [Food Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [HappyCow 👁️🔒🇺🇸](https://www.happycow.net/members/profile/niclasoverby) | [OpenVegeMap](https://github.com/Rudloff/openvegemap) | 🆗 | |
| ✅ | Collaboration | [AppFlowy 🦀](https://github.com/AppFlowy-IO/AppFlowy) | | [Import](https://docs.appflowy.io/docs/guides/import-from-notion) | [Notion 🔒🇺🇸](https://notion.so) |
| ✅ | [Online Encyclopedia](https://en.wikipedia.org/wiki/Online_encyclopedia) | [Wikipedia 🌐](https://en.wikipedia.org/wiki/User:Niclas_Overby) | [Ibis 🦀](https://github.com/Nutomic/ibis) | 🆗 | |
| ✅ | Dating | [Veggly 👁️🇧🇷](https://veggly.app) | | 🆗 | [Tinder 🔒🇺🇸](https://tinder.com) |

</details>

#### Cloud

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Cloud Provider | [Amazon AWS 🇺🇸](https://aws.amazon.com) | [Scaleway 🇪🇺](https://www.scaleway.com), [UpCloud 🇪🇺](https://www.upcloud.com) | | |
| ✅ | Bare Metal Hosting | [Hetzner 🇪🇺](https://hetzner.com) | | | |
| ✅ | Static Host | [Statichost 🇪🇺](https://statichost.eu) |  [FastFront 🇪🇺](https://www.fastfront.io) | | [Vercel 🇺🇸](https://vercel.com) |
| ✅ | Domain Registrar | [Simply 🇪🇺](https://simply.com) | | | |
| ✅ | Backend | [Nhost 🇪🇺](https://nhost.com) | [DFRNT 🇪🇺](https://dfrnt.com) | | |
| ✅ | Logging | [Bugfender 🇪🇺](https://bugfender.com) | | | [Sentry 🇺🇸](https://sentry.io) |
| ✅ | Analytics | [Counter.dev 🇪🇺](https://counter.dev) | | | [Vercel Analytics 🇺🇸](https://vercel.com/analytics) |
| ✅ | Content Delivery Network | [Bunny.net 🇪🇺](https://bunny.net) | | | |

</details>
