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
          # only what the build reads. `./tests` now carries the golden corpus,
          # because `cargo test` verifies it and `nix build` runs `cargo test`
          # in a sandbox -- so a golden edit does change this store path, and
          # must: the check depends on those bytes. The docs stay out.
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
        in
        rec {
          bgsvg = pkgs.rustPlatform.buildRustPackage {
            pname = "bgsvg";
            version = "0.1.0";
            inherit src;
            cargoLock.lockFile = ./Cargo.lock;
            # prost-build shells out to protoc; PROTOC is how it finds it
            nativeBuildInputs = [ pkgs.protobuf ];
            PROTOC = "${pkgs.protobuf}/bin/protoc";
          };
          bgsvg-wasm = pkgs.rustPlatform.buildRustPackage {
            pname = "bgsvg-wasm";
            version = "0.1.0";
            inherit src;
            cargoLock.lockFile = ./Cargo.lock;
            # protoc for prost-build; lld is the wasm32-unknown-unknown linker --
            # nixpkgs' rustc carries the target's std but not a bundled rust-lld.
            # wasm-bindgen-cli's schema version must match Cargo.lock's
            # wasm-bindgen/js-sys exactly, so a nixpkgs.url bump can break this
            # build with a "schema version" mismatch -- repair it with
            # `cargo update -p wasm-bindgen --precise <version>` (and js-sys,
            # which pins wasm-bindgen exactly) to match the new CLI's version.
            nativeBuildInputs = [ pkgs.protobuf pkgs.wasm-bindgen-cli pkgs.lld ];
            PROTOC = "${pkgs.protobuf}/bin/protoc";
            buildPhase = ''
              cargo build --release --target wasm32-unknown-unknown -p bgsvg-wasm
            '';
            # two targets from one .wasm: `web` is what a bundler consumes,
            # `nodejs` is what tests/wasm.mjs runs. The glue differs; the
            # rendered bytes cannot.
            installPhase = ''
              for t in web nodejs; do
                wasm-bindgen target/wasm32-unknown-unknown/release/bgsvg_wasm.wasm \
                  --out-dir $out/$t --target $t
              done
            '';
            # the workspace's tests run natively via `cargo test --workspace`, not here
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
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rustfmt
            pkgs.clippy
            pkgs.protobuf
            # the wasm32-unknown-unknown linker -- nixpkgs' rustc carries the
            # target's std but not a bundled rust-lld
            pkgs.lld
            pkgs.wasm-bindgen-cli
            # nodejs is here to run tests/wasm.mjs and nothing else -- `cargo
            # test` shells out to it through tests/wasm.rs. It is the last
            # non-Rust runtime in this shell: wasm-bindgen's glue is JavaScript
            # by construction, so driving the browser build from Rust would mean
            # reimplementing that glue, not deleting it.
            pkgs.nodejs
          ];
          PROTOC = "${pkgs.protobuf}/bin/protoc";
        };
      });
    };
}
