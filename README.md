# Personal Monorepo

## Overview
### Config
* [Home Manager Modules](https://codeberg.org/noverby/noverby/src/branch/main/modules/home-manager)
* [NixOS Modules](https://codeberg.org/noverby/noverby/src/branch/main/modules/nixos)
* [NixOS Devices](https://codeberg.org/noverby/noverby/src/branch/main/devices)
* [Devenv Shells](https://codeberg.org/noverby/noverby/src/branch/main/shells)

### Packages
* [Magic Package Manager](https://codeberg.org/noverby/noverby/src/branch/main/packages/magic.nix)
* [Mojo Toolchain](https://codeberg.org/noverby/noverby/src/branch/main/packages/mojo.nix)
* [Cavif-rs](https://codeberg.org/noverby/noverby/src/branch/main/packages/cavif-rs/default.nix)
* [Rcgen](https://codeberg.org/noverby/noverby/src/branch/main/packages/rcgen.nix)

### Projects
* [Homepage](https://codeberg.org/noverby/noverby/src/branch/main/projects/homepage)
* [Wiki](https://codeberg.org/noverby/noverby/src/branch/main/projects/wiki)
* [Mojo Wasm](https://codeberg.org/noverby/noverby/src/branch/main/projects/mojo-wasm)

## Stack
### State
 * ✅: Good for now
 * 🚧: WIP
 * 🚫: Blocked
 * ❓: Undecided

 ### Control
 * 🌐: Managed by [Nonprofit Org](https://en.wikipedia.org/wiki/Nonprofit_organization)
 * 🏛️: Managed by [Public Authority](https://en.wikipedia.org/wiki/Public_administration)
 * ⚖️: Managed by [Benefit Corp](https://en.wikipedia.org/wiki/Benefit_corporation)
 * 📖: [Open Standard](https://en.wikipedia.org/wiki/Open_standard)
 * 🏡: Self-hosted
 * 🔒: [Proprietary](https://en.wiktionary.org/wiki/proprietary)
 * ⏳: [Business Source License](https://en.wikipedia.org/wiki/Business_Source_License)

### Migration Compatibility
 * 🆗: Not needed
 * ⬅️: Backward compatible

 ### Language
 * 🔥: Mojo
 * 🐍: Python
 * ❄️: Nix
 * 🦀: Rust
 * 🐹: Go
 * 💣: C/C++
 * 🐒: ECMAScript

### Miscellaneous
* X➡️Y: Migrating from X to Y
* 👁️: [Sentientist](https://en.wikipedia.org/wiki/Sentientism)

### Hardware
| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Architecture | [X86-64 🔒](https://en.wikipedia.org/wiki/X86-64) | [RISC-V 📖](https://en.wikipedia.org/wiki/RISC-V), [ARM 🔒](https://en.wikipedia.org/wiki/ARM_architecture_family) | |
| 🚫 | Laptop | [Framework 13 🇺🇸](https://frame.work/products/laptop-diy-13-gen-intel), [Dell XPS 13 Plus 9320 🇺🇸](https://www.dell.com/support/home/da-dk/product-support/product/xps-13-9320-laptop) | [Tuxedo ARM Laptop 🇪🇺](https://www.tuxedocomputers.com/en/TUXEDO-on-ARM-is-coming.tuxedo) | |
| ✅ | Router | [Turris Omnia 🇪🇺](https://www.turris.com/en/products/omnia) | | |
| ✅ | Mobile | [Fairphone 4 🇪🇺](https://shop.fairphone.com/fairphone-4) | | [Google Pixel 7 Pro 🇺🇸](https://store.google.com/product/pixel_7_pro) |
| 🚫 | Watch | [Garmin Fenix 7 🔒🇺🇸](https://foundation.mozilla.org/en/privacynotincluded/garmin-fenix) | | [PineTime 🇭🇰](https://www.pine64.org/pinetime) |
| 🚫 | AR Glasses | [XReal Air 2 Pro 🔒🇨🇳](https://us.shop.xreal.com/products/xreal-air-2-pro) | | [XReal Light 🔒🇨🇳](https://www.xreal.com/light) |
| ✅ | Earphones | [Hyphen Aria 🇨🇭](https://rollingsquare.com/products/hyphen%C2%AE-aria) | | [Shokz Openfit 🇬🇧](https://shokz.com/products/openfit) |
| ✅ | E-book Reader | [reMarkable 2 🔒🇳🇴](https://remarkable.com/store/remarkable-2) | [PineNote 🇭🇰](https://pine64.org/devices/pinenote) | [reMarkable 1 🔒🇳🇴](https://remarkable.com/store/remarkable) |

### Standards
| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Application Binary Interface | [System V ABI 📖](https://wiki.osdev.org/System_V_ABI) | [CrABI 📖](https://github.com/rust-lang/rust/pull/105586) | |
| 🚧 | IoT Connectivity Standard | | [Matter 📖](https://en.wikipedia.org/wiki/Matter_(standard)) | |
| 🚧 | Wireless Media | [Google ChromeCast 🔒](https://en.wikipedia.org/wiki/Chromecast) | [MatterCast 📖](https://en.wikipedia.org/wiki/Matter_(standard)) | [Miracast 📖](https://en.wikipedia.org/wiki/Miracast) |
| ✅ | USB Interface | [USB4 📖](https://www.usb.org/usb4) | | [Thunderbolt 3 🔒](https://www.intel.com/content/www/us/en/architecture-and-technology/thunderbolt/thunderbolt-technology-general.html) |
| ✅ | Display Interface | [DisplayPort 📖](https://en.wikipedia.org/wiki/DisplayPort) | | [HDMI 2.1 🔒](https://en.wikipedia.org/wiki/HDMI) |
| ✅ | Image Codec | [PNG 📖](https://en.wikipedia.org/wiki/Portable_Network_Graphics), [JPEG 📖](https://en.wikipedia.org/wiki/JPEG) | [AVIF 📖](https://en.wikipedia.org/wiki/AVIF) | |
| ✅ | Audio Codec | [Opus 📖](https://opus-codec.org) | | [AAC 🔒](https://en.wikipedia.org/wiki/Advanced_Audio_Coding) |
| ✅ | Video Codec | [AV1 📖](https://aomedia.org/av1-features/get-started) | | [H.264 🔒](https://en.wikipedia.org/wiki/Advanced_Video_Coding) |
| 🚧 | Network Transport | [TCP 📖](https://en.wikipedia.org/wiki/Transmission_Control_Protocol) | [QUIC 📖](https://www.chromium.org/quic/) | |
| 🚧 | Web Protocol | [HTTP/2 📖](https://en.wikipedia.org/wiki/HTTP/2) | [HTTP/3 📖](https://en.wikipedia.org/wiki/HTTP/3) | [HTTP/1.1 📖](https://en.wikipedia.org/wiki/HTTP/1.1) |
| ✅ | GPU Compute | [Vulkan Compute 📖](https://www.vulkan.org) | | [OpenCL 📖](https://www.khronos.org/opencl) |
| ✅ | Graphics API | [Vulkan 📖](https://www.vulkan.org) | | [OpenGL 📖](https://www.opengl.org) |
| ✅ | Windowing | [Wayland 📖](https://wayland.freedesktop.org) | | [X11 📖](https://www.x.org) |
| ✅ | Heterogeneous Compute | [SYCL 📖](https://www.khronos.org/sycl) | | |
| 🚧 | Payment Systems | [Dankort 🔒](https://www.dankort.dk), [Visa 🔒](https://www.visa.com) | [Digital Euro 🏛️](https://www.ecb.europa.eu/paym/digital_euro/html/index.en.html), [GNU Taler 📖](https://taler.net) | |
| 🚫 | Tensor Operations | | | | |
| 🚫 | AI Inference | | | | |
| 🚧 | Satellite Navigation | [Galileo 🇪🇺](https://www.euspa.europa.eu/eu-space-programme/galileo), [GPS 🏛️🇺🇸](https://www.gps.gov) | | |
| 🚧 | Satellite Internet | | [Iris² 🏛️🇪🇺](https://defence-industry-space.ec.europa.eu/eu-space-policy/iris2_en) | [Starlink 🔒🇺🇸](https://www.starlink.com) |
| 🚧 | Object Notation | [JSON 📖](https://www.json.org) | [CBOR 📖](https://cbor.io) | |

### System Core
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Distro | [NixOS 🌐❄️](https://github.com/NixOS/nixpkgs) | | [OCI 📖](https://github.com/opencontainers/runtime-spec), [Distrobox](https://github.com/89luca89/distrobox) | [Fedora Silverblue](https://fedoraproject.org/silverblue) |
| ✅ | Kernel | [Linux 🌐💣](https://github.com/torvalds/linux) | [Asterinas 🦀](https://github.com/asterinas/asterinas), [Redox OS 🦀](https://gitlab.redox-os.org/redox-os/redox) | [Rust For Linux 🦀](https://rust-for-linux.com/) | |
| 🚧 | Libc | [Glibc 💣](https://www.gnu.org/software/libc) | [Musl 💣](https://www.musl-libc.org), [Relibc 🦀](https://github.com/redox-os/relibc) | [Gcompat 💣](https://git.adelielinux.org/adelie/gcompat) | |
| 🚫 | Init System | [Systemd 💣](https://github.com/systemd/systemd) | [Rustysd 🦀](https://github.com/KillingSpark/rustysd) | ⬅️ | |
| 🚧 | IPC | [Dbus 💣](https://gitlab.freedesktop.org/dbus/dbus) | [Busd 🦀](https://github.com/dbus2/busd) | ⬅️ | |
| ✅ | Filesystem | [Btrfs 📖💣](https://btrfs.wiki.kernel.org/index.php/Main_Page) | | 🆗 | [Ext4 📖💣](https://ext4.wiki.kernel.org/index.php/Main_Page) |
| 🚧 | Config Language | [Nix 🌐💣](https://github.com/NixOS/nix) | [Nickel 🦀](https://github.com/tweag/nickel), [Glistix 🦀](https://github.com/Glistix/glistix) | | |
| 🚧 | Package Manager | [Nix 🌐💣](https://github.com/NixOS/nix) | [Snix 🦀](https://git.snix.dev/snix/snix) | ⬅️ | |
| ✅ | Config Manager | [Home Manager 🌐](https://github.com/nix-community/home-manager) | [Organist](https://github.com/nickel-lang/organist) | | |

### CLI Tools
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Shell | [Nushell 🦀](https://github.com/nushell/nushell) | | [Rusty Bash 🦀](https://github.com/shellgei/rusty_bash) | [Bash 💣](https://git.savannah.gnu.org/cgit/bash.git) |
| ✅ | Core Utilities | [Nushell Builtins 🦀](https://github.com/nushell/nushell) | | [Uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Directory Usage | [Dust 🦀](https://github.com/bootandy/dust) | | [Uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Superuser | [Sudo-rs 🦀](https://github.com/memorysafety/sudo-rs) | | ⬅️ | [Sudo 💣](https://www.sudo.ws/repos/sudo) |
| ✅ | Fortune | [Fortune-kind 🦀](https://github.com/cafkafk/fortune-kind) | | ⬅️ | [Fortune-mod 💣](https://github.com/shlomif/fortune-mod) |
| ✅ | List Processes | [Procs 🦀](https://github.com/dalance/procs) | | 🆗 | [Ps 💣](https://gitlab.com/procps-ng/procps) |
| ✅ | List Files | [Nushell Builtins 🦀](https://github.com/nushell/nushell) | [Eza 🦀](https://github.com/eza-community/eza) | 🆗 | [Ls 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Find Files | [Fd 🦀](https://github.com/sharkdp/fd) | | [Uutils Findutils 🦀](https://github.com/uutils/findutils) | [Findutils 💣](https://git.savannah.gnu.org/cgit/findutils.git) |
| ✅ | Find Patterns | [Ripgrep 🦀](https://github.com/BurntSushi/ripgrep) | | 🆗 | [Grep 💣](https://git.savannah.gnu.org/cgit/grep.git) |
| ✅ | Diff | [Batdiff 🦀](https://github.com/eth-p/bat-extras) + [Delta 🦀](https://github.com/dandavison/delta) | [Difftastic 🦀](https://github.com/wilfred/difftastic) | [Uutils Diffutils 🦀](https://github.com/uutils/diffutils) | [Diffutils 💣](https://git.savannah.gnu.org/cgit/diffutils.git) |
| ✅ | Terminal Workspace | [Zellij 🦀](https://github.com/zellij-org/zellij) | | 🆗 | [Tmux 💣](https://github.com/tmux/tmux) |
| ✅ | Parallel Processing | [Rust Parallel 🦀](https://github.com/aaronriekenberg/rust-parallel) | | 🆗 | [GNU Parallel 💣](https://git.savannah.gnu.org/cgit/parallel.git) |
| ✅ | Process Monitor | [Bottom 🦀](https://github.com/ClementTsang/bottom) | | 🆗 | [Top 💣](https://gitlab.com/procps-ng/procps) |
| ✅ | Fuzzy Finder | [Television 🦀](https://github.com/alexpasmantier/television) | | 🆗 | [Fzf 🐹](https://github.com/junegunn/fzf) |

### Dev Tools
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Compiler Framework | [Mlir 💣](https://github.com/llvm/llvm-project/tree/main/mlir), [LLVM 💣](https://github.com/llvm/llvm-project) | [Cranelift 🦀](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift) | ⬅️ | |
| 🚧 | Linker | [Mold 💣](https://github.com/rui314/mold) | [Wild 🦀](https://github.com/davidlattimore/wild) | ⬅️ | [GNU ld 💣](https://sourceware.org/binutils) |
| ✅ | System Language | [Mojo 🔒🔥](https://github.com/modularml/mojo), [Rust 🦀](https://github.com/rust-lang/rust) | | [cxx 🦀](https://github.com/dtolnay/cxx), [bindgen 🦀](https://github.com/rust-lang/rust-bindgen) | |
| ✅ | Scripting Language | [Mojo 🔒🔥](https://github.com/modularml/mojo) | | [RustPython 🦀](https://github.com/RustPython/RustPython), [WASI 📖](https://github.com/WebAssembly/WASI), [Interface Types 📖](https://github.com/WebAssembly/interface-types/tree/main/proposals/interface-types) | [TypeScript](https://github.com/microsoft/TypeScript) |
| 🚧 | Version Control | [Jujutsu 🦀](https://github.com/jj-vcs/jj), [Git 💣](https://github.com/git/git) | [Gitoxide 🦀](https://github.com/Byron/gitoxide) | [Gix 🦀](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md#gix) | |
| ✅ | Merge | [Mergiraf 🦀](https://codeberg.org/mergiraf/mergiraf) | | ⬅️ | |
| ✅ | Build Script | [Just 🦀](https://github.com/casey/just) | | Rusty Make ([Rusty Bash 🦀](https://github.com/shellgei/rusty_bash)) | [GNU Make 💣](https://git.savannah.gnu.org/cgit/make.git) |
| ✅ | Editor | [Evil Helix 🦀](https://github.com/usagi-flow/evil-helix) | | 🆗 | [Helix 🦀](https://github.com/helix-editor/helix), [Neovim 💣](https://github.com/neovim/neovim) |
| ✅ | IDE | [Zed 🦀](https://github.com/zed-industries/zed) | | [LSP](https://github.com/microsoft/language-server-protocol), [DAP](https://github.com/Microsoft/debug-adapter-protocol), [BSP](https://github.com/build-server-protocol/build-server-protocol) | [VS Codium 🐒💣](https://github.com/VSCodium/vscodium) |
| ✅ | System Call Tracing | [Lurk 🦀](https://github.com/JakWai01/lurk), [Tracexec 🦀](https://github.com/kxxt/tracexec) | | 🆗 | [Strace 💣](https://github.com/strace/strace) |
| ✅ | Network Client | [Xh 🦀](https://github.com/ducaale/xh) | | 🆗 | [Curl 💣](https://github.com/curl/curl) |
| ✅ | Dev Environment | [Devenv 🦀️❄️](https://github.com/cachix/devenv) | | 🆗 | |
| ✅ | Environment Loader | [Direnv 🐹](https://github.com/direnv/direnv) | [Envy 🦀](https://github.com/mre/envy) | ⬅️ | |
| ✅ | Pager | [Tailspin 🦀](https://github.com/bensadeh/tailspin) | | 🆗 | [Less 💣](https://github.com/gwsw/less) |
| ✅ | Performance Profiler | [Samply 🦀](https://github.com/mstange/samply) | | 🆗 | [Perf 💣](https://perf.wiki.kernel.org/) |
| 🚧 | Bundler | [Webpack 🐒](https://github.com/webpack/webpack), [Turbopack 🦀](https://github.com/vercel/turbo) | [Rsbuild 🦀](https://github.com/web-infra-dev/rsbuild), [Farm 🦀](https://github.com/farm-fe/farm) | 🆗 | |
| ✅ | Certificate Generation | [Rcgen 🦀](https://github.com/rustls/rcgen) | | 🆗 | [Mkcert 🐹](https://github.com/FiloSottile/mkcert) |

### Infrastructure
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | WebAssembly Runtime | [Wasmtime 🦀](https://github.com/bytecodealliance/wasmtime) | | [WASI 📖](https://wasi.dev) | |
| ✅ | ECMAScript Runtime | [Deno 🦀](https://github.com/denoland/deno) | | [Deno Node APIs](https://docs.deno.com/runtime/reference/node_apis) | [Node.js 💣](https://github.com/nodejs/node) |
| ✅ | Container Runtime | [Youki 🦀](https://github.com/containers/youki) | | [OCI 📖](https://github.com/opencontainers/runtime-spec) | [Runc 🐹](https://github.com/opencontainers/runc) |
| 🚧 | Meta Database | [Hasura 🦀](https://github.com/hasura/graphql-engine) | [Surrealdb ⏳🦀](https://github.com/surrealdb/surrealdb) | [GraphQL](https://graphql.org) |
| 🚧 | Database | [Postgres 💣](https://github.com/postgres/postgres) | [Tikv 🦀](https://github.com/tikv/tikv) | 🆗 | |
| 🚧 | Storage Engine | | [Sled 🦀](https://github.com/spacejam/sled) | 🆗 | [RocksDB 💣](https://github.com/facebook/rocksdb) |
| 🚫 | Web Server | [Nginx 💣](https://github.com/nginx/nginx) | [Moella 🦀](https://github.com/Icelk/moella) | | |
| ✅ | VPN | [Tailscale 🐹](https://github.com/tailscale/tailscale) | [Innernet 🦀](https://github.com/tonarino/innernet) | | |

### Libraries
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Compression | [Zlib-rs 🦀](https://github.com/memorysafety/zlib-rs) | | ⬅️ | [Zlib 💣](https://github.com/madler/zlib) |
| ✅ | TLS Protocol |  [Rustls 🦀](https://github.com/rustls/rustls) | | 🆗 | [Openssl 💣](https://github.com/openssl/openssl) |
| ✅ | HTTP Protocol | [Hyper 🦀](https://github.com/hyperium/hyper) | | 🆗 | [Nghttp2 💣](https://github.com/nghttp2/nghttp2), [Nghttp3 💣](https://github.com/ngtcp2/nghttp3) |
| ✅ | HTTP Client | [Reqwest 🦀](https://github.com/seanmonstar/reqwest) | | 🆗 | [Curl 💣](https://github.com/curl/curl) |
| ✅ | SSH Protocol | [Russh 🦀](https://github.com/warp-tech/russh) | | 🆗 | [OpenSSH 💣](https://github.com/openssh/openssh-portable) |
| ✅ | Font Rendering | [Cosmic-text 🦀](https://github.com/pop-os/cosmic-text) | | 🆗 | [HarfBuzz 💣](https://github.com/harfbuzz/harfbuzz), [FreeType 💣](https://github.com/freetype/freetype) |
| 🚧 | Browser Engine | [Gecko 🦀💣](https://en.wikipedia.org/wiki/Gecko_(software)) | [Servo 🦀](https://github.com/servo/servo) | ⬅️ | |

### GUI
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Design System | [Material You 🔒](https://m3.material.io) | | 🆗 | [Material Design 2 🔒](https://m2.material.io) |
| ✅ | 2D Renderer | [Wgpu 🦀](https://github.com/gfx-rs/wgpu) | | 🆗 | [Skia 💣](https://github.com/google/skia), [Cairo 💣](https://www.cairographics.org) |
| 🚧 | GUI Toolkit | [React 🐒](https://react.dev) | [WIP Toolkit 🔥](https://codeberg.org/noverby/noverby/src/branch/main/projects/mojo-wasm), [Dixous 🦀](https://github.com/dioxusLabs/dioxus) | [Web Component 📖](https://www.webcomponents.org/) | |
| 🚧 | Component Library | [MUI 🐒](https://mui.com) | [Dioxus Components 🦀](https://github.com/DioxusLabs/components) | 🆗 | |
| ✅ | Desktop Environment | [Cosmic Epoch 🦀](https://github.com/pop-os/cosmic-epoch) | | 🆗 | [Gnome Shell 💣](https://gitlab.gnome.org/GNOME/gnome-shell) |
| ✅ | File Manager | [Cosmic Files 🦀](https://github.com/pop-os/cosmic-files) | | 🆗 | [GNOME Files 💣](https://gitlab.gnome.org/GNOME/nautilus) |
| ✅ | Web Browser | [Zen Browser 💣🦀](https://zen-browser.app) | [Verso 🦀](https://github.com/versotile-org/verso) | [Chrome Extension API 🔒](https://developer.chrome.com/docs/extensions/reference) | [Firefox 💣🦀](https://github.com/mozilla/gecko-dev), [Unbraved Brave 💣🦀](https://github.com/MulesGaming/brave-debullshitinator) |
| ✅ | App Browser | [Cosmic Store 🦀](https://github.com/pop-os/cosmic-store) | | 🆗 | [GNOME Software 💣](https://gitlab.gnome.org/GNOME/gnome-software) |
| 🚫 | GUI Package Manager | [Flatpak 💣](https://github.com/flatpak/flatpak) | | 🆗 | [Snap 🔒](https://github.com/canonical/snapd), [AppImage 💣](https://github.com/AppImage) |

### Media
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Media Player | [Cosmic Player 🦀](https://github.com/pop-os/cosmic-player) | | [FFMPEG 💣](https://github.com/FFmpeg/FFmpeg), [GStreamer 💣](https://gitlab.freedesktop.org/gstreamer) | [Mpv 💣](https://github.com/mpv-player/mpv) |
| 🚧 | Raster Graphics | [GIMP 💣](https://gitlab.gnome.org/GNOME/gimp) | [Graphite 🦀](https://github.com/GraphiteEditor/Graphite) | 🆗 | |
| ✅ | Vector Graphics | [Graphite 🦀](https://github.com/GraphiteEditor/Graphite) | | 🆗 | [Inkscape 💣](https://gitlab.com/inkscape/inkscape) |
| ✅ | Typesetting | [Typst 🦀](https://github.com/typst) | | 🆗 | [LaTeX 💣](https://github.com/latex3/latex3) |
| ✅ | Optimize Image | [Oxipng 🦀](https://github.com/shssoichiro/oxipng) | | 🆗 | [Optpng 💣](https://optipng.sourceforge.net) |

### Mobile
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | OS | [/e/OS 🌐🇪🇺](https://e.foundation/e-os) | | [MicroG 🌐](https://microg.org), [Magisk](https://github.com/topjohnwu/Magisk) | [GrapheneOS 🇨🇦](https://grapheneos.org) |
| ✅ | Launcher | [Olauncher](https://github.com/tanujnotes/Olauncher) | | 🆗 | [Minimalist Phone 🔒](https://www.minimalistphone.com) |
| ✅ | Keyboard | [Thumb-Key](https://github.com/dessalines/thumb-key) | | 🆗 | [OpenBoard](https://github.com/openboard-team/openboard) |
| ✅ | Alarm | [Chrono](https://github.com/vicolo-dev/chrono) | | 🆗 | [Sleep 🔒](https://sleep.urbandroid.org) |
| ✅ | Browser | [Fennec 💣🦀](https://f-droid.org/en/packages/org.mozilla.fennec_fdroid) | | 🆗 | [Mull 💣🦀](https://github.com/mull-project/mull) |
| ✅ | Maps | [CoMaps 💣](https://comaps.app) | | [Openstreetmap 🌐📖](https://www.openstreetmap.org) | [Organic Maps 💣](https://organicmaps.app), [Google Maps 🔒🇺🇸](https://maps.google.com)|
| ✅ | Distraction Blockers | [TimeLimit](https://codeberg.org/timelimit/timelimit-android), [LeechBlock NG](https://github.com/proginosko/LeechBlockNG), [Adguard DNS 🇪🇺](https://adguard.com) | | 🆗 | |
| ✅ | Authenticator | [Aegis](https://getaegis.app) | | [HOTP 📖](https://en.wikipedia.org/wiki/HMAC-based_One-time_Password_algorithm), [TOTP 📖](https://en.wikipedia.org/wiki/Time-based_One-time_Password_algorithm) | |


### Services
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Password Manager | [Bitwarden.eu 🇪🇺](https://bitwarden.eu) | | 🆗 | [Bitwarden.com 🇺🇸](https://bitwarden.com) |
| ✅ | Mail | [Tuta Mail 🇪🇺](https://tuta.com) | | [Mail Import](https://tuta.com/blog/tuta-release-update-february) | [Proton Mail 🌐🇨🇭➡️🇪🇺](https://proton.me/mail) |
| ✅ | Calendar | [Tuta Calendar 🇪🇺](https://tuta.com) | | [iCalendar 📖](https://en.wikipedia.org/wiki/ICalendar) | [Proton Calendar 🔒🌐🇨🇭➡️🇪🇺](https://proton.me/calendar) |
| ✅ | Storage | [Syncthing 🐹🏡🇪🇺](https://github.com/syncthing/syncthing) | [Tuta Drive 🇪🇺](https://tuta.com/blog/pqdrive-project) | 🆗 | [Proton Drive 🌐🇨🇭➡️🇪🇺](https://proton.me/drive) |
| ✅ | VPN | [Adguard VPN 🇪🇺](https://adguard.com) | | 🆗 | [Proton VPN 🌐🇨🇭➡️🇪🇺](https://proton.me/vpn) |
| ✅ | DNS | [Adguard DNS 🇪🇺](https://adguard.com) | | 🆗 | [NextDNS 🔒🇺🇸](https://nextdns.io) |
| ✅ | Search Engine | [StartPage 🔒🇪🇺](https://startpage.com) | [Stract 🦀🇪🇺](https://github.com/StractOrg/stract) | [Search Shortcuts](https://support.mozilla.org/en-US/kb/assign-shortcuts-search-engines) | [Kagi 🔒🇺🇸](https://kagi.com), [DuckDuckGo 🔒🇺🇸](https://duckduckgo.com) |
| ✅ | LLM | [Ollama 🐹🏡🇪🇺](https://github.com/ollama/ollama), [Mistral 🇪🇺](https://mistral.ai) | [Lumo 🇪🇺](https://lumo.proton.me) | 🆗 | [Claude 🔒🇺🇸](https://claude.ai), [OpenAI 🔒🇺🇸](https://openai.com) |
| ✅ | Version Control | [Codeberg 🇪🇺](https://codeberg.org) | | [Mirror](https://codeberg.org/Recommendations/Mirror_to_Codeberg) | [Microsoft GitHub 🔒🇺🇸](https://github.com), [GitLab 🇺🇸](https://gitlab.com) |
| ✅ | Music |  [Spotify 🇪🇺](https://spotify.com) | | 🆗 | [Deezer 🇪🇺](https://deezer.com) |
| ✅ | Audiobooks |  [LibreVox 🌐](https://librivox.org/) | | 🆗 | [Amazon Audible 🇺🇸](https://www.audible.com) |
| 🚧 | Payment | [MobilePay 🇪🇺](https://mobilepay.dk), [PayPal 🇺🇸](https://paypal.com) | [Wero 🇪🇺](https://wero-wallet.eu) | 🆗 | |
| 🚧 | Donation | [Ko-fi 🇬🇧](https://ko-fi.com) | [Liberapay 🌐🇪🇺](https://liberapay.com) | [GNU Taler 📖🇪🇺](https://taler.net) | [Patreon 🔒🇺🇸](https://patreon.com) |
| ✅ | Translation | [DeepL 🔒🇪🇺](https://www.deepl.com) | | 🆗 | [Google Translate 🔒🇺🇸](https://translate.google.com) |

### Social
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Messaging | [Etke.cc Matrix 🇪🇺](https://etke.cc)  | | [Matrix 🌐](https://matrix.org), [Matrix Bridges](https://matrix.org/ecosystem/bridges) | [Telegram 🔒🇦🇪](https://telegram.org), [Automattic Beeper 🔒🇺🇸](https://www.beeper.com), [Meta Messenger 🔒🇺🇸](https://messenger.com), [Meta WhatsApp 🔒🇺🇸](https://whatsapp.com) |
| ✅ | Events | [Smoke Signal Events 🌐](https://smokesignal.events), [Meetup 🔒🇪🇺](https://meetup.com) | | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub) | [Meta Facebook Events 🔒🇺🇸](https://facebook.com) |
| ✅ | Media Sharing | [Pixelfed 🇪🇺](https://pixelfed.social/niclasoverby) | | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub) | [Meta Instagram 🔒🇺🇸](https://instagram.com) |
| ✅ | Discussion | [Lemmy World 🌐🇪🇺](https://lemmy.world) | | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub) | [Reddit 🔒🇺🇸](https://reddit.com), [Lemmy.ml 🌐🇪🇺](https://lemmy.world) |
| ✅ | Microblogging | [Mastodon 🌐🇪🇺](https://mas.to/niclasoverby), [Bluesky ⚖️🇺🇸](https://bsky.app/profile/overby.me) | | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub), [ATProtocol](https://atproto.com), [X-Cancel](https://xcancel.com) | [X-Twitter 🔒🇺🇸](https://x.com), [Meta Threads 🔒🇺🇸](https://www.threads.net) |
| ✅ | [Book Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [Bookwyrm 🐍🇪🇺](https://bookwyrm.social/user/niclasoverby) | [OpenLibrary 🌐📖](https://openlibrary.org) | [Amazon Goodreads 🔒🇺🇸](https://www.goodreads.com/niclasoverby) |
| ✅ | [Film Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Neodb 🐍](https://github.com/neodb-social/neodb) | | [OpenLibrary 🌐📖](https://openlibrary.org) | [Letterboxd 🔒🇳🇿](https://letterboxd.com/niclasoverby), [Amazon IMDB 🔒🇺🇸](https://www.imdb.com) |
| ✅ | [Music Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Spotify 🔒🇪🇺](https://open.spotify.com/user/1148979230) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [OpenLibrary 🌐📖](https://openlibrary.org) | |
| 🚫 | [Fitness Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Garmin Connect 🔒🇺🇸](https://connect.garmin.com) | [FitTrackee 🐍](https://github.com/SamR1/FitTrackee) | [GPX 📖](https://en.wikipedia.org/wiki/GPS_Exchange_Format) | [Strava 🔒🇺🇸](https://www.strava.com/athletes/116425039) |
| ✅ | [Food Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [HappyCow 👁️🔒🇺🇸](https://www.happycow.net/members/profile/niclasoverby) | [OpenVegeMap](https://github.com/Rudloff/openvegemap) | 🆗 | |
| ✅ | Collaboration | [AppFlowy 🦀](https://github.com/AppFlowy-IO/AppFlowy) | | [Import](https://docs.appflowy.io/docs/guides/import-from-notion) | [Notion 🔒🇺🇸](https://notion.so) |
| ✅ | [Online Encyclopedia](https://en.wikipedia.org/wiki/Online_encyclopedia) | [Wikipedia 🌐](https://en.wikipedia.org/wiki/User:Niclas_Overby) | [Ibis 🦀](https://github.com/Nutomic/ibis) | 🆗 | |

### Cloud
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Cloud Provider | [Amazon AWS 🇺🇸](https://aws.amazon.com) | [Scaleway 🇪🇺](https://www.scaleway.com) | | |
| ✅ | Bare Metal Hosting | [Hetzner 🇪🇺](https://hetzner.com) | | | |
| ✅ | Static Host | [Statichost 🇪🇺](https://statichost.eu) |  [FastFront 🇪🇺](https://www.fastfront.io) | | [Vercel 🇺🇸](https://vercel.com) |
| ✅ | Domain Registrar | [Simply 🇪🇺](https://simply.com) | | | |
| ✅ | Backend | [Nhost 🇪🇺](https://nhost.com) | [DFRNT 🇪🇺](https://dfrnt.com) | | |
| ✅ | Logging | [Bugfender 🇪🇺](https://bugfender.com) | | | [Sentry 🇺🇸](https://sentry.io) |
| ✅ | Analytics | [Counter.dev 🇪🇺](https://counter.dev) | | | [Vercel Analytics 🇺🇸](https://vercel.com/analytics) |
