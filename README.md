## Homepage
[![Niclas Overby's Homepage](homepage.png)](https://niclas.overby.me/)

## Projects
* [My Nix Flakes](/noverby/nixflakes)
* [RadikalWiki](/RadikalWiki/radikalwiki)

## Github Stats
[![GitHub Streak](https://streak-stats.demolab.com/?user=noverby&theme=dark)](https://git.io/streak-stats)

## Stack
 * ✅: Good for now
 * 🚧: Transitioning
 * 🚫: Blocked
 * ❓: Undecided
 * 🆗: Not needed
 * ⬅️: Backward compatible

### Hardware
| Status | Component | Upcoming | Current | Legacy |
|:-:|-|-|-|-|
| ✅ | Laptop | | [Framwork 13](https://frame.work/products/laptop-diy-13-gen-intel) | [Dell XPS 13 Plus 9320](https://www.dell.com/support/home/da-dk/product-support/product/xps-13-9320-laptop) |
| ✅ | Mobile | | [Google Pixel 7 Pro](https://store.google.com/product/pixel_7_pro) | [Samsung Galaxy S23 Plus](https://www.samsung.com/dk/smartphones/galaxy-s23) |
| ✅ | Watch | | [Fēnix 7 – Sapphire Solar Edition](https://www.garmin.com/en-US/p/735520) | [PineTime](https://www.pine64.org/pinetime) |
| ✅ | AR Glasses | | [XReal Air 2 Pro](https://us.shop.xreal.com/products/xreal-air-2-pro) | [XReal Light](https://www.xreal.com/light/) |
| ✅ | Input | | [Tap XR](https://www.tapwithus.com/product/tap-xr) | [Tap Strap 2](https://www.tapwithus.com/product/tap-strap-2) |
| ✅ | Earphones | | [Shokz Openfit](https://shokz.com/products/openfit)| [Shokz Openrun Pro](https://shokz.com/products/openrunpro) |
| ✅ | E-book Reader | | [reMarkable 2](https://remarkable.com/store/remarkable-2) | [reMarkable 1](https://remarkable.com/store/remarkable) |

### Standards
| Status | Component | Upcoming | Current | Legacy |
|:-:|-|-|-|-|
| 🚧 | IoT Connectivity Standard | [Matter](https://en.wikipedia.org/wiki/Matter_(standard)) | | |
| 🚧 | Wireless Media | [Matter](https://en.wikipedia.org/wiki/Matter_(standard)) | [ChromeCast](https://en.wikipedia.org/wiki/Chromecast) | [Miracast](https://en.wikipedia.org/wiki/Miracast) |

### Base
| Status | Component | Upcoming | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Config Language | [Nickel](/tweag/nickel) | [Nix](/NixOS/nix) | [Organist](/nickel-lang/organist) |  |
| 🚧 | Package Manager | [Tvix](/tvlfyi/tvix) | [Nix](/NixOS/nix) | ⬅️ |  |
| 🚧 | Web Runtime | [Deno](/denoland/deno), [Bun](/oven-sh/bun) | [Node.js](/nodejs/node) | [Node.js API](https://nodejs.org/api) |
| ✅ | Distro | | [NixOS](/NixOS/nixpkgs) | [OCI](/opencontainers/runtime-spec), [Distrobox](/89luca89/distrobox) | [Fedora Silverblue](https://fedoraproject.org/silverblue/) |
| 🚫 | Container CLI | | | [OCI](/opencontainers/runtime-spec) | [Docker](/docker) |
| 🚧 | Container Runtime | | [Youki](/containers/youki) | [OCI](/opencontainers/runtime-spec) | [Runc](/opencontainers/runc) |
| ✅ | Typesetting |  | [Typst](/typst/) | ❓ | [LaTeX](/latex3/latex3) |

### Shell
| Status | Component | Upcoming | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Shell | | [Nushell](/nushell/nushell) | ❓ | [Bash](https://git.savannah.gnu.org/cgit/bash.git) |
| ✅ | Core Utilities | | [Nushell Builtins](/nushell/nushell) | [uutils](/uutils/coreutils) | [Coreutils](https://git.savannah.gnu.org/cgit/coreutils.git) |
| ✅ | Directory Usage | | [Dust](/bootandy/dust) | [uutils](/uutils/coreutils) | [Coreutils](https://git.savannah.gnu.org/cgit/coreutils.git) | 
| ✅ | Superuser | | [Sudo-rs](/memorysafety/sudo-rs) | ⬅️ | [Sudo](https://www.sudo.ws/repos/sudo) |
| ✅ | Fortune | | [Fortune-kind](/cafkafk/fortune-kind) | ⬅️ | [Fortune-mod](/shlomif/fortune-mod) |
| ✅ | Find Files | | [Fd](/sharkdp/fd) | 🆗 | [Findutils](https://git.savannah.gnu.org/cgit/findutils.git) |
| ✅ | Find Patterns | | [Ripgrep](/BurntSushi/ripgrep) | 🆗 | [Grep](https://git.savannah.gnu.org/cgit/grep.git) |
| ✅ | Terminal Workspace | | [Zellij](/zellij-org/zellij) | 🆗 | [Tmux](/tmux/tmux) |
| ✅ | Network Client | | [Xh](/ducaale/xh) | ❓ | [Curl](/curl/curl) |
| 🚫 | Environment Loader | [Envy](/mre/envy) | [Direnv](/direnv/direnv) | ❓ | |

### Dev
| Status | Component | Upcoming | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Compiler Framework | [Mlir](/llvm/llvm-project/tree/main/mlir/), [Cranelift](/bytecodealliance/wasmtime/tree/main/cranelift) | [LLVM](/llvm/llvm-project) | ⬅️ | |
| 🚧 | System Language | | [Mojo](/modularml/mojo), [Rust](/rust-lang/rust) | [cxx](/dtolnay/cxx), [bindgen](/rust-lang/rust-bindgen) |  [Clang](/llvm/llvm-project) |
| 🚧 | Scripting Language | [Mojo](/modularml/mojo) | [TypeScript](/microsoft/TypeScript) | [RustPython](/RustPython/RustPython), [WASI](/WebAssembly/WASI), [Interface Types](/WebAssembly/interface-types/tree/main/proposals/interface-types) | [Python](/python/cpython) | 
| 🚫 | Version Control | [Gitoxide](/Byron/gitoxide) | [Git](/git/git) | ⬅️ ️️️️| |
| ✅ | Build Script| | [Just](/casey/just) | ❓ | [GNU Make](https://git.savannah.gnu.org/cgit/make.git) |
| ✅ | Editor | | [Helix](/helix-editor/helix) | 🆗 | [Neovim](/neovim/neovim), [Vim](/vim/vim) |
| 🚧 | IDE | [Zed](/zed-industries/zed) | [VS Codium](/VSCodium/vscodium) | [LSP](/microsoft/language-server-protocol), [DAP](/Microsoft/debug-adapter-protocol), [BSP](/build-server-protocol/build-server-protocol) |
| ✅ | System Call Tracing | | [Lurk](/JakWai01/lurk) | 🆗 | [Strace](/strace/strace) |
| ✅ | Optimize PNG | | [Oxipng](/shssoichiro/oxipngc) | 🆗 | [Optpng](https://optipng.sourceforge.net) |
| 🚫 | Meta Database | [Surrealdb](/surrealdb/surrealdb) | [Hasura](/hasura/graphql-engine) | [GraphQL](https://graphql.org/) | 
| 🚫 | Database | [Tikv](/tikv/tikv) | [Postgres](/postgres/postgres) | ❓ |  |
| 🚫 | Storage Engine | [Sled](/spacejam/sled) | | ❓ | [RocksDB](/facebook/rocksdb) |

### Libraries
| Status | Component | Upcoming | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Compression | [Zlib-rs](/memorysafety/zlib-rs) | [Zlib](/madler/zlib) | ⬅️ | |
| 🚧 | TLS Protocol | [Rustls](/rustls/rustls) | [Openssl](/openssl/openssl) | ⬅️ | |
| 🚧 | HTTP Protocol | [Hyper](/hyperium/hyper) | [Nghttp2](/nghttp2/nghttp2), [Nghttp3](/ngtcp2/nghttp3) | ⬅️ | |
| 🚧 | HTTP Client | [Reqwest](/seanmonstar/reqwest) | [Curl](/curl/curl) | ⬅️ | |
| 🚧 | Font Rendering | [Cosmic-text](/pop-os/cosmic-text) | [HarfBuzz](/harfbuzz/harfbuzz), [FreeType](/freetype/freetype) | ⬅️ | |
| 🚧 | Browser Engine | [Servo](/servo/servo) | [Gecko](https://en.wikipedia.org/wiki/Gecko_(software)) | ⬅️ | |

### GUI
| Status | Component | Upcoming | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| 🚫 | Web Toolkit | | [React](/facebook/react) | [Web Component](https://kagi.com/search?q=Web+Components) | |
| ✅ | 2D Toolkit | | [Iced](/iced-rs/iced) | [Cosmic Gtk Theme](/pop-os/gtk-theme) | [GTK](https://gitlab.gnome.org/GNOME/gtk), [Qt](/qt/qt5) |
| 🚧 <br> /NixOS/nixpkgs/issues/259641 | Desktop Environment | [Cosmic Epoch](/pop-os/cosmic-epoch) | [Cosmic](/pop-os/cosmic) | [Gnome Shell](https://gitlab.gnome.org/GNOME/gnome-shell) |
| 🚧 | File Manager | [Cosmic Files](/pop-os/cosmic-files) | [GNOME Files](https://gitlab.gnome.org/GNOME/nautilus) | | |
| 🚧 | Web Browser | [Verso](/versotile-org/verso) | [Firefox](/mozilla/gecko-dev) | [Chrome Extension API](https://developer.chrome.com/docs/extensions/reference) |  |
| 🚧 | Media Player | [Cosmic Player](/pop-os/cosmic-player) | [Mpv](/mpv-player/mpv) | [FFMPEG](/FFmpeg/FFmpeg), [GStreamer](https://gitlab.freedesktop.org/gstreamer/) |  |
| ✅ | GUI Package Manager | | [Flatpak](/flatpak/flatpak) | 🆗 |  [Snap](/canonical/snapd), [AppImage](/AppImage) |

### Mobile
| Status | Component | Upcoming | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| ✅ | OS | [Murena](https://murena.com/) | [GrapheneOS](https://grapheneos.org) |  |
| ✅ | Keyboard | | [Thumb-Key](/dessalines/thumb-key) | | [OpenBoard](/openboard-team/openboard) |

### Services
| Status | Component | Upcoming | Current | Compat | Legacy |
|:-:|-|-|-|-|-|
| ✅ | DNS | | [NextDNS](https://nextdns.io) | | [Rethink DNS](https://rethinkdns.com) |
| ✅ | Search Engine | [Stract](/StractOrg/stract) | [Kagi](https://kagi.com) | [Search Shortcuts](https://support.mozilla.org/en-US/kb/assign-shortcuts-search-engines) | [StartPage](https://startpage.com), [DuckDuckGo](https://duckduckgo.com) |
| ✅ | LLM | | [Claude](https://claude.ai) | | [OpenAI o1](https://openai.com/o1) |
| ✅ | Microblogging | | [Mastodon](https://mas.to/niclasoverby), [Bluesky](https://bsky.app/profile/overby.me) | ❓ | [Twitter](https://twitter.com) |
| ✅ | Messaging | | [Beeper](https://www.beeper.com), [Matrix](https://matrix.org) | [Matrix Bridges](https://matrix.org/ecosystem/bridges) | [Telegram](https://telegram.org) |
| ✅ | Media Sharing |  | [Pixelfed](https://pixelfed.social/niclasoverby) | | [Instagram](https://instagram.com) |
| 🚫 | [Book Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Bookwyrm](https://bookwyrm.social/user/niclasoverby) | [Goodreads](https://www.goodreads.com/niclasoverby) | [OpenLibrary](https://openlibrary.org) | |
| 🚫 | [Film Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | ❓ | [Letterboxd](https://letterboxd.com/niclasoverby) | [OpenLibrary](https://openlibrary.org) | |
| 🚫 | [Music Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | ❓ | [Spotify](https://open.spotify.com/user/1148979230) | [OpenLibrary](https://openlibrary.org) | |
| 🚫 | [Fitness Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | ❓ | [Strava](https://www.strava.com/athletes/116425039) | | |
| ✅ | [Food Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | | [HappyCow](https://www.happycow.net/members/profile/niclasoverby) | | |
| ✅ | [Online Encyclopedia](https://en.wikipedia.org/wiki/Online_encyclopedia) | | [Wikipedia](https://en.wikipedia.org/wiki/User:Niclas_Overby) | | |

## Watch List

### Stack

### Zed
* [Helix Keymap](/zed-industries/zed/issues/4642)
* [Direnv](/zed-industries/zed/issues/4977)

### Helix
* [Nushell Helix Mode](/nushell/reedline/issues/639)
* [VSCode Helix Keymap](/71/dance/issues/299)

#### Zig
* [Divorce from LLVM](/ziglang/zig/issues/16270)
* [Comptime Interfaces](/ziglang/zig/issues/1268)

#### Matrix
* [Discord Forum Support](/mautrix/discord/issues/101)

#### Nix
* [Flamegraph Profiler](/NixOS/nix/pull/11373)
* [Multithreaded Evaluator](/NixOS/nix/pull/10938)
* [Meta Categories](/NixOS/rfcs/pull/146)
* [fromYAML Builtin](/NixOS/nix/pull/7340)
* [Allow Derivations To Hardlink](/NixOS/nix/issues/1272)
* [Pipe Operator](/NixOS/rfcs/pull/148)
* [Inherit As List](/NixOS/rfcs/pull/110)
* [Meson Port](/NixOS/nix/issues/2503)

### Redox
* [The Road to Nix](https://gitlab.redox-os.org/redox-os/redox/-/issues/1552)

### World

#### Mastodon
* [View Remote Followers](/mastodon/mastodon/issues/20533)
* [View Old Posts](/mastodon/mastodon/issues/17213)
* [Make Financial Supporters Visible](/mastodon/mastodon/issues/5380)

### Legacy

#### Bun
* [Implement Node-API](/oven-sh/bun/issues/158)

#### ECMAScript 
* [Pattern Matching](/tc39/proposal-pattern-matching):
  * [Extractors](/tc39/proposal-extractors)
* [Pipeline Operator](/tc39/proposal-pipeline-operator):
  * [Call This](/tc39/proposal-call-this)
* [Type Annotations](/tc39/proposal-type-annotations)
* [Record & Tuple](/tc39/proposal-record-tuple)
* [ADT Enum](/Jack-Works/proposal-enum)
* [Do Expressions](/tc39/proposal-do-expressions)
* [Operator Overloading](/tc39/proposal-operator-overloading)
* [Array Grouping](/tc39/proposal-array-grouping)
* [Signals](/proposal-signals/proposal-signals)

#### JS/TS Toolchain
* [Ezno: Static JS Type Checker](/kaleidawave/ezno)

#### React/JSX
* [JSX Props Pruning](/facebook/jsx/issues/23)
* [React Native Promise](/acdlite/rfcs/blob/first-class-promises/text/0000-first-class-support-for-promises.md)

