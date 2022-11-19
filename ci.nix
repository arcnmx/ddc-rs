{ config, channels, pkgs, lib, ... }: with pkgs; with lib; let
  inherit (import ./. { inherit pkgs; }) checks;
in {
  name = "ddc-rs";
  ci.gh-actions.enable = true;
  cache.cachix.arc.enable = true;
  channels = {
    nixpkgs = "22.11";
  };
  tasks = {
    build.inputs = singleton checks.test;
  };
  jobs = {
    nixos = {
    };
    macos.system = "x86_64-darwin";
  };
}
