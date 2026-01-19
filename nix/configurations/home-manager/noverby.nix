{
  inputs,
  src,
  ...
}: {
  system = "x86_64-linux";
  extraSpecialArgs = {
    inherit inputs src;
    stateVersion = "24.05";
  };
  modules = [inputs.self.homeModules.noverby];
}
