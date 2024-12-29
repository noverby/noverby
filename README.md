# Personal Monorepo

## Projects
* [Nix Config](https://github.com/noverby/noverby/tree/master/config)
* [Homepage](https://github.com/noverby/noverby/tree/master/projects/homepage)
* [Wiki](https://github.com/noverby/noverby/tree/master/projects/wiki)

## Stack
### State
 * ✅: Good for now
 * 🚧: Transitioning
 * 🚫: Blocked
 * ❓: Undecided

 ### Ownership
 * 🌐: Magaged by non-profit
 * 📖: Open standard
 * 🏡: Self-hosted
 * 🔒: Proprietary

 ### Language
 * 🔥: Mojo
 * 🐍: Python
 * ❄️: Nix
 * 🦀: Rust
 * 💣: C/C++

### Compatibility
 * 🆗: Not needed
 * ⬅️: Backward compatible

### Hardware
| Status | Component | R&D | Current | Legacy |
|:-:|-|-|-|-|
| 🚧 | Architecture | [RISC-V 📖](https://en.wikipedia.org/wiki/RISC-V), [ARM 🔒](https://en.wikipedia.org/wiki/ARM_architecture_family) | [X86-64 🔒](https://en.wikipedia.org/wiki/X86-64) | |
| 🚫 | Laptop | [Tuxedo ARM Laptop 🇪🇺](https://www.tuxedocomputers.com/en/TUXEDO-on-ARM-is-coming.tuxedo) | [Framework 13 🇺🇸](https://frame.work/products/laptop-diy-13-gen-intel) | [Dell XPS 13 Plus 9320 🇺🇸](https://www.dell.com/support/home/da-dk/product-support/product/xps-13-9320-laptop) |
| ✅ | Mobile | | [Fairphone 4 🇪🇺](https://shop.fairphone.com/fairphone-4) | [Google Pixel 7 Pro 🇺🇸](https://store.google.com/product/pixel_7_pro) |
| 🚫 | Watch | | [Garmin Fenix 7 🇺🇸](https://www.garmin.com/en-US/p/735520) | [PineTime 🇭🇰](https://www.pine64.org/pinetime) |
| 🚫 | AR Glasses | | [XReal Air 2 Pro 🔒🇨🇳](https://us.shop.xreal.com/products/xreal-air-2-pro) | [XReal Light 🔒🇨🇳](https://www.xreal.com/light/) |
| ✅ | Earphones | | [Hyphen Aria 🇨🇭](https://rollingsquare.com/products/hyphen%C2%AE-aria) | [Shokz Openfit 🇬🇧](https://shokz.com/products/openfit) |
| ✅ | E-book Reader | | [reMarkable 2 🔒🇳🇴](https://remarkable.com/store/remarkable-2) | [reMarkable 1 🔒🇳🇴](https://remarkable.com/store/remarkable) |

### Standards
| Status | Component | R&D | Current | Legacy |
|:-:|-|-|-|-|
| 🚧 | IoT Connectivity Standard | [Matter 📖](https://en.wikipedia.org/wiki/Matter_(standard)) | | |
| 🚧 | Wireless Media | [MatterCast 📖](https://en.wikipedia.org/wiki/Matter_(standard)) | [ChromeCast](https://en.wikipedia.org/wiki/Chromecast) | [Miracast](https://en.wikipedia.org/wiki/Miracast) |

### Base
| Status | Component | R&D | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Config Language | [Nickel 🦀](https://github.com/tweag/nickel) | [Nix 🌐💣](https://github.com/NixOS/nix) | [Organist](https://github.com/nickel-lang/organist) | |
| 🚧 | Package Manager | [Tvix 🦀](https://github.com/tvlfyi/tvix) | [Nix 🌐💣](https://github.com/NixOS/nix) | ⬅️ | |
| 🚧 | Web Runtime | [Deno 🦀](https://github.com/denoland/deno) | [Node.js 💣](https://github.com/nodejs/node) | [Node.js API](https://nodejs.org/api) |
| ✅ | Distro | | [NixOS 🌐❄️](https://github.com/NixOS/nixpkgs) | [OCI 📖](https://github.com/opencontainers/runtime-spec), [Distrobox](https://github.com/89luca89/distrobox) | [Fedora Silverblue](https://fedoraproject.org/silverblue/) |
| ✅ | Kernel | [Asterinas 🦀](https://github.com/asterinas/asterinas), [Redox OS 🦀](https://gitlab.redox-os.org/redox-os/redox) | [Linux 🌐💣](https://github.com/torvalds/linux) | | |
| ✅ | Init System | | [Systemd 💣](https://github.com/systemd/systemd) | | |
| ✅ | IPC | [Busd 🦀](https://github.com/dbus2/busd) | [Dbus 💣](https://gitlab.freedesktop.org/dbus/dbus/) | | |
| 🚫 | Container CLI | | | [OCI 📖](https://github.com/opencontainers/runtime-spec) | [Docker](https://github.com/docker) |
| 🚧 | Container Runtime | | [Youki 🦀](https://github.com/containers/youki) | [OCI 📖](https://github.com/opencontainers/runtime-spec) | [Runc](https://github.com/opencontainers/runc) |
| ✅ | Typesetting | | [Typst 🦀](https://github.com/typst/) | 🆗 | [LaTeX 💣](https://github.com/latex3/latex3) |

### Shell
| Status | Component | R&D | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Shell | | [Nushell 🦀](https://github.com/nushell/nushell) | ❓ | [Bash 💣](https://git.savannah.gnu.org/cgit/bash.git) |
| ✅ | Core Utilities | | [Nushell Builtins 🦀](https://github.com/nushell/nushell) | [uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Directory Usage | | [Dust 🦀](https://github.com/bootandy/dust) | [uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Superuser | | [Sudo-rs 🦀](https://github.com/memorysafety/sudo-rs) | ⬅️ | [Sudo 💣](https://www.sudo.ws/repos/sudo) |
| ✅ | Fortune | | [Fortune-kind 🦀](https://github.com/cafkafk/fortune-kind) | ⬅️ | [Fortune-mod 💣](https://github.com/shlomif/fortune-mod) |
| ✅ | Find Files | | [Fd 🦀](https://github.com/sharkdp/fd) | 🆗 | [Findutils 💣](https://git.savannah.gnu.org/cgit/findutils.git) |
| ✅ | Find Patterns | | [Ripgrep 🦀](https://github.com/BurntSushi/ripgrep) | 🆗 | [Grep 💣](https://git.savannah.gnu.org/cgit/grep.git) |
| ✅ | Terminal Workspace | | [Zellij 🦀](https://github.com/zellij-org/zellij) | 🆗 | [Tmux 💣](https://github.com/tmux/tmux) |
| ✅ | Network Client | | [Xh 🦀](https://github.com/ducaale/xh) | ❓ | [Curl 💣](https://github.com/curl/curl) |
| 🚫 | Environment Loader | [Envy 🦀](https://github.com/mre/envy) | [Direnv](https://github.com/direnv/direnv) | ⬅️ | |

### Dev
| Status | Component | R&D | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Compiler Framework | [Cranelift 🦀](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift) | [Mlir 💣](https://github.com/llvm/llvm-project/tree/main/mlir/), [LLVM 💣](https://github.com/llvm/llvm-project) | ⬅️ | |
| 🚧 | Application binary interface | [CrABI 📖](https://github.com/rust-lang/rust/pull/105586) | C ABI | ⬅️ | |
| ✅ | System Language | | [Mojo 🔒🔥](https://github.com/modularml/mojo), [Rust 🦀](https://github.com/rust-lang/rust) | [cxx 🦀](https://github.com/dtolnay/cxx), [bindgen 🦀](https://github.com/rust-lang/rust-bindgen) | [Go](https://github.com/golang/go) |
| ✅ | Scripting Language | | [Mojo 🔒🔥](https://github.com/modularml/mojo) | [RustPython 🦀](https://github.com/RustPython/RustPython), [WASI 📖](https://github.com/WebAssembly/WASI), [Interface Types 📖](https://github.com/WebAssembly/interface-types/tree/main/proposals/interface-types) | [TypeScript](https://github.com/microsoft/TypeScript) |
| 🚫 | Version Control | [Gitoxide 🦀](https://github.com/Byron/gitoxide) | [Git 💣](https://github.com/git/git) | ⬅️ ️️️️| |
| ✅ | Build Script| | [Just 🦀](https://github.com/casey/just) | ❓ | [GNU Make 💣](https://git.savannah.gnu.org/cgit/make.git) |
| ✅ | Editor | | [Helix 🦀](https://github.com/helix-editor/helix) | 🆗 | [Neovim 💣](https://github.com/neovim/neovim) |
| ✅ | IDE | | [Zed 🦀](https://github.com/zed-industries/zed) | [LSP](https://github.com/microsoft/language-server-protocol), [DAP](https://github.com/Microsoft/debug-adapter-protocol), [BSP](https://github.com/build-server-protocol/build-server-protocol) | [VS Codium](https://github.com/VSCodium/vscodium) |
| ✅ | System Call Tracing | | [Lurk 🦀](https://github.com/JakWai01/lurk) | 🆗 | [Strace 💣](https://github.com/strace/strace) |
| ✅ | Optimize PNG | | [Oxipng 🦀](https://github.com/shssoichiro/oxipngc) | 🆗 | [Optpng 💣](https://optipng.sourceforge.net) |
| 🚫 | Meta Database | [Surrealdb 🔒🦀](https://github.com/surrealdb/surrealdb) | [Hasura 🦀](https://github.com/hasura/graphql-engine) | [GraphQL](https://graphql.org) |
| 🚫 | Database | [Tikv 🦀](https://github.com/tikv/tikv) | [Postgres 💣](https://github.com/postgres/postgres) | ❓ | |
| 🚫 | Storage Engine | [Sled 🦀](https://github.com/spacejam/sled) | | ❓ | [RocksDB 💣](https://github.com/facebook/rocksdb) |

### Libraries
| Status | Component | R&D | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Compression | [Zlib-rs 🦀](https://github.com/memorysafety/zlib-rs) | [Zlib 💣](https://github.com/madler/zlib) | ⬅️ | |
| 🚧 | TLS Protocol | [Rustls 🦀](https://github.com/rustls/rustls) | [Openssl 💣](https://github.com/openssl/openssl) | ⬅️ | |
| 🚧 | HTTP Protocol | [Hyper 🦀](https://github.com/hyperium/hyper) | [Nghttp2 💣](https://github.com/nghttp2/nghttp2), [Nghttp3 💣](https://github.com/ngtcp2/nghttp3) | ⬅️ | |
| 🚧 | HTTP Client | [Reqwest 🦀](https://github.com/seanmonstar/reqwest) | [Curl 💣](https://github.com/curl/curl) | ⬅️ | |
| 🚧 | Font Rendering | [Cosmic-text 🦀](https://github.com/pop-os/cosmic-text) | [HarfBuzz 💣](https://github.com/harfbuzz/harfbuzz), [FreeType 💣](https://github.com/freetype/freetype) | ⬅️ | |
| 🚧 | Browser Engine | [Servo 🦀](https://github.com/servo/servo) | [Gecko 🦀💣](https://en.wikipedia.org/wiki/Gecko_(software)) | ⬅️ | |

### GUI
| Status | Component | R&D | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Design System | | [Material You](https://m3.material.io) | | [Material Design 2](https://m2.material.io) |
| 🚧 | GUI Toolkit | [Dixous 🦀](https://github.com/dioxusLabs/dioxus) | WIP Toolkit 🔥 | [Web Component 📖](https://kagi.com/search?q=Web+Components) | |
| ✅ | Desktop Environment | | [Cosmic Epoch 🦀](https://github.com/pop-os/cosmic-epoch) | | [Gnome Shell 💣](https://gitlab.gnome.org/GNOME/gnome-shell) |
| ✅ | File Manager | | [Cosmic Files 🦀](https://github.com/pop-os/cosmic-files) | | [GNOME Files 💣](https://gitlab.gnome.org/GNOME/nautilus) |
| 🚫 | Web Browser | [Verso 🦀](https://github.com/versotile-org/verso) | [Unbraved Brave 💣🦀](https://github.com/MulesGaming/brave-debullshitinator) | [Chrome Extension API](https://developer.chrome.com/docs/extensions/reference) | [Firefox 🦀💣](https://github.com/mozilla/gecko-dev) |
| ✅ | Media Player | [Cosmic Player 🦀](https://github.com/pop-os/cosmic-player) | [Mpv 💣](https://github.com/mpv-player/mpv) | [FFMPEG 💣](https://github.com/FFmpeg/FFmpeg), [GStreamer 💣](https://gitlab.freedesktop.org/gstreamer/) | |
| ✅ | GUI Package Manager | | [Flatpak 💣](https://github.com/flatpak/flatpak) | 🆗 | [Snap🔒](https://github.com/canonical/snapd), [AppImage 💣](https://github.com/AppImage) |
| ✅ | App Browser | | [Cosmic Store 🦀](https://github.com/pop-os/cosmic-store) | 🆗 | [GNOME Software 💣](https://gitlab.gnome.org/GNOME/gnome-software) |

### Media
| Status | Component | R&D | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Image Editing | | [GIMP 💣](https://gitlab.gnome.org/GNOME/gimp) | | |
| ✅ | Vector Graphics | | [Inkscape 💣](https://gitlab.com/inkscape/inkscape) | | |

### Mobile
| Status | Component | R&D | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| ✅ | OS | | [/e/OS 🌐🇪🇺](https://e.foundation/e-os) | [MicroG](https://microg.org) | [GrapheneOS 🇨🇦](https://grapheneos.org) |
| ✅ | Launcher | | [Olauncher](https://github.com/tanujnotes/Olauncher) | | [Minimalist Phone🔒](https://www.minimalistphone.com/) |
| ✅ | Keyboard | | [Thumb-Key](https://github.com/dessalines/thumb-key) | | [OpenBoard](https://github.com/openboard-team/openboard) |
| ✅ | Alarm | | [Chrono](https://github.com/vicolo-dev/chrono) | | [Sleep🔒](https://sleep.urbandroid.org/) |
| ✅ | Browser | | [Mull 🦀💣](https://github.com/mull-project/mull) | | |

### Services
| Status | Component | R&D | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Password Manager | | [Bitwarden.eu 🇪🇺](https://bitwarden.eu) | | [Bitwarden.com 🇺🇸](https://bitwarden.com) |
| ✅ | Version Control | | [Codeberg 🇪🇺](https://codeberg.org) | | [GitHub 🔒🇺🇸](https://github.com), [GitLab 🔒🇺🇸](https://gitlab.com) |
| ✅ | Mail | | [Tuta Mail 🇪🇺](https://tuta.com) | | [Proton Mail 🌐🇨🇭](https://proton.me/mail) |
| ✅ | Calendar | | [Tuta Calendar 🇪🇺](https://tuta.com) | | [Proton Calendar 🌐🇨🇭](https://proton.me/calendar) |
| ✅ | Storage | [Tuta Drive 🇪🇺](https://tuta.com/blog/pqdrive-project) | [Syncthing 🏡🇪🇺](https://github.com/syncthing/syncthing) | | [Proton Drive 🌐🇨🇭](https://proton.me/drive) |
| ✅ | VPN | | [Adguard VPN 🇪🇺](https://adguard.com) | | [Proton VPN 🌐🇨🇭](https://proton.me/vpn) |
| ✅ | DNS | | [Adguard DNS 🇪🇺](https://adguard.com) | | [NextDNS 🔒🇺🇸](https://nextdns.io) |
| ✅ | Search Engine | [Stract 🇪🇺](https://github.com/StractOrg/stract) | [StartPage 🔒🇪🇺](https://startpage.com) | [Search Shortcuts](https://support.mozilla.org/en-US/kb/assign-shortcuts-search-engines) | [Kagi 🔒🇺🇸](https://kagi.com), [DuckDuckGo 🔒🇺🇸](https://duckduckgo.com) |
| ✅ | LLM | | [Ollama 🏡🇪🇺](https://github.com/ollama/ollama) | | [Claude 🔒🇺🇸](https://claude.ai), [OpenAI 🔒🇺🇸](https://openai.com) |
| ✅ | Microblogging | | [Mastodon 🌐🇪🇺](https://mas.to/niclasoverby), [Bluesky 🇺🇸](https://bsky.app/profile/overby.me) | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub), [ATProtocol](https://atproto.com), [X-Cancel](https://xcancel.com) | [X-Twitter 🔒🇺🇸](https://x.com) |
| ✅ | Messaging | | [Matrix 🌐](https://matrix.org), [Beeper 🔒🇺🇸](https://www.beeper.com) | [Matrix Bridges](https://matrix.org/ecosystem/bridges) | [Telegram 🔒🇦🇪](https://telegram.org) |
| ✅ | Media Sharing | | [Pixelfed 🇪🇺](https://pixelfed.social/niclasoverby) | | [Instagram 🔒🇺🇸](https://instagram.com) |
| 🚫 | [Book Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Neodb 🐍](https://github.com/neodb-social/neodb), [Bookwyrm 🐍🇪🇺](https://bookwyrm.social/user/niclasoverby) | [Goodreads 🔒🇺🇸](https://www.goodreads.com/niclasoverby) | [OpenLibrary 🌐📖](https://openlibrary.org) | |
| 🚫 | [Film Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [Letterboxd 🔒🇺🇸](https://letterboxd.com/niclasoverby) | [OpenLibrary 🌐📖](https://openlibrary.org) | |
| 🚫 | [Music Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [Spotify 🔒🇺🇸](https://open.spotify.com/user/1148979230) | [OpenLibrary 🌐📖](https://openlibrary.org) | |
| 🚫 | [Fitness Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [FitTrackee 🐍](https://github.com/SamR1/FitTrackee) | [Strava 🔒🇺🇸](https://www.strava.com/athletes/116425039) | | |
| 🚫 | [Food Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [OpenVegeMap](https://github.com/Rudloff/openvegemap) | [HappyCow 🔒🇺🇸](https://www.happycow.net/members/profile/niclasoverby) | | |
| ✅ | [Online Encyclopedia](https://en.wikipedia.org/wiki/Online_encyclopedia) | [Ibis 🦀](https://github.com/Nutomic/ibis) | [Wikipedia 🌐](https://en.wikipedia.org/wiki/User:Niclas_Overby) | | |

## Watch List

### Stack

### Zed
* [Helix Keymap](https://github.com/zed-industries/zed/issues/4642)
* [Direnv](https://github.com/zed-industries/zed/issues/4977)

### Helix
* [Nushell Helix Mode](https://github.com/nushell/reedline/issues/639)
* [VSCode Helix Keymap](https://github.com/71/dance/issues/299)

#### Matrix
* [Discord Forum Support](https://github.com/mautrix/discord/issues/101)

#### Nix
* [Flamegraph Profiler](https://github.com/NixOS/nix/pull/11373)
* [Multithreaded Evaluator](https://github.com/NixOS/nix/pull/10938)
* [Meta Categories](https://github.com/NixOS/rfcs/pull/146)
* [fromYAML Builtin](https://github.com/NixOS/nix/pull/7340)
* [Allow Derivations To Hardlink](https://github.com/NixOS/nix/issues/1272)
* [Pipe Operator](https://github.com/NixOS/rfcs/pull/148)
* [Inherit As List](https://github.com/NixOS/rfcs/pull/110)
* [Meson Port](https://github.com/NixOS/nix/issues/2503)

### Redox
* [The Road to Nix](https://gitlab.redox-os.org/redox-os/redox/-/issues/1552)

### Git
* [Mergiraf](https://codeberg.org/mergiraf/mergiraf)

### World

#### Mastodon
* [View Remote Followers](https://github.com/mastodon/mastodon/issues/20533)
* [View Old Posts](https://github.com/mastodon/mastodon/issues/17213)
* [Make Financial Supporters Visible](https://github.com/mastodon/mastodon/issues/5380)

### Legacy

#### ECMAScript
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
* [Signals](https://github.com/proposal-signals/proposal-signals)

#### JS/TS Toolchain
* [Ezno: Static JS Type Checker](https://github.com/kaleidawave/ezno)

#### React/JSX
* [JSX Props Pruning](https://github.com/facebook/jsx/issues/23)
* [React Native Promise](https://github.com/acdlite/rfcs/blob/first-class-promises/text/0000-first-class-support-for-promises.md)
