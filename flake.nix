{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        toolchain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
        packages = with pkgs; [
          cargo
          cargo-tauri
          toolchain
          nodejs_22
          pnpm
        ];

        nativeBuildPackages = with pkgs; [
          pkg-config
          dbus
          openssl
          glib
          gtk3
          libsoup_3
          webkitgtk_4_1
          librsvg
        ];

        libraries = with pkgs; [
          webkitgtk_4_1
          gtk3
          cairo
          gdk-pixbuf
          glib
          dbus
          openssl
          librsvg
        ];

      in {
        devShells.default = pkgs.mkShell {
          buildInputs = packages;
          nativeBuildInputs = nativeBuildPackages;
          shellHook = with pkgs; ''
            export LD_LIBRARY_PATH="${
              lib.makeLibraryPath libraries
            }:$LD_LIBRARY_PATH"
            export OPENSSL_INCLUDE_DIR="${openssl.dev}/include/openssl"
            export OPENSSL_LIB_DIR="${openssl.out}/lib"
            export OPENSSL_ROOT_DIR="${openssl.out}"
            export RUST_SRC_PATH="${toolchain}/lib/rustlib/src/rust/library"
          '';
        };
      });
}
