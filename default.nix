{ pkgs ? import <nixpkgs> { }, system ? pkgs.system or null }: let
  lockData = builtins.fromJSON (builtins.readFile ./flake.lock);
  sourceInfo = lockData.nodes.std.locked;
  src = fetchTarball {
    url = "https://github.com/${sourceInfo.owner}/${sourceInfo.repo}/archive/${sourceInfo.rev}.tar.gz";
    sha256 = sourceInfo.narHash;
  };
  inherit (import src) Flake Set;
  inputs = Flake.Lock.Node.inputs (Flake.Lock.root (Flake.Lock.New (lockData // {
    override.sources = if pkgs ? path then {
      nixpkgs = pkgs.path;
    } else { };
  })));
  outputs = Flake.CallDir ./. inputs;
  systemAttrNames = [
    "packages" "legacyPackages" "devShells" "apps" "checks"
  ];
  systemAttrs = Set.map (_: Set.get system) (Set.retain systemAttrNames outputs) // {
    pkgs = inputs.nixpkgs.legacyPackages.${system};
  };
  systemOutputs = Set.optional (system != null) systemAttrs;
in systemOutputs.packages.default or { } // systemOutputs // {
  inherit inputs outputs;
  lib = outputs.lib or { };
}
