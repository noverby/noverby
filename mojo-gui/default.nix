{
  devShells.mojo-gui = pkgs: let
    # Rust toolchain with Windows cross-compilation target (from rust-overlay).
    # This is NOT added to PATH (to avoid shadowing the devenv-provided Rust).
    # Instead, its sysroot is exposed via the `rust-sysroot-windows` helper script
    # so that cross-compilation recipes can pass --sysroot to rustc.
    rustWithWindows = pkgs.rust-bin.stable.latest.default.override {
      extensions = ["rust-src"];
      targets = ["x86_64-unknown-linux-gnu" "x86_64-pc-windows-gnu"];
    };
  in {
    packages = with pkgs; [
      # Build tools
      just
      mojo
      mojo-windows

      # Web renderer (WASM + TypeScript)
      deno
      wabt
      llvmPackages_latest.llvm
      llvmPackages_latest.lld
      wasmtime.lib
      wasmtime.dev
      servo
      jq

      # Desktop renderer (Blitz shim build)
      pkg-config
      cmake
      python3

      # Desktop renderer (Wayland + GPU runtime deps)
      fontconfig
      freetype
      libxkbcommon
      wayland
      vulkan-loader
      vulkan-headers
      libGL

      # Windows cross-compilation (MinGW-w64 linker for x86_64-pc-windows-gnu)
      # Strip nix-support/ to avoid setup hooks that set CC/AR/etc. to cross names,
      # which would break native builds. We only need the binaries on PATH.
      (symlinkJoin {
        name = "mingw-w64-cc-noenv";
        paths = [pkgsCross.mingwW64.stdenv.cc];
        postBuild = "rm -rf $out/nix-support";
      })

      # Windows verification (Wine)
      wine64Packages.stable

      # Helper: prints the Rust sysroot path that includes x86_64-pc-windows-gnu std.
      # Usage in justfile: _rust-sysroot-windows := `rust-sysroot-windows`
      (writeShellScriptBin "rust-sysroot-windows" ''
        echo -n "${rustWithWindows}"
      '')

      # Helper: prints the MinGW-w64 library path (contains libpthread.a etc.)
      # Usage in justfile: _mingw-lib-path := `mingw-lib-path`
      (writeShellScriptBin "mingw-lib-path" ''
        echo -n "${pkgsCross.mingwW64.windows.pthreads}/lib"
      '')

      # Helper: prints the MinGW-w64 mcfgthread library path
      # Usage in justfile: _mingw-mcf-lib-path := `mingw-mcf-lib-path`
      (writeShellScriptBin "mingw-mcf-lib-path" ''
        echo -n "${pkgsCross.mingwW64.windows.mcfgthreads}/lib"
      '')
    ];
  };

  # ── CI Check Derivations ──────────────────────────────────────────────
  #
  # These checks run via `nix flake check` in the Tangled CI pipeline.
  # Each check is a derivation that succeeds (exit 0) when the check passes.
  #
  # Implemented checks (S-1 from PLAN.md):
  #   - mojo-gui-test-desktop  — 75 Rust integration tests for Blitz shim (headless)
  #   - mojo-gui-test-xr       — 37 Rust integration tests for XR shim (headless)
  #   - mojo-gui-test          — 52 Mojo test suites via wasmtime
  #   - mojo-gui-test-js       — 30 JS integration test suites (~3,375 tests) via Deno
  #   - mojo-gui-build-all     — Build all 4 examples × {web, desktop, xr}

  checks = pkgs: let
    inherit (pkgs) lib;

    # ── Shared Blitz build dependencies (desktop + XR shims) ────────────
    blitzNativeBuildInputs = with pkgs; [pkg-config cmake python3];
    blitzBuildInputs = with pkgs; [
      fontconfig
      freetype
      libxkbcommon
      wayland
      vulkan-loader
      vulkan-headers
      libGL
      openssl
    ];

    # ── Monorepo source ─────────────────────────────────────────────────
    #
    # The Mojo tests and build-all checks need both mojo-gui/ and
    # mojo-wasmtime/ (sibling project). Using the flake root as source
    # ensures both are available. Nix filters to git-tracked files only.
    monoSrc = ../.;

    # ── Deno dependency cache (fixed-output derivation) ─────────────────
    #
    # The JS tests import `npm:linkedom` which Deno fetches from npm.
    # In the Nix sandbox there is no network access, so we pre-fetch all
    # Deno dependencies into a fixed-output derivation whose hash is
    # pinned here. To update after changing JS dependencies:
    #
    #   1. Set outputHash to lib.fakeHash
    #   2. Run: nix build .#checks.x86_64-linux.mojo-gui-test-js 2>&1 | grep 'got:'
    #   3. Replace outputHash with the printed hash
    #
    denoDeps = pkgs.stdenv.mkDerivation {
      name = "mojo-gui-deno-deps";
      src = ./web;
      nativeBuildInputs = [pkgs.deno pkgs.cacert];

      outputHash = "sha256-YsA7jpdtIBypy1zUoe3M79NubahNCkdlK3gPPcsbTjw=";
      outputHashMode = "recursive";
      outputHashAlgo = "sha256";

      buildPhase = ''
        export DENO_DIR=$out
        export SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt
        deno cache --lock=deno.lock test-js/run.ts
      '';
      installPhase = "true";
    };

    # ── Deno dependency cache for XR web tests ──────────────────────────
    #
    # Same npm:linkedom dependency as the main JS tests, but cached from
    # the xr/web/deno.lock file. The XR web runtime tests also import
    # from web/runtime/ (shared Interpreter), but those are local .ts
    # files that don't require npm fetching.
    #
    # To update after changing XR JS dependencies:
    #   1. Set outputHash to lib.fakeHash
    #   2. Run: nix build .#checks.x86_64-linux.mojo-gui-test-xr-js 2>&1 | grep 'got:'
    #   3. Replace outputHash with the printed hash
    #
    denoXrDeps = pkgs.stdenv.mkDerivation {
      name = "mojo-gui-deno-xr-deps";
      src = ./xr/web;
      nativeBuildInputs = [pkgs.deno pkgs.cacert];

      outputHash = "sha256-YsA7jpdtIBypy1zUoe3M79NubahNCkdlK3gPPcsbTjw=";
      outputHashMode = "recursive";
      outputHashAlgo = "sha256";

      buildPhase = ''
        export DENO_DIR=$out
        export SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt
        deno cache --lock=deno.lock test-js/run.ts
      '';
      installPhase = "true";
    };

    # ── System libraries required by the Mojo native linker ─────────────
    #
    # mojo build (native target) invokes the system linker with:
    #   -lrt -ldl -lpthread -lm -lz -ltinfo
    # In the Nix sandbox, zlib and ncurses must be explicit buildInputs.
    mojoLinkInputs = with pkgs; [zlib ncurses];
  in {
    # ── 1. Desktop Blitz shim integration tests ────────────────────────
    #
    # 75 Rust integration tests covering: context lifecycle, DOM operations,
    # attributes, tree structure, templates, events, mutation batching,
    # DOM serialization, ID mapping, stress tests, and integration scenarios.
    # All tests run in headless mode — no display server needed.
    mojo-gui-test-desktop = pkgs.rustPlatform.buildRustPackage {
      pname = "mojo-gui-test-desktop";
      version = "0.0.0";
      src = ./desktop/shim;

      cargoLock = {
        lockFile = ./desktop/shim/Cargo.lock;
        allowBuiltinFetchGit = true;
      };

      nativeBuildInputs = blitzNativeBuildInputs;
      buildInputs = blitzBuildInputs;

      doCheck = true;

      # We only care about the test result, not the build artifacts.
      installPhase = "touch $out";

      meta.description = "mojo-gui desktop Blitz shim integration tests (headless)";
    };

    # ── 2. XR shim integration tests ──────────────────────────────────
    #
    # 37 Rust integration tests covering: session lifecycle, panel lifecycle,
    # DOM operations, attributes, text nodes, placeholders, serialization,
    # events, raycasting, focus, frame loop, reference spaces, ID mapping,
    # stack operations, multi-panel isolation, Blitz document structure,
    # nested elements, and layout resolution.
    # All tests run in headless mode — no XR runtime or GPU needed.
    mojo-gui-test-xr = pkgs.rustPlatform.buildRustPackage {
      pname = "mojo-gui-test-xr";
      version = "0.0.0";
      src = ./xr/native/shim;

      cargoLock = {
        lockFile = ./xr/native/shim/Cargo.lock;
        allowBuiltinFetchGit = true;
      };

      nativeBuildInputs = blitzNativeBuildInputs;
      buildInputs = blitzBuildInputs ++ [pkgs.openxr-loader];

      doCheck = true;

      # We only care about the test result, not the build artifacts.
      installPhase = "touch $out";

      meta.description = "mojo-gui XR shim integration tests (headless)";
    };

    # ── 3. Mojo test suites ───────────────────────────────────────────
    #
    # 52 Mojo test suites run via wasmtime. Each test module is compiled
    # to a native binary that internally loads the WASM module and
    # exercises the reactive framework end-to-end.
    #
    # Pipeline: build WASM → precompile → compile test binaries → run.
    mojo-gui-test = pkgs.stdenv.mkDerivation {
      name = "check-mojo-gui-test";
      src = monoSrc;

      nativeBuildInputs = with pkgs; [
        just
        mojo
        nushell
        llvmPackages_latest.llvm # llc
        llvmPackages_latest.lld # wasm-ld
        wabt # wasm-objdump etc.
        wasmtime # wasmtime CLI (compile)
      ];

      buildInputs =
        [
          pkgs.wasmtime.lib # libwasmtime.so for test runtime
          pkgs.wasmtime.dev # wasmtime headers / pkg-config
        ]
        ++ mojoLinkInputs;

      buildPhase = ''
        export HOME=$TMPDIR

        # Ensure libwasmtime.so is findable at runtime via LD_LIBRARY_PATH.
        # The test binaries use DLHandle to dlopen it; NIX_LDFLAGS is also
        # checked as a fallback by the mojo-wasmtime loader.
        export LD_LIBRARY_PATH="${lib.makeLibraryPath [pkgs.wasmtime.lib]}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

        cd mojo-gui

        # Build WASM binary (bypass just shebang recipes — /usr/bin/env
        # may not exist in the Nix sandbox).
        cd web
        mkdir -p build
        mojo build -Werror --emit llvm -I ../core/src -I ../examples -o build/out.ll src/main.mojo
        sed -i '/call void @llvm\.lifetime\.\(start\|end\)/d' build/out.ll
        sed -i 's/ nocreateundeforpoison//g' build/out.ll
        sed -i 's/ "target-cpu"="[^"]*"//g; s/ "target-features"="[^"]*"//g' build/out.ll
        sed -i '/^attributes #[0-9]* = { }$/d' build/out.ll
        llc --mtriple=wasm64-wasi -filetype=obj build/out.ll
        wasm-ld --no-entry --export-all --allow-undefined -mwasm64 \
          --initial-memory=268435456 -o build/out.wasm build/out.o

        # Precompile WASM for fast loading
        wasmtime compile -o build/out.cwasm build/out.wasm

        # Build + run test binaries via nu scripts (called directly,
        # not through just shebang recipes)
        nu scripts/build-test-binaries.nu
        nu scripts/run-test-binaries.nu
        cd ..
      '';

      installPhase = "touch $out";

      meta.description = "mojo-gui Mojo test suites (52 suites via wasmtime)";
    };

    # ── 4. JS integration tests ───────────────────────────────────────
    #
    # 30 JS integration test suites (~3,375 tests) run via Deno.
    # Tests load the WASM binary, instantiate apps, simulate events,
    # and verify DOM mutations via the TypeScript runtime.
    #
    # Uses a pre-fetched Deno dependency cache (denoDeps) for the
    # npm:linkedom dependency, since the Nix sandbox has no network.
    mojo-gui-test-js = pkgs.stdenv.mkDerivation {
      name = "check-mojo-gui-test-js";
      src = monoSrc;

      nativeBuildInputs = with pkgs; [
        just
        mojo
        deno
        llvmPackages_latest.llvm # llc
        llvmPackages_latest.lld # wasm-ld
        wabt # wasm-objdump etc.
      ];

      buildInputs = mojoLinkInputs;

      buildPhase = ''
        export HOME=$TMPDIR

        # Point Deno at the pre-fetched dependency cache.
        # Use a writable copy since Deno may write metadata files.
        cp -r ${denoDeps} $TMPDIR/deno-cache
        chmod -R u+w $TMPDIR/deno-cache
        export DENO_DIR=$TMPDIR/deno-cache

        cd mojo-gui

        # Build WASM binary (bypass just shebang recipes — /usr/bin/env
        # may not exist in the Nix sandbox).
        cd web
        mkdir -p build
        mojo build -Werror --emit llvm -I ../core/src -I ../examples -o build/out.ll src/main.mojo
        sed -i '/call void @llvm\.lifetime\.\(start\|end\)/d' build/out.ll
        sed -i 's/ nocreateundeforpoison//g' build/out.ll
        sed -i 's/ "target-cpu"="[^"]*"//g; s/ "target-features"="[^"]*"//g' build/out.ll
        sed -i '/^attributes #[0-9]* = { }$/d' build/out.ll
        llc --mtriple=wasm64-wasi -filetype=obj build/out.ll
        wasm-ld --no-entry --export-all --allow-undefined -mwasm64 \
          --initial-memory=268435456 -o build/out.wasm build/out.o

        # Run JS integration tests
        deno run --allow-read test-js/run.ts
        cd ..
      '';

      installPhase = "touch $out";

      meta.description = "mojo-gui JS integration tests (30 suites, ~3,375 tests via Deno)";
    };

    # ── 5. XR web runtime JS tests ───────────────────────────────────
    #
    # 4 JS test suites (414 tests) for the WebXR browser renderer.
    # Tests cover: XR types/config, panel management/raycasting/layout,
    # input handler (hover/click/focus), and runtime state machine.
    # Runs via Deno with linkedom for headless DOM — no WebXR, GPU,
    # or WASM needed.
    #
    # The XR runtime imports the shared Interpreter and TemplateCache
    # from web/runtime/, so monoSrc is used (not just xr/web/).
    mojo-gui-test-xr-js = pkgs.stdenv.mkDerivation {
      name = "check-mojo-gui-test-xr-js";
      src = monoSrc;

      nativeBuildInputs = [pkgs.deno];

      buildPhase = ''
        export HOME=$TMPDIR

        # Point Deno at the pre-fetched dependency cache.
        # Use a writable copy since Deno may write metadata files.
        cp -r ${denoXrDeps} $TMPDIR/deno-cache
        chmod -R u+w $TMPDIR/deno-cache
        export DENO_DIR=$TMPDIR/deno-cache

        cd mojo-gui/xr/web

        # Run XR web runtime JS tests
        deno run --allow-read --allow-env test-js/run.ts
      '';

      installPhase = "touch $out";

      meta.description = "mojo-gui XR web runtime JS tests (4 suites, 414 tests via Deno)";
    };

    # ── 6. Build all examples for all targets ─────────────────────────
    #
    # Verifies that all 4 shared examples (Counter, Todo, Benchmark,
    # MultiView) compile for web (WASM), desktop (native), and XR (native).
    # Does NOT build Rust shims (those have their own checks above).
    mojo-gui-build-all = pkgs.stdenv.mkDerivation {
      name = "check-mojo-gui-build-all";
      src = monoSrc;

      nativeBuildInputs = with pkgs; [
        mojo
        llvmPackages_latest.llvm # llc
        llvmPackages_latest.lld # wasm-ld
        wabt # wasm-objdump etc.
        gnused # sed -i
      ];

      buildInputs = mojoLinkInputs;

      buildPhase = ''
        export HOME=$TMPDIR
        cd mojo-gui

        echo "==> Building WASM (web target)..."
        mkdir -p web/build
        mojo build -Werror --emit llvm \
          -I core/src -I examples \
          -o web/build/out.ll web/src/main.mojo
        sed -i '/call void @llvm\.lifetime\.\(start\|end\)/d' web/build/out.ll
        sed -i 's/ nocreateundeforpoison//g' web/build/out.ll
        sed -i 's/ "target-cpu"="[^"]*"//g; s/ "target-features"="[^"]*"//g' web/build/out.ll
        sed -i '/^attributes #[0-9]* = { }$/d' web/build/out.ll
        llc --mtriple=wasm64-wasi -filetype=obj web/build/out.ll
        wasm-ld --no-entry --export-all --allow-undefined -mwasm64 \
          --initial-memory=268435456 \
          -o web/build/out.wasm web/build/out.o
        echo "  ✅ web/build/out.wasm"

        echo "==> Building desktop examples (native target)..."
        mkdir -p build
        for app in counter todo bench app; do
          mojo build examples/$app/main.mojo \
            -I core/src -I desktop/src -I xr/native/src -I examples \
            -o build/$app-desktop
          echo "  ✅ build/$app-desktop"
        done

        echo "==> Building XR examples (native + -D MOJO_TARGET_XR)..."
        for app in counter todo bench app; do
          mojo build examples/$app/main.mojo \
            -D MOJO_TARGET_XR \
            -I core/src -I xr/native/src -I desktop/src -I examples \
            -o build/$app-xr
          echo "  ✅ build/$app-xr"
        done

        echo "==> All builds passed ✅"
      '';

      installPhase = "touch $out";

      meta.description = "mojo-gui build verification — all 4 examples × {web, desktop, xr}";
    };
  };
}
