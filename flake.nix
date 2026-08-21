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
        pkgs: rec {
          bgsvg = pkgs.rustPlatform.buildRustPackage {
            pname = "bgsvg";
            version = "0.1.0";
            # only what the build reads: the corpus and the docs stay out of the
            # store path, so editing a golden cannot trigger a rebuild
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./build.rs
                ./parameters.proto
                ./askama.toml
                ./src
                ./templates
                ./tests
                ./crates/bgsvg-wasm/Cargo.toml
                ./crates/bgsvg-wasm/src
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            # prost-build shells out to protoc; PROTOC is how it finds it
            nativeBuildInputs = [ pkgs.protobuf ];
            PROTOC = "${pkgs.protobuf}/bin/protoc";
          };
          bgsvg-wasm = pkgs.rustPlatform.buildRustPackage {
            pname = "bgsvg-wasm";
            version = "0.1.0";
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./build.rs
                ./parameters.proto
                ./askama.toml
                ./src
                ./templates
                ./tests
                ./crates/bgsvg-wasm/Cargo.toml
                ./crates/bgsvg-wasm/src
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            # protoc for prost-build; lld is the wasm32-unknown-unknown linker --
            # nixpkgs' rustc carries the target's std but not a bundled rust-lld
            nativeBuildInputs = [ pkgs.protobuf pkgs.wasm-bindgen-cli pkgs.lld ];
            PROTOC = "${pkgs.protobuf}/bin/protoc";
            buildPhase = ''
              cargo build --release --target wasm32-unknown-unknown -p bgsvg-wasm
            '';
            # two targets from one .wasm: `web` is what a bundler consumes,
            # `nodejs` is what test/wasm.mjs runs. The glue differs; the
            # rendered bytes cannot.
            installPhase = ''
              for t in web nodejs; do
                wasm-bindgen target/wasm32-unknown-unknown/release/bgsvg_wasm.wasm \
                  --out-dir $out/$t --target $t
              done
            '';
            # the workspace's tests run natively via `cargo test`, not here
            doCheck = false;
          };
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
          # python3 is here for test/golden.py and nothing else
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rustfmt
            pkgs.clippy
            pkgs.protobuf
            pkgs.python3
            # the wasm32-unknown-unknown linker -- nixpkgs' rustc carries the
            # target's std but not a bundled rust-lld
            pkgs.lld
            pkgs.wasm-bindgen-cli
            # nodejs is here to run test/wasm.mjs and nothing else, the same way
            # python3 is here only for test/golden.py
            pkgs.nodejs
          ];
          PROTOC = "${pkgs.protobuf}/bin/protoc";
        };
      });
    };
}
