{inputs, ...}: {
  system = "x86_64-linux";
  extraSpecialArgs = {
    inherit inputs;
    stateVersion = "24.05";
  };
  modules = [inputs.self.homeModules.noverby];
}
