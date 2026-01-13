final: prev: {
  ragenix = prev.inputs.ragenix.packages.${prev.system}.default.override {
    plugins = [final.age-plugin-fido2-hmac];
  };
}
