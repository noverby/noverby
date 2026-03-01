{lib, ...}: {
  _module.args = {
    lib = lib.extend (_: _: {
      inherit (builtins) toJSON fromJSON toFile toString readDir filterSource;
    });
  };
}
