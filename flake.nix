{
  description = "svg_builder — animated SVG background generator (mood + resolution matching)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      pythonFor = pkgs: pkgs.python3.withPackages (ps: [ ps.protobuf ]);
    in
    {
      packages = forAll (
        pkgs:
        let
          # the generated module must sit beside background.py, so ship a source
          # directory rather than the single-file store path
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [ ./background.py ./parameters_pb2.py ];
          };
          bgsvg = pkgs.writeShellApplication {
            name = "bgsvg";
            runtimeInputs = [ (pythonFor pkgs) ];
            text = ''exec python3 ${src}/background.py "$@"'';
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
        default = pkgs.mkShell {
          # protoc regenerates parameters_pb2.py:
          #   protoc --python_out=. parameters.proto
          packages = [ (pythonFor pkgs) pkgs.protobuf ];
        };
      });
    };
}
