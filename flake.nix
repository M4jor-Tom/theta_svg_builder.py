{
  description = "svg_builder — animated SVG background generator (mood + resolution matching)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (
        pkgs:
        let
          bgsvg = pkgs.writeShellApplication {
            name = "bgsvg";
            runtimeInputs = [ pkgs.python3 ];
            text = ''exec python3 ${./background.py} "$@"'';
          };
        in
        {
          inherit bgsvg;
          default = bgsvg;
        }
      );

      apps = forAll (pkgs: rec {
        bgsvg = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.bgsvg}/bin/bgsvg";
        };
        default = bgsvg;
      });

      devShells = forAll (pkgs: {
        default = pkgs.mkShell { packages = [ pkgs.python3 ]; };
      });
    };
}
