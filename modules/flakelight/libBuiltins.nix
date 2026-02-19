{lib, ...}: {
  _module.args = {
    lib = lib.extend (final: prev: {
      inherit (builtins) toJSON fromJSON toFile toString readDir filterSource;
    });
  };
}
