_: prev: {
  non-spatial-input = prev.inputs.non-spatial-input.packages.${prev.system}.default.overrideAttrs (_: {
    #cargoHash = prev.lib.fakeHash;
    #cargoLock = "${prev.inputs.non-spatial-input}/cargo.lock";

    src = prev.inputs.non-spatial-input.outPath; # Use the full flake input source
    cargoLock = {
      lockFile = "${prev.inputs.non-spatial-input.outPath}/Cargo. lock";
    };
  });
}
