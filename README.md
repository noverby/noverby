# [@overby.me](https://tangled.org/overby.me/overby.me)

<a id="toc"></a>
<!-- BEGIN mktoc {"min_depth": 2, "max_depth": 3} -->

- [📁 Repository](#-repository)
  - [🚀 Projects](#-projects)
  - [📦 Packages](#-packages)
  - [📋 Configurations](#-configurations)
  - [🧩 Modules](#-modules)
- [💻 Hardware](#-hardware)
- [🌐 Services](#-services)
  - [🔒 Personal](#-personal)
  - [👥 Social Platforms](#-social-platforms)
  - [☁️ Cloud](#-cloud)
- [📏 Standards](#-standards)
  - [🔌 Hardware](#-hardware)
  - [🔗 Software](#-software)
  - [📝 Data](#-data)
  - [📡 Network](#-network)
- [🖥️ System](#-system)
  - [⚙️ Core](#-core)
  - [📚 Libraries](#-libraries)
  - [🏗️ Infrastructure](#-infrastructure)
- [🛠️ Development](#-development)
  - [🔧 Tooling](#-tooling)
  - [❄️ Configuration](#-configuration)
  - [🦀 Systems](#-systems)
  - [🌐 Web](#-web)
  - [🐍 Scripting](#-scripting)
- [📱 Applications](#-applications)
  - [💻 Command Line](#-command-line)
  - [🖥️ Desktop Environment](#-desktop-environment)
  - [🚀 Productivity](#-productivity)
  - [🎨 Media](#-media)
  - [🌐 Browser Extensions](#-browser-extensions)
  - [📱 Mobile](#-mobile)
<!-- END mktoc -->

| Category | Specifiers |
|----------|----------------------|
| **State** | ✅ Good for now, 🚧 WIP, 🚫 Blocked, ❓ Undecided |
| **Control** | 🌐 [Nonprofit](https://en.wikipedia.org/wiki/Nonprofit_organization), 🏛️ [Public Authority](https://en.wikipedia.org/wiki/Public_administration), ⚖️ [Benefit Corp](https://en.wikipedia.org/wiki/Benefit_corporation), 📖 [Open Standard](https://en.wikipedia.org/wiki/Open_standard), 🏡 Self-hosted, 🔒 [Proprietary](https://en.wiktionary.org/wiki/proprietary), ⏳ [BSL](https://en.wikipedia.org/wiki/Business_Source_License), 🇲🇮: Country of origin (Only for Public Authority/Proprietary) |
| **Compatibility** | 🆗 Not needed, ⬅️ Backward compatible |
| **Language** | 🔥 [Mojo](https://en.wikipedia.org/wiki/Mojo_(programming_language)), 🐍 Python, ❄️ Nix/[Nickel](https://github.com/tweag/nickel), 🦀 Rust, 🦪 Nushell, 🐹 Go, 💣 C/C++, 🐒 [ECMAScript](https://en.wikipedia.org/wiki/ECMAScript), 🐷 Java/Kotlin, 🌙 Lua, λ Haskell |
| **Miscellaneous** | 🇽➡️🇾 Migrating from 🇽 to 🇾, 🌀 [Atmosphere (AT Protocol)](https://atproto.com) , 👁️ [Sentientist](https://en.wikipedia.org/wiki/Sentientism) |

## 📁 Repository

[⬆](#toc)

### 🚀 Projects

[⬆](#toc)

<details open>

| Project | Description |
|-|-|
| [Homepage 🐒➡️🦀](https://tangled.org/@overby.me/overby.me/tree/main/homepage) | Personal website and portfolio |
| [Wiki 🐒➡️🦀🌀](https://tangled.org/@overby.me/overby.me/tree/main/wiki) | Platform for political conference & beyond |
| [Wasm-mojo 🔥](https://tangled.org/@overby.me/overby.me/tree/main/wasm-mojo) | WebAssembly interop layer for Mojo |
| [Wasmtime-mojo 🔥](https://tangled.org/@overby.me/overby.me/tree/main/wasmtime-mojo) | Mojo FFI bindings for the Wasmtime WebAssembly runtime |
| [Zed-mojo 🔥](https://tangled.org/@overby.me/overby.me/tree/main/zed-mojo) | Mojo language extension for Zed |
| [NixOS-rs 🦀❄️](https://tangled.org/@overby.me/overby.me/tree/main/nixos-rs) | NixOS with Rust user space |
| [Systemd-rs 🦀](https://tangled.org/@overby.me/overby.me/tree/main/systemd-rs) | A Systemd replacement in Rust |
| [Pkg-config-rs 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkg-config-rs) | A pkg-config implementation in Rust |

</details>

### 📦 Packages

[⬆](#toc)

<details open>

#### Jupyter

| Package | Description |
|-|-|
| [Deno-jupyter-kernel 🐒](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/deno-jupyter-kernel.nix) | Jupyter Notebook kernel for Deno |
| [Mojo-jupyter-kernel 🔥](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/mojo-jupyter-kernel.nix) | Jupyter Notebook kernel for Mojo |
| [Nu-jupyter-kernel 🦪](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/nu-jupyter-kernel.nix) | Jupyter Notebook kernel for Nushell |
| [Rust-jupyter-kernel 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/Rust-jupyter-kernel.nix) | Jupyter Notebook kernel for Rust |
| [Sidecar 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/sidecar/default.nix) | Jupyter Notebook viewer |
| [Xeus-lix ❄️](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/xeus-lix/default.nix) | Jupyter Notebook kernel for Nix |

#### Development

| Package | Description |
|-|-|
| [Mojo 🔥](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/mojo.nix) | Mojo development toolchain |
| [Envy 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/envy.nix) | Environment loader CLI tool |
| [Nix-sweep 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/nix-sweep.nix) | Nix garbage collector |
| [Rcgen 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/rcgen.nix) | X.509 certificate generation CLI tool |
| [Starship-jj 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/starship-jj.nix) | Starship plugin for jj |

#### Media

| Package | Description |
|-|-|
| [Cavif-rs 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/cavif-rs/default.nix) | AVIF image encoder CLI tool |
| [Layout 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/layout/default.nix) | Graphviz dot renderer |
| [Vibe 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/vibe.nix) | A desktop audio visualizer |
| [Wondermagick 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/wondermagick/default.nix) | Memory-safe replacement for Imagemagick |

#### System

| Package | Description |
|-|-|
| [Busd 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/busd.nix) | A D-Bus bus implementation in Rust |
| [Cpx 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/cpx.nix) | Cp reimagined |
| [Lacy 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/lacy.nix) | Fast magical cd alternative |
| [Sunsetc 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/sunsetc.nix) | SSH in Rust |

#### Security

| Package | Description |
|-|-|
| [Age-plugin-fido2prf 🐹](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/age-plugin-fido2prf.nix) | FIDO2 PRF plugin for age |
| [Forkfs 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/forkfs.nix) | Sandbox a process's changes to file system |
| [Hakoniwa 🦀](https://tangled.org/@overby.me/overby.me/tree/main/pkgs/hakoniwa.nix) | Process isolation CLI tool |

</details>

### 📋 Configurations

[⬆](#toc)

<details open>

| Configuration | Description |
|-|-|
| [Home Manager ❄️](https://tangled.org/@overby.me/overby.me/tree/main/config/home-manager) | Home Manager configurations |
| [NixOS ❄️](https://tangled.org/@overby.me/overby.me/tree/main/config/nixos) | NixOS configurations |
| [Devenv ❄️](https://tangled.org/@overby.me/overby.me/tree/main/config/devenv) | Devenv configurations |

</details>

### 🧩 Modules

[⬆](#toc)

<details open>

| Module | Description |
|-|-|
| [Flakelight ❄️](https://tangled.org/@overby.me/overby.me/tree/main/modules/flakelight) | Flakelight modules |
| [Home Manager ❄️](https://tangled.org/@overby.me/overby.me/tree/main/modules/home-manager) | Home Manager modules |
| [NixOS ❄️](https://tangled.org/@overby.me/overby.me/tree/main/modules/nixos) | NixOS modules |
| [Devenv ❄️](https://tangled.org/@overby.me/overby.me/tree/main/modules/devenv) | Devenv modules |

</details>

## 💻 Hardware

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚫 | CPU | [AMD 🇺🇸](https://en.wikipedia.org/wiki/AMD) | | [Intel 🇺🇸](https://en.wikipedia.org/wiki/Intel) |
| 🚫 | GPU | [AMD 🇺🇸](https://en.wikipedia.org/wiki/AMD) | [Vortex 📖](https://github.com/vortexgpgpu/vortex) | [Intel 🇺🇸](https://en.wikipedia.org/wiki/Intel), [NVIDIA 🇺🇸](https://en.wikipedia.org/wiki/NVIDIA) |
| 🚫 | Laptop | [Thinkpad T14 Ryzen 7 Pro Gen 6](https://www.lenovo.com/dk/da/p/laptops/thinkpad/thinkpadt/lenovo-thinkpad-t14s-gen-6-14-inch-amd-laptop/len101t0109) | [Tuxedo ARM Laptop 🇪🇺](https://www.tuxedocomputers.com/en/TUXEDO-on-ARM-is-coming.tuxedo), [StarLabs Systems 🇬🇧](https://starlabs.systems) | [Framework 13 🇺🇸](https://frame.work/products/laptop-diy-13-gen-intel), [Dell XPS 13 Plus 9320 🇺🇸](https://www.dell.com/support/home/da-dk/product-support/product/xps-13-9320-laptop) |
| ✅ | Security Key | [Nitrokey 📖🇪🇺](https://www.nitrokey.com) | | [YubiKey 🔒🇺🇸](https://www.yubico.com) |
| ✅ | Mobile | [Fairphone 4 🇪🇺](https://en.wikipedia.org/wiki/Fairphone_4) | | [Google Pixel 7 Pro 🇺🇸](https://store.google.com/product/pixel_7_pro) |
| ✅ | Router | [Turris Omnia 🇪🇺](https://www.turris.com/en/products/omnia) | | |
| 🚫 | Watch | [Garmin Fenix 7 🔒🇺🇸](https://foundation.mozilla.org/en/privacynotincluded/garmin-fenix) | [Polar 🇬🇧](https://www.polar.com) | [PineTime 🇭🇰](https://www.pine64.org/pinetime) |
| 🚫 | AR Glasses | [XReal Air 2 Pro 🔒🇨🇳](https://next.xreal.com/air2) | | [XReal Light 🔒🇨🇳](https://www.xreal.com/light) |
| ✅ | Earphones | [Shokz Openfit 2 🇬🇧](https://shokz.com/products/openfit2) | | [Shokz Openfit 🇬🇧](https://shokz.com/products/openfit) |
| ✅ | E-book Reader | [reMarkable 2 🔒🇳🇴](https://remarkable.com/store/remarkable-2) | [PineNote 🇭🇰](https://pine64.org/devices/pinenote) | [reMarkable 1 🔒🇳🇴](https://remarkable.com/store/remarkable) |

</details>

## 🌐 Services

[⬆](#toc)

### 🔒 Personal

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Password Manager | [Bitwarden.eu 🇪🇺](https://bitwarden.eu) | | 🆗 | [Bitwarden.com 🇺🇸](https://bitwarden.com) |
| ✅ | Mail | [Tuta Mail 🇪🇺](https://tuta.com) | | [Mail Import](https://tuta.com/blog/tuta-release-update-february) | [Proton Mail 🌐🇨🇭➡️🇪🇺](https://proton.me/mail) |
| ✅ | Calendar | [Tuta Calendar 🇪🇺](https://tuta.com) | | [iCalendar 📖](https://en.wikipedia.org/wiki/ICalendar) | [Proton Calendar 🔒🌐🇨🇭➡️🇪🇺](https://proton.me/calendar) |
| ✅ | Storage | [Syncthing 🐹🏡🇪🇺](https://github.com/syncthing/syncthing) | [Tuta Drive 🇪🇺](https://tuta.com/blog/pqdrive-project) | 🆗 | [Proton Drive 🌐🇨🇭➡️🇪🇺](https://proton.me/drive) |
| ✅ | Virtual Private Network | [Adguard VPN 🇪🇺](https://adguard.com) | | 🆗 | [Proton VPN 🌐🇨🇭➡️🇪🇺](https://proton.me/vpn) |
| ✅ | Domain Name System | [Adguard DNS 🇪🇺](https://adguard.com) | | 🆗 | [NextDNS 🔒🇺🇸](https://nextdns.io) |
| ✅ | Search Engine | [Qwant 🇪🇺](https://www.qwant.com) | [Stract 🦀🇪🇺](https://github.com/StractOrg/stract) | [Search Shortcuts](https://support.mozilla.org/en-US/kb/assign-shortcuts-search-engines), [EU Search Perspective 🇪🇺](https://eu-searchperspective.com) | [StartPage 🔒🇪🇺](https://startpage.com) |
| ✅ | Large Language Model | [Devstral 2 🇪🇺](https://mistral.ai/news/devstral-2-vibe-cli) | [EuroLLM 🇪🇺](https://eurollm.io) | | |
| ✅ | Large Language Model Provider | [Ollama 🐹🏡🇪🇺](https://github.com/ollama/ollama), [Mistral 🇪🇺](https://mistral.ai) | [Lumo 🇪🇺](https://lumo.proton.me) | 🆗 | [OpenAI 🔒🇺🇸](https://openai.com) |
| ✅ | Version Control | [Tangled 🇪🇺🌀](https://tangled.sh/@overby.me), [Codeberg 🇪🇺](https://codeberg.org/noverby) | | [Mirror](https://codeberg.org/Recommendations/Mirror_to_Codeberg) | [Microsoft GitHub 🔒🇺🇸](https://github.com/noverby), [GitLab 🇺🇸](https://gitlab.com/noverby) |
| 🚧 | Music | [Spotify 🇪🇺](https://spotify.com) | [Qobuz 🇪🇺](https://www.qobuz.com) | 🆗 | [Deezer 🇪🇺](https://deezer.com) |
| ✅ | Audiobooks | [LibreVox 🌐](https://librivox.org/) | | 🆗 | [Amazon Audible 🇺🇸](https://www.audible.com) |
| 🚧 | Payment | [MobilePay 🇪🇺](https://mobilepay.dk), [PayPal 🇺🇸](https://paypal.com) | [Wero 🇪🇺](https://wero-wallet.eu) | 🆗 | |
| 🚧 | Payment Medium | [Dankort 🔒🇪🇺](https://www.dankort.dk), [Visa 🔒🇺🇸](https://www.visa.com) | [Digital Euro 🏛️🇪🇺](https://www.ecb.europa.eu/paym/digital_euro/html/index.en.html), [GNU Taler 📖](https://taler.net) | | |
| 🚧 | Donation | [Ko-fi 🇬🇧](https://ko-fi.com) | [Liberapay 🌐🇪🇺](https://liberapay.com) | [GNU Taler 📖🇪🇺](https://taler.net) | [Patreon 🔒🇺🇸](https://patreon.com) |
| ✅ | Translation | [DeepL 🔒🇪🇺](https://www.deepl.com) | | 🆗 | [Google Translate 🔒🇺🇸](https://translate.google.com) |

</details>

### 👥 Social Platforms

[⬆](#toc)

<details open>

#### Communication Platforms

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Messaging | [Etke.cc Matrix 🇪🇺](https://etke.cc) | | [Matrix 🌐](https://matrix.org), [Matrix Bridges](https://matrix.org/ecosystem/bridges) | [Telegram 🔒🇦🇪](https://telegram.org), [Automattic Beeper 🔒🇺🇸](https://www.beeper.com), [Meta Messenger 🔒🇺🇸](https://messenger.com), [Meta WhatsApp 🔒🇺🇸](https://whatsapp.com) |
| ✅ | Event Hosting | [Smoke Signal Events 🌐🌀](https://smokesignal.events), [Meetup 🔒🇪🇺](https://meetup.com) | | [AT Protocol 🌀](https://atproto.com) | [Meta Facebook Events 🔒🇺🇸](https://facebook.com) |

#### Media Platforms

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Media Sharing | [Pixelfed 🇪🇺](https://pixelfed.social/niclasoverby) | [Flashes 🔒🇪🇺🌀](https://github.com/birdsongapps/Flashes) | [AT Protocol 🌀](https://atproto.com) | [Meta Instagram (Flufi) 🔒🇺🇸](https://flufi.me) |
| 🚧 | Long-form Video | [Alphabet YouTube 🔒🇺🇸](https://youtube.com) | [Skytube 🔒🇺🇸🌀](https://skytube.video) | [AT Protocol 🌀](https://atproto.com) | |
| 🚧 | Short-form Video | | [SkyLight 🔒🇺🇸🌀](https://skylight.social), [Spark 🔒🇺🇸🌀](https://sprk.so) | | [ByteDance TikTok (Tikviewr) 🔒🇨🇳](https://tikviewr.com) |
| 🚧 | Live Streaming | [Stream.place 🌀](https://stream.place/) | | [AT Protocol 🌀](https://atproto.com) | [Twitch 🔒🇺🇸](https://twitch.tv) |

#### Discussion Platforms

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Discussion Forums | [Lemmy World 🌐🇪🇺](https://lemmy.world) | [Frontpage 🔒🇬🇧🌀](https://frontpage.fyi) | [AT Protocol 🌀](https://atproto.com) | [Reddit (Libred) 🔒🇺🇸](https://github.com/redlib-org/redlib), [Lemmy.ml 🌐🇪🇺](https://lemmy.world) |
| 🚧 | Microblogging | [Bluesky ⚖️🇺🇸🌀](https://bsky.app/profile/overby.me) | [Eurosky 🌐🇪🇺🌀](https://eurosky.social) | [AT Protocol 🌀](https://atproto.com) | [Mastodon 🌐🇪🇺](https://mas.to/@niclasoverby), [X (X-cancel) 🔒🇺🇸](https://xcancel.com) |
| ✅ | Macroblogging | [Leaflet 🌀](https://leaflet.pub) | | [AT Protocol 🌀](https://atproto.com) | |

#### Content Cataloging

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | [Book Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [Popfeed 🌀](https://popfeed.social/profile/overby.me) | | [Amazon Goodreads 🔒🇺🇸](https://www.goodreads.com/niclasoverby) |
| ✅ | [Film Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Neodb 🐍](https://github.com/neodb-social/neodb) | [Popfeed 🌀](https://popfeed.social/profile/overby.me) | | [Letterboxd 🔒🇳🇿](https://letterboxd.com/niclasoverby), [Amazon IMDB 🔒🇺🇸](https://www.imdb.com) |
| ✅ | [Music Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Rocksky 🇲🇬🌀](https://rocksky.app/profile/overby.me) | [Popfeed 🌀](https://popfeed.social/profile/overby.me) | | [Spotify 🔒🇪🇺](https://open.spotify.com/user/1148979230) |
| 🚫 | [Fitness Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [Garmin Connect 🔒🇺🇸](https://connect.garmin.com) | [FitTrackee 🐍](https://github.com/SamR1/FitTrackee) | [GPX 📖](https://en.wikipedia.org/wiki/GPS_Exchange_Format) | [Strava 🔒🇺🇸](https://www.strava.com/athletes/116425039) |
| ✅ | [Food Cataloging](https://en.wikipedia.org/wiki/Social_cataloging_application) | [HappyCow 👁️🔒🇺🇸](https://www.happycow.net/members/profile/niclasoverby) | [OpenVegeMap](https://github.com/Rudloff/openvegemap) | 🆗 | |

#### Collaboration & Knowledge

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Collaboration Tools | [AppFlowy 🦀](https://github.com/AppFlowy-IO/AppFlowy) | | [Import](https://docs.appflowy.io/docs/guides/import-from-notion) | [Notion 🔒🇺🇸](https://notion.so) |
| ✅ | [Online Encyclopedia](https://en.wikipedia.org/wiki/Online_encyclopedia) | [Wikipedia 🌐](https://en.wikipedia.org/wiki/User:Niclas_Overby) | [Ibis 🦀](https://github.com/Nutomic/ibis) | 🆗 | |

#### Social & Dating

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Dating | [Veggly 🔒👁️🇧🇷](https://veggly.net) | | 🆗 | [Tinder 🔒🇺🇸](https://en.wikipedia.org/wiki/Tinder_(app)) |

</details>

### ☁️ Cloud

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Cloud Provider | [Amazon AWS 🇺🇸](https://aws.amazon.com) | [Scaleway 🇪🇺](https://www.scaleway.com), [UpCloud 🇪🇺](https://www.upcloud.com) | | |
| ✅ | Bare Metal Hosting | [Hetzner 🇪🇺](https://hetzner.com) | | | |
| ✅ | Static Host | [Statichost 🇪🇺](https://statichost.eu) | [FastFront 🇪🇺](https://www.fastfront.io) | | [Vercel 🇺🇸](https://vercel.com) |
| ✅ | Domain Registrar | [Simply 🇪🇺](https://simply.com) | | | |
| 🚧 | Backend | [Nhost 🇪🇺](https://nhost.io) | WIP Backend 🔥 | | |
| ✅ | Logging | [Bugfender 🇪🇺](https://bugfender.com) | | | [Sentry 🇺🇸](https://sentry.io) |
| ✅ | Analytics | [Counter.dev 🇪🇺](https://counter.dev) | | | [Vercel Analytics 🇺🇸](https://vercel.com/analytics) |
| ✅ | Content Delivery Network | [Bunny.net 🇪🇺](https://bunny.net) | | | |

</details>

## 📏 Standards

[⬆](#toc)

### 🔌 Hardware

[⬆](#toc)

<details open>

#### Architecture & Firmware

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Architecture | [X86-64 🔒](https://en.wikipedia.org/wiki/X86-64) | [RISC-V 📖](https://en.wikipedia.org/wiki/RISC-V), [ARM 🔒](https://en.wikipedia.org/wiki/ARM_architecture_family) | |
| 🚧 | Firmware | [Thinkpad UEFI 🔒](https://en.wikipedia.org/wiki/UEFI) | [Coreboot 💣](https://coreboot.org), [Oreboot 🦀](https://github.com/oreboot/oreboot) | |

#### Connectivity & Interfaces

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| ✅ | Internet of Things Connectivity | [Matter 📖](https://en.wikipedia.org/wiki/Matter_(standard)), [Zigbee 📖](https://en.wikipedia.org/wiki/Zigbee) | | |
| 🚧 | Wireless Media | [Google ChromeCast 🔒](https://en.wikipedia.org/wiki/Chromecast) | [MatterCast 📖](https://en.wikipedia.org/wiki/Matter_(standard)) | [Miracast 📖](https://en.wikipedia.org/wiki/Miracast) |
| ✅ | Peripheral Interface | [USB4 📖](https://www.usb.org/usb4) | | [Thunderbolt 3 🔒](https://en.wikipedia.org/wiki/Thunderbolt_(interface)) |
| ✅ | Display Interface | [DisplayPort 📖](https://en.wikipedia.org/wiki/DisplayPort) | | [HDMI 2.1 🔒](https://en.wikipedia.org/wiki/HDMI) |

#### Navigation & Positioning

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Satellite Navigation | [Galileo 🇪🇺](https://www.euspa.europa.eu/eu-space-programme/galileo), [GPS 🏛️🇺🇸](https://www.gps.gov) | | |
| 🚧 | Satellite Internet | | [Iris² 🏛️🇪🇺](https://defence-industry-space.ec.europa.eu/eu-space-policy/iris2_en) | [Starlink 🔒🇺🇸](https://www.starlink.com) |

</details>

### 🔗 Software

[⬆](#toc)

<details open>

#### System & Compute Interfaces

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Application Binary Interface | [System V ABI 📖](https://en.wikipedia.org/wiki/X86_calling_conventions#Variations) | [CrABI 📖](https://github.com/rust-lang/rust/pull/105586) | |
| ✅ | GPU Compute | [Vulkan Compute 📖](https://www.vulkan.org) | | [OpenCL 📖](https://www.khronos.org/opencl) |
| ✅ | Graphics API | [Vulkan 📖](https://www.vulkan.org) | | [OpenGL 📖](https://www.opengl.org) |
| ✅ | Windowing | [Wayland 📖](https://wayland.freedesktop.org) | | [X11 📖](https://www.x.org) |
| ✅ | Heterogeneous Compute | [SYCL 📖](https://www.khronos.org/sycl) | | |
| 🚫 | Tensor Operations | | | |
| 🚫 | AI Inference | | | |

</details>

### 📝 Data

[⬆](#toc)

<details open>

#### Text & Object Notation

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| ✅ | Text | [UTF-8 📖](https://en.wikipedia.org/wiki/UTF-8) | | [UTF-16 📖](https://en.wikipedia.org/wiki/UTF-16) |
| ✅ | Object Notation | [JSON 📖](https://www.json.org) | [KDL](https://kdl.dev), [EON](https://github.com/emilk/eon) | |
| ✅ | Binary Object Notation | [CBOR 📖](https://cbor.io) | | |

#### Media Codecs

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| ✅ | Image Codec (Lossy) | [AVIF 📖](https://en.wikipedia.org/wiki/AVIF) | | [JPEG 📖](https://en.wikipedia.org/wiki/JPEG) |
| ✅ | Image Codec (Lossless) | [PNG 📖](https://en.wikipedia.org/wiki/Portable_Network_Graphics) | [AVIF (lossless) 📖](https://en.wikipedia.org/wiki/AVIF) | |
| ✅ | Audio Codec | [Opus 📖](https://opus-codec.org) | | [AAC 🔒](https://en.wikipedia.org/wiki/Advanced_Audio_Coding) |
| ✅ | Video Codec | [AV1 📖](https://aomedia.org/av1-features/get-started) | | [H.264 🔒](https://en.wikipedia.org/wiki/Advanced_Video_Coding) |

</details>

### 📡 Network

[⬆](#toc)

<details open>

#### Network & Web Protocols

| Status | Component | Current | Research & Development | Legacy |
|:-:|-|-|-|-|
| 🚧 | Network Transport | [TCP 📖](https://en.wikipedia.org/wiki/Transmission_Control_Protocol) | [QUIC 📖](https://www.chromium.org/quic/) | |
| 🚧 | Web Protocol | [HTTP/2 📖](https://en.wikipedia.org/wiki/HTTP/2) | [HTTP/3 📖](https://en.wikipedia.org/wiki/HTTP/3) | [HTTP/1.1 📖](https://en.wikipedia.org/wiki/HTTP/1.1) |
| 🚧 | Wireless Network | [Wi‑Fi 6 (IEEE 802.11ax) 📖](https://en.wikipedia.org/wiki/IEEE_802.11ax) | [Wi‑Fi 7 (IEEE 802.11be) 📖](https://en.wikipedia.org/wiki/IEEE_802.11be) | [Wi‑Fi 5 (IEEE 802.11ac) 📖](https://en.wikipedia.org/wiki/IEEE_802.11ac) |
| ✅ | Social Media | [AT Protocol 🌀📖](https://atproto.com) | | [ActivityPub 📖](https://www.w3.org/TR/activitypub/) |

</details>

## 🖥️ System

[⬆](#toc)

### ⚙️ Core

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Distro | [NixOS 🌐❄️](https://github.com/NixOS/nixpkgs) | [Spectrum OS ❄️](https://spectrum-os.org) | [OCI 📖](https://github.com/opencontainers/runtime-spec), [Distrobox](https://github.com/89luca89/distrobox) | [Fedora Silverblue](https://fedoraproject.org/silverblue) |
| ✅ | Kernel | [Zen Linux Kernel 🌐💣](https://github.com/zen-kernel/zen-kernel) | [Asterinas 🦀](https://github.com/asterinas/asterinas), [Redox OS 🦀](https://gitlab.redox-os.org/redox-os/redox) | [Rust For Linux 🦀](https://rust-for-linux.com/) | |
| 🚧 | Libc | [Glibc 💣](https://en.wikipedia.org/wiki/Glibc) | [Musl 💣](https://www.musl-libc.org), [Relibc 🦀](https://github.com/redox-os/relibc) | [Gcompat 💣](https://git.adelielinux.org/adelie/gcompat) | |
| 🚫 | Init System | [Systemd 💣](https://github.com/systemd/systemd) | [Redox Init 🦀](https://gitlab.redox-os.org/redox-os/init) [Systemd-rs 🦀](https://github.com/KillingSpark/rustysd) | ⬅️ | |
| 🚧 | Inter-process Communication | [Dbus 💣](https://gitlab.freedesktop.org/dbus/dbus) | [Zlink 🦀](https://github.com/z-galaxy/zlink) | [Busd 🦀](https://github.com/dbus2/busd) | |
| 🚫 | Multimedia Server | [Pipewire 💣](https://gitlab.freedesktop.org/pipewire/pipewire) | [Pipewire-native-rs 🦀](https://gitlab.freedesktop.org/pipewire/pipewire-native-rs) | ⬅️ | [Pulseaudio 💣](https://gitlab.freedesktop.org/pulseaudio/pulseaudio) |
| 🚫 | XR Runtime | [Monado 💣](https://gitlab.freedesktop.org/monado/monado) | | [OpenXR 📖](https://www.khronos.org/openxr) | [Arcan 💣](https://github.com/letoram/arcan) |
| ✅ | Filesystem | [Btrfs 📖💣](https://btrfs.wiki.kernel.org/index.php/Main_Page) | [Fxfs 🦀](https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/storage/fxfs) [Redoxfs 🦀](https://gitlab.redox-os.org/redox-os/redoxfs) | 🆗 | [Ext4 📖💣](https://docs.kernel.org/filesystems/ext4/) |
| ✅ | Sandboxing | [Hakoniwa 🦀](https://github.com/souk4711/hakoniwa) | | | [Bubblewrap 💣](https://github.com/containers/bubblewrap) |

</details>

### 📚 Libraries

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Compression | | [Zstd-rs 🦀](https://github.com/KillingSpark/zstd-rs) | [Zlib-rs 🦀](https://github.com/memorysafety/zlib-rs) | [Zlib 💣](https://github.com/madler/zlib) |
| ✅ | TLS Protocol | [Rustls 🦀](https://github.com/rustls/rustls) | | 🆗 | [Openssl 💣](https://github.com/openssl/openssl) |
| ✅ | HTTP Protocol | [Hyper 🦀](https://github.com/hyperium/hyper) | | 🆗 | [Nghttp2 💣](https://github.com/nghttp2/nghttp2), [Nghttp3 💣](https://github.com/ngtcp2/nghttp3) |
| ✅ | HTTP Client | [Reqwest 🦀](https://github.com/seanmonstar/reqwest) | | 🆗 | [Curl 💣](https://github.com/curl/curl) |
| ✅ | SSH Protocol | [Russh 🦀](https://github.com/warp-tech/russh) | | 🆗 | [OpenSSH 💣](https://github.com/openssh/openssh-portable) |
| ✅ | Font Rendering | [Cosmic-text 🦀](https://github.com/pop-os/cosmic-text) | | 🆗 | [HarfBuzz 💣](https://github.com/harfbuzz/harfbuzz), [FreeType 💣](https://github.com/freetype/freetype) |
| ✅ | Graphics Renderer | [Wgpu 🦀](https://github.com/gfx-rs/wgpu) | | 🆗 | [Skia 💣](https://github.com/google/skia), [Cairo 💣](https://www.cairographics.org) |
| ✅ | Compositor Framework | [Smithay 🦀](https://github.com/Smithay/smithay) | | 🆗 | [Mutter 💣](https://gitlab.gnome.org/GNOME/mutter) |
| 🚧 | UI Toolkit | [React 🐒](https://react.dev) | [WIP Toolkit 🔥](https://tangled.org/@overby.me/overby.me/tree/main/mojo-wasm), [Dixous 🦀](https://github.com/dioxusLabs/dioxus) | [Web Component 📖](https://www.webcomponents.org/) | |
| 🚧 | UI Components | [MUI 🐒](https://mui.com) | [Dioxus Components 🦀](https://github.com/DioxusLabs/components) | 🆗 | |
| 🚫 | XR Toolkit | [Stereokit 💣](https://github.com/StereoKit/StereoKit) | | 🆗 | |
| 🚧 | Browser Engine | [Gecko 🦀💣](https://en.wikipedia.org/wiki/Gecko_(software)) | [Servo 🦀](https://github.com/servo/servo) | ⬅️ | |
| 🚫 | ECMAScript Engine | [V8 💣](https://v8.dev) | [Boa 🦀](https://github.com/boa-dev/boa), [Nova 🦀](https://github.com/trynova/nova) | 🆗 | |
| ✅ | ECMAScript Compiler | [SWC 🦀](https://github.com/swc-project/swc) | | 🆗 | [Babel 🐒](https://github.com/babel/babel) |

</details>

### 🏗️ Infrastructure

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | WebAssembly Runtime | [Wasmtime 🦀](https://github.com/bytecodealliance/wasmtime) | | [WASI 📖](https://wasi.dev) | |
| ✅ | ECMAScript Runtime | [Deno 🦀](https://github.com/denoland/deno) | | [Deno Node APIs](https://docs.deno.com/runtime/reference/node_apis) | [Node.js 💣](https://github.com/nodejs/node) |
| ✅ | Container Runtime | [Youki 🦀](https://github.com/containers/youki) | | [OCI 📖](https://github.com/opencontainers/runtime-spec) | [Runc 🐹](https://github.com/opencontainers/runc) |
| ✅ | Virtualization | [Cloud Hypervisor 🦀](https://github.com/cloud-hypervisor/cloud-hypervisor) | | | [QEMU 💣](https://github.com/qemu/qemu) |
| 🚧 | Meta Database | [Hasura λ➡️🦀](https://github.com/hasura/graphql-engine) | [Surrealdb ⏳🦀](https://github.com/surrealdb/surrealdb) | [GraphQL](https://graphql.org) | |
| 🚧 | Database | [Postgres 💣](https://github.com/postgres/postgres) | [Tikv 🦀](https://github.com/tikv/tikv) | 🆗 | |
| 🚧 | Storage Engine | | [Sled 🦀](https://github.com/spacejam/sled), [Fjall 🦀](https://github.com/fjall-rs/fjall) | 🆗 | [RocksDB 💣](https://github.com/facebook/rocksdb) |
| ✅ | Web Server | [Caddy 🐹](https://github.com/caddyserver/caddy) | [Moella 🦀](https://github.com/Icelk/moella) | | [Nginx 💣](https://github.com/nginx/nginx) |
| ✅ | Email Server | [Stalwart 🦀](https://stalw.art) | | [IMAP 📖](https://en.wikipedia.org/wiki/Internet_Message_Access_Protocol), [POP3 📖](https://en.wikipedia.org/wiki/Post_Office_Protocol) | [Postfix 💣](https://www.postfix.org), [Dovecot 💣](https://www.dovecot.org) |
| ✅ | Virtual Private Network | [Tailscale 🐹](https://github.com/tailscale/tailscale) | [Innernet 🦀](https://github.com/tonarino/innernet) | | |
| 🚧 | Monorepo | | [Josh 🦀](https://github.com/josh-project/josh), [Mega 🦀🐒](https://github.com/web3infra-foundation/mega), [Google Piper 🔒](https://en.wikipedia.org/wiki/Piper_(source_control_system)) | 🆗 | |

</details>

## 🛠️ Development

[⬆](#toc)

<details open>

### 🔧 Tooling

#### Tools & Utilities

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Build Script | [Just 🦀](https://github.com/casey/just) | | Rusty Make ([Rusty Bash 🦀](https://github.com/shellgei/rusty_bash)) | [GNU Make 💣](https://en.wikipedia.org/wiki/Make_(software)) |
| ✅ | Editor | [Helix 🦀](https://github.com/helix-editor/helix) | | 🆗 | [Neovim 💣](https://github.com/neovim/neovim) |
| ✅ | IDE | [Zed 🦀](https://github.com/zed-industries/zed) | | [LSP](https://github.com/microsoft/language-server-protocol), [DAP](https://github.com/Microsoft/debug-adapter-protocol), [BSP](https://github.com/build-server-protocol/build-server-protocol) | [VS Codium 🐒💣](https://github.com/VSCodium/vscodium) |
| ✅ | System Call Tracing | [Lurk 🦀](https://github.com/JakWai01/lurk), [Tracexec 🦀](https://github.com/kxxt/tracexec) | | 🆗 | [Strace 💣](https://github.com/strace/strace) |
| ✅ | Network Client | [Xh 🦀](https://github.com/ducaale/xh) | | 🆗 | [Curl 💣](https://github.com/curl/curl) |
| ✅ | Environment Loader | [Direnv 🐹](https://github.com/direnv/direnv) | [Envy 🦀](https://github.com/mre/envy) | ⬅️ | |
| ✅ | Pager | [Tailspin 🦀](https://github.com/bensadeh/tailspin) | | 🆗 | [Less 💣](https://github.com/gwsw/less) |
| ✅ | Performance Profiler | [Samply 🦀](https://github.com/mstange/samply) | | 🆗 | [Perf 💣](https://perf.wiki.kernel.org/) |
| ✅ | TCP Tunnel | [Bore 🦀](https://github.com/ekzhang/bore) | | 🆗 | |

#### Version Control

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Version Control | [Jujutsu 🦀](https://github.com/jj-vcs/jj), [Git 💣](https://github.com/git/git) | [Gitoxide 🦀](https://github.com/Byron/gitoxide) | [Gix 🦀](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md#gix) | |
| ✅ | Version Control TUI | [Lazyjj 🦀](https://github.com/Cretezy/lazyjj) | | | |
| ✅ | Merger | [Mergiraf 🦀](https://codeberg.org/mergiraf/mergiraf) | | ⬅️ | |
| ✅ | Pre-commit Manager | [Prek 🦀](https://github.com/j178/prek) | | | [Pre-commit 🐍](https://github.com/pre-commit/pre-commit) |
| ✅ | Spell Checker | [Typos 🦀](https://github.com/crate-ci/typos) | | | |
| ✅ | Commit Linter | [Commitlint-rs 🦀](https://github.com/KeisukeYamashita/commitlint-rs) | | | |
| ✅ | Secret Scanner | [Ripsecrets 🦀](https://github.com/sirwart/ripsecrets) | | | |
| ✅ | Markdown Linter | [Rumdl 🦀](https://github.com/rvben/rumdl) | | | |

</details>

### ❄️ Configuration

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| 🚧 | Package Manager | [Nix 🌐💣](https://github.com/NixOS/nix) | [Snix 🦀](https://git.snix.dev/snix/snix) | ⬅️ | |
| 🚧 | Language | [Nix 🌐💣](https://github.com/NixOS/nix) | [Nickel 🦀](https://github.com/tweag/nickel), [Glistix 🦀](https://github.com/Glistix/glistix) | | |
| ✅ | Formatter | [Alejandra 🦀](https://github.com/kamadorueda/alejandra) | | | [Nixfmt λ](https://github.com/NixOS/nixfmt) |
| ✅ | Static Analyzer | [Statix 🦀](https://github.com/oppiliappan/statix), [Deadnix 🦀](https://github.com/astro/deadnix), [Nixpkgs-Lint 🦀](https://github.com/nix-community/nixpkgs-lint) | | | |
| ✅ | Language Server | [Nil 🦀](https://github.com/oxalica/nil) | | | [Nixd 💣](https://github.com/nix-community/nixd) |
| ✅ | Binary Cache | [Harmonia 🦀](https://github.com/nix-community/harmonia) | [Attic 🦀](https://github.com/zhaofengli/attic) | 🆗 | [Cachix 🔒λ](https://github.com/cachix/cachix) |
| ✅ | Config Manager | [Home Manager 🌐❄️](https://github.com/nix-community/home-manager) | | | |
| ✅ | Secret Manager | [Ragenix 🦀❄️](https://github.com/yaxitech/ragenix) | | | [Agenix 🐹❄️](https://github.com/ryantm/agenix) |
| ✅ | Deployment | [Colmena 🦀️❄️](https://github.com/zhaofengli/colmena) | | | |
| ✅ | Developer Environment | [Devenv 🦀️❄️](https://github.com/cachix/devenv) | [Organist ❄️](https://github.com/nickel-lang/organist) | 🆗 | |
| ✅ | Flake Framework | [Flakelight ❄️](https://github.com/nix-community/flakelight) | | | [Flake-parts ❄️](https://github.com/hercules-ci/flake-parts) |
| ✅ | File Locator | [Nix-index 🦀](https://github.com/nix-community/nix-index), [Comma 🦀](https://github.com/nix-community/comma) | | | |
| ✅ | Rust Integration | [Crate2nix 🦀❄️](https://github.com/nix-community/crate2nix) | | | [Crane ❄️](https://github.com/ipetkov/crane) |
| ✅ | Python Integration | [Uv2nix ❄️](https://github.com/pyproject-nix/uv2nix) | | | |
| ✅ | Nodejs Integration | [Yarnix ❄️](https://github.com/FactbirdHQ/yarnix) | | | |
| ✅ | Package Generation | [Nix-init 🦀](https://github.com/nix-community/nix-init) + [Nurl 🦀](https://github.com/nix-community/nurl) | | | |
| ✅ | Derivation Difference | [Nix-diff-rs 🦀](https://github.com/Mic92/nix-diff-rs) | | | [Nix-diff λ](https://github.com/Gabriella439/nix-diff) |
| ✅ | Dependency Explorer | [Nix-du 🦀](https://github.com/symphorien/nix-du) | | | [Nix-tree λ](https://github.com/utdemir/nix-tree) |

</details>

### 🦀 Systems

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Language | [Rust 🦀](https://github.com/rust-lang/rust) | | [cxx 🦀](https://github.com/dtolnay/cxx), [bindgen 🦀](https://github.com/rust-lang/rust-bindgen) | |
| 🚧 | Compiler Framework | [Mlir 💣](https://github.com/llvm/llvm-project/tree/main/mlir), [LLVM 💣](https://github.com/llvm/llvm-project) | [Cranelift 🦀](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift) | ⬅️ | |
| 🚧 | Linker | [Mold 💣](https://github.com/rui314/mold) | [Wild 🦀](https://github.com/davidlattimore/wild) | ⬅️ | [GNU ld 💣](https://sourceware.org/binutils) |
| ✅ | Formatter | [Rustfmt 🦀](https://github.com/rust-lang/rustfmt) | | | |
| ✅ | Language Server | [Rust-analyzer 🦀](https://github.com/rust-lang/rust-analyzer) | | | |

</details>

### 🌐 Web

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Language | [TypeScript 🐒](https://github.com/microsoft/TypeScript) | [Mojo 🔒🔥](https://github.com/modularml/mojo) | | |
| ✅ | Bundler | [Rsbuild 🦀](https://github.com/web-infra-dev/rsbuild) | [Farm 🦀](https://github.com/farm-fe/farm) | 🆗 | [Webpack 🐒](https://github.com/webpack/webpack) |
| ✅ | Formatter | [Biome 🦀](https://github.com/biomejs/biome) | | 🆗 | [Prettier 🐒](https://github.com/prettier/prettier) |
| 🚧 | ECMAScript Typechecker | [TypeScript 🐒](https://github.com/microsoft/typescript) | [Ezno 🦀](https://github.com/kaleidawave/ezno), [TypeScript Go 🐹](https://github.com/microsoft/typescript-go) | 🆗 | |
| ✅ | Certificate Generation | [Rcgen 🦀](https://github.com/rustls/rcgen) | | 🆗 | [Mkcert 🐹](https://github.com/FiloSottile/mkcert) |
| 🚧 | Language Server | [TypeScript 🐒](https://github.com/microsoft/TypeScript) | [TypeScript Go 🐹](https://github.com/microsoft/typescript-go) | 🆗 | |

</details>

### 🐍 Scripting

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Language | [Mojo 🔒🔥](https://github.com/modularml/mojo) | | [RustPython 🦀](https://github.com/RustPython/RustPython), [WASI 📖](https://github.com/WebAssembly/WASI), [Interface Types 📖](https://github.com/WebAssembly/interface-types/tree/main/proposals/interface-types) | [TypeScript 🐒🐹](https://github.com/microsoft/TypeScript) |
| ✅ | Package Manager | [Uv 🦀](https://github.com/astral-sh/uv) | [Pixi 🦀](https://github.com/prefix-dev/pixi) | 🆗 | [Poetry 🐍](https://github.com/python-poetry/poetry) |
| ✅ | Formatter | [Ruff 🦀](https://github.com/astral-sh/ruff) | | 🆗 | [Black 🐍](https://github.com/psf/black) |
| ✅ | Linter | [Ruff 🦀](https://github.com/astral-sh/ruff) | | 🆗 | [Flake8 🐍](https://github.com/PyCQA/flake8) |
| ✅ | Type Checker | [Ty 🦀](https://github.com/astral-sh/ty) | | 🆗 | [Mypy 🐍](https://github.com/python/mypy) |
| ✅ | Profiler | [Py-spy 🦀](https://github.com/benfred/py-spy) | | 🆗 | [Yappi](https://github.com/sumerc/yappi) |
| ✅ | Language Server | [Ty 🦀](https://github.com/astral-sh/ty) | | 🆗 | [Pyright 🐒](https://github.com/microsoft/pyright) |

</details>

## 📱 Applications

[⬆](#toc)

### 💻 Command Line

[⬆](#toc)

<details open>

#### Filesystem Operations

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Change Directory | [Zoxide 🦀](https://github.com/ajeetdsouza/zoxide) | [Lacy 🦀](https://github.com/timothebot/lacy) | ⬅️ | [Bash Cd 💣](https://en.wikipedia.org/wiki/Bash_(Unix_shell)) |
| ✅ | Copy | [Nushell Cp 🦪🦀](https://github.com/nushell/nushell) | [Cpx 🦀](https://github.com/11happy/cpx) | ⬅️ | [Bash Cp 💣](https://en.wikipedia.org/wiki/Bash_(Unix_shell)) |
| ✅ | Directory Usage | [Dust 🦀](https://github.com/bootandy/dust) | | [Uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://en.wikipedia.org/wiki/GNU_Core_Utilities) |
| ✅ | Find Files | [Fd 🦀](https://github.com/sharkdp/fd) | | [Uutils Findutils 🦀](https://github.com/uutils/findutils) | [Findutils 💣](https://en.wikipedia.org/wiki/List_of_GNU_packages#Base_system) |
| ✅ | Find Patterns | [Ripgrep 🦀](https://github.com/BurntSushi/ripgrep) | | 🆗 | [Grep 💣](https://en.wikipedia.org/wiki/Grep) |
| ✅ | Find & Replace | [Ast-grep 🦀](https://github.com/ast-grep/ast-grep) | | 🆗 | [Sed 💣](https://en.wikipedia.org/wiki/Sed) |
| ✅ | File Differences | [Batdiff 🦀](https://github.com/eth-p/bat-extras) + [Delta 🦀](https://github.com/dandavison/delta) | [Difftastic 🦀](https://github.com/wilfred/difftastic) | [Uutils Diffutils 🦀](https://github.com/uutils/diffutils) | [Diffutils 💣](https://en.wikipedia.org/wiki/List_of_GNU_packages#Base_system) |
| ✅ | Hex Viewer | [Hyxel 🦀](https://github.com/hyxel/hyxel) | | | [Util Linux Hexdump 💣](https://github.com/util-linux/util-linux) |
| ✅ | Tree Viewer | [Tre 🦀](https://github.com/dduan/tre) | | 🆗 | [Tree 💣](https://oldmanprogrammer.net/source.php?dir=projects/tree) |

#### Process Management

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | List Processes | [Procs 🦀](https://github.com/dalance/procs) | | 🆗 | [Ps 💣](https://gitlab.com/procps-ng/procps) |
| ✅ | Process Monitor | [Bottom 🦀](https://github.com/ClementTsang/bottom) | | 🆗 | [Top 💣](https://gitlab.com/procps-ng/procps) |
| ✅ | Parallel Processing | [Rust Parallel 🦀](https://github.com/aaronriekenberg/rust-parallel) | | 🆗 | [GNU Parallel 💣](https://en.wikipedia.org/wiki/GNU_parallel) |
| ✅ | Terminal Workspace | [Zellij 🦀](https://github.com/zellij-org/zellij) | | 🆗 | [Tmux 💣](https://github.com/tmux/tmux) |

#### Networking

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Network Client | [Xh 🦀](https://github.com/ducaale/xh) | | 🆗 | [Curl 💣](https://github.com/curl/curl) |
| ✅ | Ping | [Gping 🦀](https://github.com/orf/gping) | | | [Ping 💣](https://en.wikipedia.org/wiki/Ping_(networking_utility)) |
| ✅ | Port Scanner | [RustScan 🦀](https://github.com/rustscan/rustscan) | | 🆗 | [Nmap 💣](https://github.com/nmap/nmap) |
| 🚧 | PGP | [GnuPG 💣](https://gnupg.org) | [Sequoia-PGP 🦀](https://gitlab.com/sequoia-pgp/sequoia) | 🆗 | |
| 🚧 | SSH | [OpenSSH 💣](https://github.com/openssh/openssh-portable) | [Sunset 🦀](https://github.com/mkj/sunset) | 🆗 | |

#### System Utilities

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Shell | [Nushell 🦪🦀](https://github.com/nushell/nushell) | | [Brush 🦀](https://github.com/reubeno/brush), [Rusty Bash 🦀](https://github.com/shellgei/rusty_bash) | [Bash 💣](https://en.wikipedia.org/wiki/Bash_(Unix_shell)) |
| ✅ | Shell Prompt | [Starship 🦀](https://github.com/starship/starship) | | 🆗 | |
| ✅ | Core Utilities | [Nushell Builtins 🦪🦀](https://github.com/nushell/nushell) | | [Uutils 🦀](https://github.com/uutils/coreutils) | [Coreutils 💣](https://en.wikipedia.org/wiki/GNU_Core_Utilities) |
| ✅ | List Files | [Nushell Ls 🦪🦀](https://github.com/nushell/nushell) | [Eza 🦀](https://github.com/eza-community/eza) | 🆗 | [Ls 💣](https://en.wikipedia.org/wiki/GNU_Core_Utilities) |
| ✅ | Superuser | [Sudo-rs 🦀](https://github.com/memorysafety/sudo-rs) | | ⬅️ | [Sudo 💣](https://github.com/sudo-project/sudo) |
| ✅ | Fortune | [Fortune-kind 🦀](https://github.com/cafkafk/fortune-kind) | | ⬅️ | [Fortune-mod 💣](https://github.com/shlomif/fortune-mod) |
| ✅ | System Fetch | [Microfetch 🦀](https://github.com/NotAShelf/microfetch) | | 🆗 | |
| ✅ | Fuzzy Finder | [Television 🦀](https://github.com/alexpasmantier/television) | | 🆗 | [Fzf 🐹](https://github.com/junegunn/fzf) |
| ✅ | Benchmark | [Hyperfine 🦀](https://github.com/sharkdp/hyperfine) | | | [time 💣](https://en.wikipedia.org/wiki/Time_(Unix)) |

</details>

### 🖥️ Desktop Environment

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Color Scheme | [Catppuccin](https://github.com/catppuccin/catppuccin) | [Frosted Effect](https://github.com/pop-os/cosmic-epoch/issues/604) | 🆗 | [Adwaita](https://gitlab.gnome.org/GNOME/libadwaita) |
| ✅ | Wallpaper | [Nix-wallpaper ❄️](https://github.com/lunik1/nix-wallpaper) | | 🆗 | |
| ✅ | Design System | [Material You 🔒](https://m3.material.io) | | 🆗 | [Material Design 2 🔒](https://m2.material.io) |
| ✅ | Desktop Environment | [Cosmic Epoch 🦀](https://github.com/pop-os/cosmic-epoch) | | 🆗 | [Gnome Shell 💣](https://gitlab.gnome.org/GNOME/gnome-shell) |
| ✅ | XR Environment | [Stardust XR 🦀](https://github.com/StardustXR/server) | [Breezy Desktop](https://github.com/wheaney/breezy-desktop) | 🆗 | [Safespaces 🌙](https://github.com/letoram/safespaces) |

</details>

### 🚀 Productivity

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | File Manager | [Cosmic Files 🦀](https://github.com/pop-os/cosmic-files) | | 🆗 | [GNOME Files 💣](https://gitlab.gnome.org/GNOME/nautilus) |
| ✅ | Terminal | [Cosmic Term 🦀](https://github.com/pop-os/cosmic-term) | | 🆗 | [Wezterm 🦀](https://github.com/wez/wezterm), [GNOME Console 💣](https://gitlab.gnome.org/GNOME/console) |
| ✅ | Web Browser | [Zen Browser 💣🦀](https://zen-browser.app) | [Verso 🦀](https://github.com/versotile-org/verso) | [Chrome Extension API 🔒](https://developer.chrome.com/docs/extensions/reference) | [Firefox 💣🦀](https://github.com/mozilla/gecko-dev), [Unbraved Brave 💣🦀](https://github.com/MulesGaming/brave-debullshitinator) |
| ✅ | Application Store Frontend | [Cosmic Store 🦀](https://github.com/pop-os/cosmic-store) | | 🆗 | [GNOME Software 💣](https://gitlab.gnome.org/GNOME/gnome-software) |
| 🚫 | Application Store Backend | [Flatpak 💣](https://github.com/flatpak/flatpak) | | 🆗 | [Snap 🔒](https://github.com/canonical/snapd), [AppImage 💣](https://github.com/AppImage) |
| ✅ | Office Suite | [OnlyOffice 🐒](https://www.onlyoffice.com) | | [OpenDocument Format 📖](https://en.wikipedia.org/wiki/OpenDocument) | [LibreOffice 💣🐷](https://www.libreoffice.org) |
| ✅ | Remote Desktop | [Rustdesk 🦀](https://github.com/rustdesk/rustdesk) | | [VNC](https://en.wikipedia.org/wiki/VNC) | [GNOME Remote Desktop 💣](https://gitlab.gnome.org/GNOME/gnome-remote-desktop) |

</details>

### 🎨 Media

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Media Player | [Cosmic Player 🦀](https://github.com/pop-os/cosmic-player) | | [FFMPEG 💣](https://github.com/FFmpeg/FFmpeg), [GStreamer 💣](https://gitlab.freedesktop.org/gstreamer) | [Mpv 💣](https://github.com/mpv-player/mpv) |
| 🚧 | Raster Graphics | [GIMP 💣](https://gitlab.gnome.org/GNOME/gimp) | [Graphite 🦀](https://github.com/GraphiteEditor/Graphite) | 🆗 | |
| ✅ | Vector Graphics | [Graphite 🦀](https://github.com/GraphiteEditor/Graphite) | | 🆗 | [Inkscape 💣](https://gitlab.com/inkscape/inkscape) |
| ✅ | Screen Recorder | [Kooha 🦀](https://github.com/SeaDve/Kooha) | | 🆗 | [Mutter Built-in Recorder 💣](https://github.com/GNOME/mutter) |
| ✅ | Diagram Generation | [Layout 🦀](https://github.com/nadavrot/layout) | | 🆗 | [Graphviz 💣](https://graphviz.org) |
| ✅ | Typesetter | [Typst 🦀](https://github.com/typst) | | 🆗 | [LaTeX 💣](https://github.com/latex3/latex3) |
| 🚧 | Image Optimizer | | [Cavif-rs 🦀](https://github.com/kornelski/cavif-rs) | 🆗 | [Oxipng 🦀](https://github.com/shssoichiro/oxipng), [Optipng 💣](https://optipng.sourceforge.net) |
| 🚧 | Image Processing | | [Wondermagick 🦀](https://github.com/Shnatsel/wondermagick) | 🆗 | [ImageMagick 💣](https://github.com/ImageMagick/ImageMagick) |

</details>

### 🌐 Browser Extensions

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | Keyboard Navigation | [Surfingkeys 🐒](https://github.com/brookhong/Surfingkeys) | | 🆗 | |
| ✅ | Advertising Blocker | [uBlock Origin 🐒](https://github.com/gorhill/uBlock) | | 🆗 | |
| ✅ | Grammar Checker | [Harper 🦀](https://github.com/Automattic/harper) | | 🆗 | [LanguageTools 🐷](https://github.com/languagetools) |
| ✅ | Distraction Blocker | [LeechBlock NG 🐒](https://github.com/proginosko/LeechBlockNG) | | 🆗 | |

</details>

### 📱 Mobile

[⬆](#toc)

<details open>

| Status | Component | Current | Research & Development | Compatibility | Legacy |
|:-:|-|-|-|-|-|
| ✅ | OS | [/e/OS 🌐🇪🇺](https://e.foundation/e-os) | | [MicroG 🌐🐷](https://microg.org), [Magisk 🦀💣🐷](https://github.com/topjohnwu/Magisk) | [GrapheneOS 🇨🇦](https://grapheneos.org) |
| ✅ | Launcher | [Olauncher 🐷](https://github.com/tanujnotes/Olauncher) | | 🆗 | [Minimalist Phone 🔒](https://www.minimalistphone.com) |
| ✅ | Keyboard | [Thumb-Key 🐷](https://github.com/dessalines/thumb-key) | | 🆗 | [OpenBoard 🐷](https://github.com/openboard-team/openboard) |
| ✅ | Alarm | [Chrono 🐷](https://github.com/vicolo-dev/chrono) | | 🆗 | [Sleep 🔒](https://sleep.urbandroid.org) |
| ✅ | Browser | [Fennec 💣🦀](https://f-droid.org/en/packages/org.mozilla.fennec_fdroid) | | 🆗 | [Mull 💣🦀](https://github.com/mull-project/mull) |
| ✅ | Maps | [CoMaps 💣](https://comaps.app) | | [Openstreetmap 🌐📖](https://www.openstreetmap.org) | [Organic Maps 💣](https://organicmaps.app), [Google Maps 🔒🇺🇸](https://maps.google.com)|
| ✅ | Distraction Blocker | [TimeLimit 🐷](https://codeberg.org/timelimit/timelimit-android) | | 🆗 | |
| ✅ | Authenticator | [Aegis 🐷](https://getaegis.app) | | [HOTP 📖](https://en.wikipedia.org/wiki/HMAC-based_One-time_Password_algorithm), [TOTP 📖](https://en.wikipedia.org/wiki/Time-based_One-time_Password_algorithm) | |
| ✅ | Music Recognition | [Audile 🐷](https://github.com/aleksey-saenko/MusicRecognizer) | | 🆗 | [Soundhound 🔒🇺🇸](https://www.soundhound.com) |
| ✅ | Malware Scanner | [Hypatia 🐷](https://github.com/MaintainTeam/Hypatia) | | 🆗 | |
| ✅ | Developer Environment | [Nix-on-droid ❄️🐍](https://github.com/nix-community/nix-on-droid) | | 🆗 | [Termux 🐷💣](https://github.com/termux/termux-app) |

</details>
