{
  lib,
  stdenv,
  fetchurl,
  autoPatchelfHook,
  unzip,
  zstd,
  libedit,
  zlib,
  curl,
  libbsd,
}:
stdenv.mkDerivation rec {
  pname = "mojo";
  version = "25.6.1";

  srcs = [
    (fetchurl {
      url = "https://conda.modular.com/max/linux-64/mojo-compiler-0.${version}-release.conda";
      sha256 = "sha256-npniXlmGOT3quMvT/25kQMXOouNqBXn5H/2YKl258+s=";
    })
    (fetchurl {
      url = "https://conda.modular.com/max/linux-64/mojo-0.${version}-release.conda";
      sha256 = "sha256-eGEVy+r7VSbxlARB/rWkzdFe6FtGBybun0iHB0uGldI=";
    })
    # Using nixpkgs ncurses, mojo fails with error:
    # version `NCURSES6_5.0.19991023' not found (required by <NIX-STORE-PATH>/lib/liblldb20.0.0git.so)
    # So let's use the ncurses from Conda
    (fetchurl {
      url = "https://conda.anaconda.org/conda-forge/linux-64/ncurses-6.5-h2d0b736_3.conda";
      sha256 = "sha256-P94pMjL6P8qYY14RZ95rfH/ag8ryS51skeye77T01YY=";
    })
  ];

  sourceRoot = ".";
  preferLocalBuild = true;

  nativeBuildInputs = [
    autoPatchelfHook
    unzip
    zstd
  ];

  buildInputs = [
    stdenv.cc.cc.lib
    libedit
    zlib
    curl
    libbsd
  ];

  unpackPhase = ''
    for src in $srcs; do
      unzip -o $src
      tar --zstd -xf pkg-*.tar.zst
      rm pkg-*.tar.zst
    done
  '';

  installPhase = ''
    mkdir -p $out
    cp -r lib/ $out/lib/
    cp -r bin/ $out/bin/
    cp -r share/ $out/share

    ln -s ${libedit}/lib/libedit.so.0 $out/lib/libedit.so.2

    # /etc/modular/modular.cfg contains hardcoded paths to libs
    mkdir -p $out/etc/modular
    cat > $out/etc/modular/modular.cfg << EOF
    [max]
    cache_dir = $out/share/max/.max_cache
    driver_lib = $out/lib/libDeviceDriver.so
    enable_compile_progress = true
    enable_model_ir_cache = true
    engine_lib = $out/lib/libmodular-framework-common.so
    graph_lib = $out/lib/libmof.so
    name = MAX Platform
    path = $out
    serve_lib = $out/lib/libServeRTCAPI.so
    torch_ext_lib = $out/lib/libmodular-framework-torch-ext.so
    version = ${version}

    [mojo-max]
    compilerrt_path = $out/lib/libKGENCompilerRTShared.so
    mgprt_path = $out/lib/libMGPRT.so
    atenrt_path = $out/lib/libATenRT.so
    shared_libs = $out/lib/libAsyncRTMojoBindings.so,$out/lib/libAsyncRTRuntimeGlobals.so,$out/lib/libMSupportGlobals.so,-Xlinker,-rpath,-Xlinker,$out/lib
    driver_path = $out/bin/mojo
    import_path = $out/lib/mojo
    jupyter_path = $out/lib/libMojoJupyter.so
    lldb_path = $out/bin/mojo-lldb
    lldb_plugin_path = $out/lib/libMojoLLDB.so
    lldb_visualizers_path = $out/lib/lldb-visualizers
    lldb_vscode_path = $out/bin/mojo-lldb-dap
    lsp_server_path = $out/bin/mojo-lsp-server
    mblack_path = $out/bin/mblack
    orcrt_path = $out/lib/liborc_rt.a
    repl_entry_point = $out/lib/mojo-repl-entry-point
    system_libs = -lrt,-ldl,-lpthread,-lm,-lz,-ltinfo
    test_executor_path = $out/lib/mojo-test-executor
    EOF

    # Create mojo wrapper that uses generated modular.cfg
    mkdir -p $out/bin
    mv $out/bin/mojo $out/bin/mojo-unwrapped
    cat > $out/bin/mojo << EOF
    #!${stdenv.shell}
    export MODULAR_HOME=$out/etc/modular
    export TERMINFO_DIRS=$out/share/terminfo
    exec $out/bin/mojo-unwrapped "\$@"
    EOF
    chmod +x $out/bin/mojo

    # Create mojo-lldb wrapper that uses generated modular.cfg
    mkdir -p $out/bin
    mv $out/bin/mojo-lldb $out/bin/mojo-lldb-unwrapped
    cat > $out/bin/mojo-lldb << EOF
    #!${stdenv.shell}
    export MODULAR_HOME=$out/etc/modular
    export TERMINFO_DIRS=$out/share/terminfo
    exec $out/bin/mojo-lldb-unwrapped "\$@"
    EOF
    chmod +x $out/bin/mojo-lldb

    # Create mojo-lsp-server wrapper that uses generated modular.cfg
    mv $out/bin/mojo-lsp-server $out/bin/mojo-lsp-server-unwrapped
    cat > $out/bin/mojo-lsp-server << EOF
    #!${stdenv.shell}
    export MODULAR_HOME=$out/etc/modular
    exec $out/bin/mojo-lsp-server-unwrapped -I $out/lib/mojo "\$@"
    EOF
    chmod +x $out/bin/mojo-lsp-server

    # /etc/modular/crashdb needs to be mutable
    ln -s /tmp/ $out/etc/modular/crashdb
  '';

  doInstallCheck = true;
  installCheckPhase = ''
    $out/bin/mojo --version
    $out/bin/mojo-lldb --version
    $out/bin/mojo-lsp-server --version
  '';

  meta = with lib; {
    description = "Mojo Programming Language";
    homepage = "https://www.modular.com/mojo";
    platforms = platforms.linux;
    maintainers = with lib.maintainers; [noverby];
    license = licenses.unfree;
  };
}
