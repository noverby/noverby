---
title: Organize Nix
author: Niclas Overby
---

Nix: What is it?
---

* Goal:
  * Reproducible: Same inputs always produce the same outputs
  * Declarative: Describe what you want, not how to build it
  * Reliable: Multiple versions of packages and rollbacks
* Nix: The package manager:
  * *CppNix*: Official implementation in C++ started in 2003
  * Snix: Rewrite in Rust
  * Determinate Systems' Nix: Accelerate development (Performance and enterprise)
  * Lix: Community fork (Controversial sponsors, project leadership)
* Nix: The language:
  * Functional programming language
  * Dynamic typed
  * Lazy evaluated

<!-- end_slide -->

Nix: The Ecosystem
---

* [Nixpkgs](https://search.nixos.org/packages): +120'000 packages
  * Unstable: Rolling release
  * Stable: 24.11, 25.05, 25.11 (7-month support)
  * [Ctrl-OS](https://cyberus-technology.de/en/ctrlos): 5-year long-term support
* System configuration
  * [NixOS](https://search.nixos.org/options): Linux distro with 20'000+ options
  * [Home-manager](https://home-manager-options.extranix.com): Config management with 4'600 options
  * [Nix-darwin](https://github.com/nix-darwin/nix-darwin): Mac config management
* Cloud orchestration:
  * Colmena
  * Much more!

<!-- end_slide -->

Nix Language: List
---

```nix
# List (Whitespace separated)
[1 2 3]
[
  "hello"
  "world"
]

# List Parentheses
[ (1 + 2) (3 + 4) ] == [ 3 7 ]

```

<!-- end_slide -->

Nix Language: Attribute Set
---

```nix
# Attribute set (Record, Dictionary)
# Convertible to JSON, YAML, TOML, etc.
{
  attribute_1 = 1;
  attribute-2 = 2;
  "attribute=3" = 3;
}

# Nested attribute sets
{
  a.b.c = "value";
}
# Same as:
{
  a = {
    b = {
      c = "value";
    };
  };
}

# Inherit
{ x = x; y = y; } == { inherit x y; }

```

<!-- end_slide -->

Nix Language: Constructs
---

```nix
# Let in
let
  x = "hello ";
  y = "world";
in
  x + y

# Import file
import ./file.nix { arg1 = "hello"; arg2 = 2; }

# Import directory (Will import ./dir/default.nix)
import ./dir
```

<!-- end_slide -->

Nix Language: Functions
---

```nix
# Function
x: x * 2

# Function with multiple arguments
x: y: x * y

# Access elements of nixpkgs attribute set
pkgs: [ pkgs.cmake pkgs.gcc ]

# Access elements of attribute set using with
pkgs: with pkgs; [ cmake gcc ]

# Destructuring
{cmake, gcc, ... }: [ cmake gcc ]

```

<!-- end_slide -->

Flake: Intro
---

* Standardizing Nix project management
* Introduced in 2021 (Still experimental)
* Adds `flake.nix` project file
* Define inputs: Dependencies from other flakes
* Define outputs: Packages, shells, configurations
* Reproducible with lock file

```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixpkgs-unstable";
  };

  # outputs.packages.x86_64-linux.hello = pkgs.hello;
  outputs = inputs: let
      pkgs = import inputs.nixpkgs { system = "x86_64-linux"; };
    in {
      packages.x86_64-linux.hello = pkgs.hello;
    };
}
```

<!-- end_slide -->

Flake: Schema
---

```nix
{
  # Nix package
  packages."<system>"."<name>"

  # Nix development shell
  devShells."<system>"."<name>"

  # Complete NixOS system configurations
  nixosConfigurations."<hostname>" = {};

  # Module for a specific NixOS feature,
  # that can be used to build a complete NixOS system
  nixosModules."<name>"

  # There additional `official` schema attributesets,
  # that this presentation does not cover
  ...
}
```

<!-- end_slide -->

Flake: Systems Abstraction
---

```nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  # outputs.packages.x86_64-linux.hello = pkgs.hello;
  # outputs.packages.aarch64-linux.hello = pkgs.hello;
  outputs = { nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux"];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          hello = pkgs.hello;
        }
      );
    };
}
```

<!-- end_slide -->

Flakelight: Introduction
---

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flakelight.url = "github:nix-community/flakelight";
  };
  outputs = inputs:
    inputs.flakelight ./. {
      inherit inputs;

      systems = [ "x86_64-linux" "aarch64-linux"];

      packages.hello = pkgs: pkgs.hello;

      devShells.hello-shell = pkgs: pkgs.mkShell {
        packages = [ pkgs.hello ];
      };
    };
}
```

<!-- end_slide -->

Flakelight: Access Flake Packages
---

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flakelight.url = "github:nix-community/flakelight";
  };
  outputs = inputs:
    inputs.flakelight ./. {
      inherit inputs;

      # hello2 is added to pkgs
      packages.hello2 = pkgs: pkgs.hello;

      devShells.hello-shell = pkgs: pkgs.mkShell {
        # You can access hello2 like this now
        packages = [ pkgs.hello2 ];
      };
    };
}
```

<!-- end_slide -->

Flakelight: Imports
---

<!-- column_layout: [1, 1] -->

<!-- column: 0 -->

```nix
# flake.nix
{
  inputs = {
    # ...
  };
  outputs = inputs:
    inputs.flakelight ./. {
      inherit inputs;

      imports = [
        ./hello2
      ];
    };
}
```

<!-- column: 1 -->

```nix
# hello2/default.nix
{
  # Nested imports
  imports = [ ./devshell.nix ];

  packages.hello2 = pkgs: pkgs.hello;
}
```

```nix
# hello2/devshell.nix
{
  devShells.hello-shell = pkgs: pkgs.mkShell {
    packages = [ pkgs.hello2 ];
  };
}
```

<!-- end_slide -->

Flakelight: Nix Directory
---

<!-- column_layout: [1, 1] -->

<!-- column: 0 -->

```nix
# flake.nix
{
  inputs = {
    # ...
  };
  outputs = inputs:
    inputs.flakelight ./. {
      inherit inputs;

      # Already implicit set to ./nix
      nixDir = ./nix;
    };
}
```

<!-- column: 1 -->

```nix
# nix/packages/hello2.nix
pkgs: pkgs.hello
```

```nix
# nix/devShells/hello-shell.nix
pkgs: pkgs.mkShell {
  packages = [ pkgs.hello2 ];
}
```

<!-- end_slide -->

Flakelight: Alternatives
---

<!-- column_layout: [1, 1] -->

<!-- column: 0 -->

* Flakelight:
  * Abstracts over systems
  * Make packages available
  * Structure with imports/nixDir
* Flake-parts
  * perSystem
  * Does not make packages available
* Build abstractions ourselves
  * Fragmentation
  * Exposing hard to read Nix code
  * Create our own shared Flake library?

<!-- column: 1 -->

```nix
{
  outputs = inputs:
    inputs.flakelight ./. {
      perSystem = pkgs: {
        packages.hello2 = pkgs.hello;
        devShells.hello-shell = pkgs.mkShell {
          # Flake-parts cannot access hello2
          # without additional configuration
          packages = [ pkgs.hello2 ]; 
        };
      };
    };
}
```
