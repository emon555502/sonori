{
  description = "Rust development environment";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    # Rust
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        lib = pkgs.lib;
        toolchain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          extensions = [ "rust-src" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          name = "rust-dev";
          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.clang
            pkgs.cmake
            # Mold Linker for faster builds (only on Linux)
            (lib.optionals pkgs.stdenv.isLinux pkgs.mold)
          ];
          buildInputs = [
            pkgs.libxkbcommon
            pkgs.libxkbcommon.dev
            pkgs.wayland
            pkgs.wayland.dev
            pkgs.xorg.libX11.dev
            pkgs.xorg.libX11
            pkgs.xorg.libXcursor
            pkgs.xorg.libXi
            pkgs.xorg.libXrandr
            pkgs.libiconv
            pkgs.openssl.dev
            pkgs.alsa-lib
            pkgs.portaudio
            pkgs.fftw
            pkgs.curl
            pkgs.ctranslate2
            pkgs.rust-analyzer-unwrapped
            pkgs.wtype
            pkgs.onnxruntime
            pkgs.vulkan-loader
            toolchain
          ];
          packages = [ ];

          # Environment variables
          RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
          LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.libxkbcommon pkgs.libxkbcommon.dev pkgs.wayland pkgs.wayland.dev pkgs.xorg.libX11 pkgs.xorg.libX11.dev pkgs.xorg.libXcursor pkgs.xorg.libXi pkgs.xorg.libXrandr pkgs.libiconv pkgs.openssl.dev pkgs.vulkan-loader ];
          OPENSSL_STATIC = "0";
          OPENSSL_DIR = pkgs.openssl.dev;
          OPENSSL_INCLUDE_DIR = (
            lib.makeSearchPathOutput "dev" "include" [ pkgs.openssl.dev ]
          ) + "/openssl";
          ORT_DYLIB_PATH = "${pkgs.onnxruntime}/lib/libonnxruntime.so";
        };
      });
}
