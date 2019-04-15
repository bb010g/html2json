{ pkgs ? import <nixpkgs> { } }:

rec {
  # development

  ## development.tools

  html2json = pkgs.callPackage ./. { };
}
