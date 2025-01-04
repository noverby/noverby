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

 ### Control
 * 🌐: Managed by [Nonprofit Org](https://en.wikipedia.org/wiki/Nonprofit_organization)
 * ⚖️: Managed by [Benefit Corp](https://en.wikipedia.org/wiki/Benefit_corporation)
 * 📖: [Open standard](https://en.wikipedia.org/wiki/Open_standard)
 * 🏡: Self-hosted
 * 🔒: [Proprietary](https://en.wiktionary.org/wiki/proprietary)

 ### Language
 * 🔥: Mojo
 * 🐍: Python
 * ❄️: Nix
 * 🦀: Rust
 * 🐹: Go
 * 💣: C/C++

### Migration Compatibility
 * 🆗: Not needed
 * ⬅️: Backward Compatible

### Hardware
| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Architecture | [X86-64 🔒](https://en.wikipedia.org/wiki/X86-64) | [RISC-V 📖](https://en.wikipedia.org/wiki/RISC-V), [ARM 🔒](https://en.wikipedia.org/wiki/ARM_architecture_family) | |
| 🚫 | Laptop | [Framework 13 🇺🇸](https://frame.work/products/laptop-diy-13-gen-intel) | [Tuxedo ARM Laptop 🇪🇺](https://www.tuxedocomputers.com/en/TUXEDO-on-ARM-is-coming.tuxedo) | [Dell XPS 13 Plus 9320 🇺🇸](https://www.dell.com/support/home/da-dk/product-support/product/xps-13-9320-laptop) |
| ✅ | Mobile | [Fairphone 4 🇪🇺](https://shop.fairphone.com/fairphone-4) | | [Google Pixel 7 Pro 🇺🇸](https://store.google.com/product/pixel_7_pro) |
| 🚫 | Watch | [Garmin Fenix 7 🔒🇺🇸](https://www.garmin.com/en-US/p/735520) | | [PineTime 🇭🇰](https://www.pine64.org/pinetime) |
| 🚫 | AR Glasses | [XReal Air 2 Pro 🔒🇨🇳](https://us.shop.xreal.com/products/xreal-air-2-pro) | | [XReal Light 🔒🇨🇳](https://www.xreal.com/light) |
| ✅ | Earphones | [Hyphen Aria 🇨🇭](https://rollingsquare.com/products/hyphen%C2%AE-aria) | | [Shokz Openfit 🇬🇧](https://shokz.com/products/openfit) |
| 🚫 | E-book Reader | [reMarkable 2 🔒🇳🇴](https://remarkable.com/store/remarkable-2) | [PineNote 🇭🇰](https://pine64.org/devices/pinenote) | [reMarkable 1 🔒🇳🇴](https://remarkable.com/store/remarkable) |

### Standards
| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Application Binary Interface | [System V ABI 📖](https://wiki.osdev.org/System_V_ABI) | [CrABI 📖](https://github.com/rust-lang/rust/pull/105586) | |
| 🚧 | IoT Connectivity Standard | | [Matter 📖](https://en.wikipedia.org/wiki/Matter_(standard)) | |
| 🚧 | Wireless Media | [ChromeCast 🔒](https://en.wikipedia.org/wiki/Chromecast) | [MatterCast 📖](https://en.wikipedia.org/wiki/Matter_(standard)) | [Miracast 📖](https://en.wikipedia.org/wiki/Miracast) |
| ✅ | USB Interface | [USB 3.2 📖](https://www.usb.org/usb-32-specification) | [USB4 📖](https://www.usb.org/usb4) | [Thunderbolt 3 🔒](https://www.intel.com/content/www/us/en/architecture-and-technology/thunderbolt/thunderbolt-technology-general.html) |
| ✅ | Display Interface | [DisplayPort 1.4 📖](https://en.wikipedia.org/wiki/DisplayPort) | [DisplayPort 2.1 📖](https://en.wikipedia.org/wiki/DisplayPort) | [HDMI 2.1 🔒](https://en.wikipedia.org/wiki/HDMI) |
| ✅ | Video Codec | [AV1 📖](https://aomedia.org/av1-features/get-started) | [AV1.1 📖](https://aomedia.org/av1-version-1-1-freezes-bitstream) | [H.264 🔒](https://en.wikipedia.org/wiki/Advanced_Video_Coding) |
| ✅ | Audio Codec | [Opus 📖](https://opus-codec.org) | | [AAC 🔒](https://en.wikipedia.org/wiki/Advanced_Audio_Coding) |
| 🚧 | Network Transport | [TCP 📖](https://en.wikipedia.org/wiki/Transmission_Control_Protocol) | [QUIC 📖](https://www.chromium.org/quic/) | |
| 🚧 | Web Protocol | [HTTP/2 📖](https://en.wikipedia.org/wiki/HTTP/2) | [HTTP/3 📖](https://en.wikipedia.org/wiki/HTTP/3) | [HTTP/1.1 📖](https://en.wikipedia.org/wiki/HTTP/1.1) |

### System Core
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Distro | [NixOS 🌐❄️](https://github.com/NixOS/nixpkgs) | | [OCI 📖](https://github.com/opencontainers/runtime-spec), [Distrobox](https://github.com/89luca89/distrobox) | [Fedora Silverblue](https://fedoraproject.org/silverblue) |
| ✅ | Kernel | [Linux 🌐💣](https://github.com/torvalds/linux) | [Asterinas 🦀](https://github.com/asterinas/asterinas), [Redox OS 🦀](https://gitlab.redox-os.org/redox-os/redox) | | |
| 🚫 | Init System | [Systemd 💣](https://github.com/systemd/systemd) | [Rustysd 🦀](https://github.com/KillingSpark/rustysd) | | |
| 🚧 | IPC | [Dbus 💣](https://gitlab.freedesktop.org/dbus/dbus) | [Busd 🦀](https://github.com/dbus2/busd) | | |
| 🚧 | Filesystem | [Ext4 📖💣](https://ext4.wiki.kernel.org/index.php/Main_Page) | [Btrfs 📖💣](https://btrfs.wiki.kernel.org/index.php/Main_Page) | | |
| 🚧 | Config Language | [Nix 🌐💣](https://github.com/NixOS/nix) | [Nickel 🦀](https://github.com/tweag/nickel) | [Organist](https://github.com/nickel-lang/organist) | |
| 🚧 | Package Manager | [Nix 🌐💣](https://github.com/NixOS/nix) | [Tvix 🦀](https://github.com/tvlfyi/tvix) | ⬅️ | |

### CLI Tools
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Shell | [Nushell 🦀](https://github.com/nushell/nushell) | | ❓ | [Bash 💣](https://git.savannah.gnu.org/cgit/bash.git) |
| ✅ | Core Utilities | [Nushell Builtins 🦀](https://github.com/nushell/nushell) | | [uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Directory Usage | [Dust 🦀](https://github.com/bootandy/dust) | | [uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Superuser | [Sudo-rs 🦀](https://github.com/memorysafety/sudo-rs) | | ⬅️ | [Sudo 💣](https://www.sudo.ws/repos/sudo) |
| ✅ | Fortune | [Fortune-kind 🦀](https://github.com/cafkafk/fortune-kind) | | ⬅️ | [Fortune-mod 💣](https://github.com/shlomif/fortune-mod) |
| ✅ | Find Files | [Fd 🦀](https://github.com/sharkdp/fd) | | 🆗 | [Findutils 💣](https://git.savannah.gnu.org/cgit/findutils.git) |
| ✅ | Find Patterns | [Ripgrep 🦀](https://github.com/BurntSushi/ripgrep) | | 🆗 | [Grep 💣](https://git.savannah.gnu.org/cgit/grep.git) |
| ✅ | Terminal Workspace | [Zellij 🦀](https://github.com/zellij-org/zellij) | | 🆗 | [Tmux 💣](https://github.com/tmux/tmux) |

### Dev Tools
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Compiler Framework | [Mlir 💣](https://github.com/llvm/llvm-project/tree/main/mlir), [LLVM 💣](https://github.com/llvm/llvm-project) | [Cranelift 🦀](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift) | ⬅️ | |
| ✅ | System Language | [Mojo 🔒🔥](https://github.com/modularml/mojo), [Rust 🦀](https://github.com/rust-lang/rust) | | [cxx 🦀](https://github.com/dtolnay/cxx), [bindgen 🦀](https://github.com/rust-lang/rust-bindgen) | [Go](https://github.com/golang/go) |
| ✅ | Scripting Language | [Mojo 🔒🔥](https://github.com/modularml/mojo) | | [RustPython 🦀](https://github.com/RustPython/RustPython), [WASI 📖](https://github.com/WebAssembly/WASI), [Interface Types 📖](https://github.com/WebAssembly/interface-types/tree/main/proposals/interface-types) | [TypeScript](https://github.com/microsoft/TypeScript) |
| 🚧 | Version Control | [Git 💣](https://github.com/git/git) | [Gitoxide 🦀](https://github.com/Byron/gitoxide) | ⬅️ ️️️️| |
| ✅ | Build Script| [Just 🦀](https://github.com/casey/just) | | ❓ | [GNU Make 💣](https://git.savannah.gnu.org/cgit/make.git) |
| ✅ | Editor | [Helix 🦀](https://github.com/helix-editor/helix) | | 🆗 | [Neovim 💣](https://github.com/neovim/neovim) |
| ✅ | IDE | [Zed 🦀](https://github.com/zed-industries/zed) | | [LSP](https://github.com/microsoft/language-server-protocol), [DAP](https://github.com/Microsoft/debug-adapter-protocol), [BSP](https://github.com/build-server-protocol/build-server-protocol) | [VS Codium](https://github.com/VSCodium/vscodium) |
| ✅ | System Call Tracing | [Lurk 🦀](https://github.com/JakWai01/lurk) | | 🆗 | [Strace 💣](https://github.com/strace/strace) |
| ✅ | Network Client | [Xh 🦀](https://github.com/ducaale/xh) | | ❓ | [Curl 💣](https://github.com/curl/curl) |
| 🚧 | Environment Loader | [Direnv 🐹](https://github.com/direnv/direnv) | [Envy 🦀](https://github.com/mre/envy) | ⬅️ | |

### Infrastructure
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | WebAssembly Runtime | [Wasmtime 🦀](https://github.com/bytecodealliance/wasmtime) | | | |
| 🚧 | JavaScript Runtime | [Node.js 💣](https://github.com/nodejs/node) | [Deno 🦀](https://github.com/denoland/deno) | [Node.js API](https://nodejs.org/api) |
| 🚫 | Container CLI | [Docker 🐹](https://github.com/docker/cli) | | [OCI 📖](https://github.com/opencontainers/runtime-spec) | |
| 🚧 | Container Runtime | [Runc 🐹](https://github.com/opencontainers/runc) | [Youki 🦀](https://github.com/containers/youki) | [OCI 📖](https://github.com/opencontainers/runtime-spec) | |
| 🚫 | Meta Database | [Hasura 🦀](https://github.com/hasura/graphql-engine) | [Surrealdb 🔒🦀](https://github.com/surrealdb/surrealdb) | [GraphQL](https://graphql.org) |
| 🚫 | Database | [Postgres 💣](https://github.com/postgres/postgres) | [Tikv 🦀](https://github.com/tikv/tikv) | ❓ | |
| 🚫 | Storage Engine | | [Sled 🦀](https://github.com/spacejam/sled) | ❓ | [RocksDB 💣](https://github.com/facebook/rocksdb) |

### Libraries
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Compression | [Zlib 💣](https://github.com/madler/zlib) | [Zlib-rs 🦀](https://github.com/memorysafety/zlib-rs) | ⬅️ | |
| 🚧 | TLS Protocol | [Openssl 💣](https://github.com/openssl/openssl) | [Rustls 🦀](https://github.com/rustls/rustls) | ⬅️ | |
| 🚧 | HTTP Protocol | [Nghttp2 💣](https://github.com/nghttp2/nghttp2), [Nghttp3 💣](https://github.com/ngtcp2/nghttp3) | [Hyper 🦀](https://github.com/hyperium/hyper) | ⬅️ | |
| 🚧 | HTTP Client | [Curl 💣](https://github.com/curl/curl) | [Reqwest 🦀](https://github.com/seanmonstar/reqwest) | ⬅️ | |
| 🚧 | Font Rendering | [HarfBuzz 💣](https://github.com/harfbuzz/harfbuzz), [FreeType 💣](https://github.com/freetype/freetype) | [Cosmic-text 🦀](https://github.com/pop-os/cosmic-text) | ⬅️ | |
| 🚧 | Browser Engine | [Gecko 🦀💣](https://en.wikipedia.org/wiki/Gecko_(software)) | [Servo 🦀](https://github.com/servo/servo) | ⬅️ | |

### GUI
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Design System | [Material You 🔒](https://m3.material.io) | | | [Material Design 2 🔒](https://m2.material.io) |
| 🚧 | GUI Toolkit | WIP Toolkit 🔥 | [Dixous 🦀](https://github.com/dioxusLabs/dioxus) | [Web Component 📖](https://kagi.com/search?q=Web+Components) | |
| ✅ | Desktop Environment | [Cosmic Epoch 🦀](https://github.com/pop-os/cosmic-epoch) | | | [Gnome Shell 💣](https://gitlab.gnome.org/GNOME/gnome-shell) |
| ✅ | File Manager | [Cosmic Files 🦀](https://github.com/pop-os/cosmic-files) | | | [GNOME Files 💣](https://gitlab.gnome.org/GNOME/nautilus) |
| 🚫 | Web Browser | [Unbraved Brave 💣🦀](https://github.com/MulesGaming/brave-debullshitinator) | [Verso 🦀](https://github.com/versotile-org/verso) | [Chrome Extension API 🔒](https://developer.chrome.com/docs/extensions/reference) | [Firefox 🦀💣](https://github.com/mozilla/gecko-dev) |
| 🚫 | GUI Package Manager | [Flatpak 💣](https://github.com/flatpak/flatpak) | | 🆗 | [Snap 🔒](https://github.com/canonical/snapd), [AppImage 💣](https://github.com/AppImage) |
| ✅ | App Browser | [Cosmic Store 🦀](https://github.com/pop-os/cosmic-store) | | 🆗 | [GNOME Software 💣](https://gitlab.gnome.org/GNOME/gnome-software) |

### Media
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Media Player | [Cosmic Player 🦀](https://github.com/pop-os/cosmic-player) | | [FFMPEG 💣](https://github.com/FFmpeg/FFmpeg), [GStreamer 💣](https://gitlab.freedesktop.org/gstreamer) | [Mpv 💣](https://github.com/mpv-player/mpv) |
| 🚫 | Image Editing | [GIMP 💣](https://gitlab.gnome.org/GNOME/gimp) | | | |
| 🚫 | Vector Graphics | [Inkscape 💣](https://gitlab.com/inkscape/inkscape) | | | |
| ✅ | Typesetting | [Typst 🦀](https://github.com/typst) | | 🆗 | [LaTeX 💣](https://github.com/latex3/latex3) |
| ✅ | Optimize PNG | [Oxipng 🦀](https://github.com/shssoichiro/oxipngc) | | 🆗 | [Optpng 💣](https://optipng.sourceforge.net) |

### Mobile
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | OS | [/e/OS 🌐🇪🇺](https://e.foundation/e-os) | | [MicroG 🌐](https://microg.org) | [GrapheneOS 🇨🇦](https://grapheneos.org) |
| ✅ | Launcher | [Olauncher](https://github.com/tanujnotes/Olauncher) | | | [Minimalist Phone 🔒](https://www.minimalistphone.com) |
| ✅ | Keyboard | [Thumb-Key](https://github.com/dessalines/thumb-key) | | | [OpenBoard](https://github.com/openboard-team/openboard) |
| ✅ | Alarm | [Chrono](https://github.com/vicolo-dev/chrono) | | | [Sleep 🔒](https://sleep.urbandroid.org) |
| 🚫 | Browser | [Fennec 💣🦀](https://f-droid.org/en/packages/org.mozilla.fennec_fdroid) | | | [Mull 💣🦀](https://github.com/mull-project/mull) |

### Services
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Password Manager | [Bitwarden.eu 🇪🇺](https://bitwarden.eu) | | | [Bitwarden.com 🇺🇸](https://bitwarden.com) |
| ✅ | Version Control | [Codeberg 🇪🇺](https://codeberg.org) | | | [GitHub 🔒🇺🇸](https://github.com), [GitLab 🔒🇺🇸](https://gitlab.com) |
| ✅ | Mail | [Tuta Mail 🇪🇺](https://tuta.com) | | | [Proton Mail 🌐🇨🇭](https://proton.me/mail) |
| ✅ | Calendar | [Tuta Calendar 🇪🇺](https://tuta.com) | | | [Proton Calendar 🌐🇨🇭](https://proton.me/calendar) |
| ✅ | Storage | [Syncthing 🐹🏡🇪🇺](https://github.com/syncthing/syncthing) | [Tuta Drive 🇪🇺](https://tuta.com/blog/pqdrive-project) | | [Proton Drive 🌐🇨🇭](https://proton.me/drive) |
| ✅ | VPN | [Adguard VPN 🇪🇺](https://adguard.com) | | | [Proton VPN 🌐🇨🇭](https://proton.me/vpn) |
| ✅ | DNS | [Adguard DNS 🇪🇺](https://adguard.com) | | | [NextDNS 🔒🇺🇸](https://nextdns.io) |
| ✅ | Search Engine | [StartPage 🔒🇪🇺](https://startpage.com) | [Stract 🇪🇺](https://github.com/StractOrg/stract) | [Search Shortcuts](https://support.mozilla.org/en-US/kb/assign-shortcuts-search-engines) | [Kagi 🔒🇺🇸](https://kagi.com), [DuckDuckGo 🔒🇺🇸](https://duckduckgo.com) |
| ✅ | LLM | [Ollama 🐹🏡🇪🇺](https://github.com/ollama/ollama) | | | [Claude 🔒🇺🇸](https://claude.ai), [OpenAI 🔒🇺🇸](https://openai.com) |

### Social
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Microblogging | [Mastodon 🌐🇪🇺](https://mas.to/niclasoverby), [Bluesky ⚖️🇺🇸](https://bsky.app/profile/overby.me) | | [ActivityPub 🌐📖](https://www.w3.org/TR/activitypub), [ATProtocol](https://atproto.com), [X-Cancel](https://xcancel.com) | [X-Twitter 🔒🇺🇸](https://x.com) |
| ✅ | Messaging | [Matrix 🌐](https://matrix.org), [Beeper 🔒🇺🇸](https://www.beeper.com) | | [Matrix Bridges](https://matrix.org/ecosystem/bridges) | [Telegram 🔒🇦🇪](https://telegram.org) |
| ✅ | Media Sharing | [Pixelfed 🇪🇺](https://pixelfed.social/niclasoverby) | | | [Instagram 🔒🇺🇸](https://instagram.com) |
| 🚫 | [Book Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Goodreads 🔒🇺🇸](https://www.goodreads.com/niclasoverby) | [Neodb 🐍](https://github.com/neodb-social/neodb), [Bookwyrm 🐍🇪🇺](https://bookwyrm.social/user/niclasoverby) | [OpenLibrary 🌐📖](https://openlibrary.org) | |
| 🚫 | [Film Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Letterboxd 🔒🇺🇸](https://letterboxd.com/niclasoverby) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [OpenLibrary 🌐📖](https://openlibrary.org) | |
| 🚫 | [Music Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Spotify 🔒🇺🇸](https://open.spotify.com/user/1148979230) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [OpenLibrary 🌐📖](https://openlibrary.org) | |
| 🚫 | [Fitness Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Strava 🔒🇺🇸](https://www.strava.com/athletes/116425039) | [FitTrackee 🐍](https://github.com/SamR1/FitTrackee) | | |
| 🚫 | [Food Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [HappyCow 🔒🇺🇸](https://www.happycow.net/members/profile/niclasoverby) | [OpenVegeMap](https://github.com/Rudloff/openvegemap) | | |
| ✅ | [Online Encyclopedia](https://en.wikipedia.org/wiki/Online_encyclopedia) | [Wikipedia 🌐](https://en.wikipedia.org/wiki/User:Niclas_Overby) | [Ibis 🦀](https://github.com/Nutomic/ibis) | | |

### Cloud
| Status | Component | Current | Research & Development | Migration Path | Legacy |
|:-:|-|-|-|-|-|
| 🚫 | Cloud Provider | [AWS 🇺🇸](https://aws.amazon.com) | | | |

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
