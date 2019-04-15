{ pkgs ? import <nixos-unstable> { } }:

let
  lpkgs = import ./pkgs.nix { inherit pkgs; };
in rec {
  env = pkgs.mkShell { name = "html2json-env";
    inputsFrom = [ lpkgs.html2json ];
    buildInputs = [
      pkgs.cargo-edit
      pkgs.cargo-fuzz
      pkgs.cargo-tree
      pkgs.rustfmt
    ];
  };
  env-built = pkgs.mkShell { name = "html2json-env-built";
    buildInputs = [ lpkgs.html2json ];
  };
  env-fallback = pkgs.mkShell { name = "html2json-env-fallback";
    buildInputs = [ pkgs.cacert pkgs.cargo pkgs.git pkgs.rustc ] ++
      env.buildInputs;
  };
  env-stable = pkgs.mkShell { name = "html2json-env-stable";
    buildInputs = [ pkgs.nur.repos.mozilla.rustChannels.stable.rust ];
  };
}
