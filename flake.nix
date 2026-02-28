{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rust-toolchain = pkgs.rust-bin.stable.latest.default.override { extensions = [ "rust-src" ]; };

        commonBuildInputs = with pkgs; [
          openssl
          zlib
          wayland
          wayland-protocols
          libxkbcommon
          vulkan-loader
          libx11
          libxcursor
          libxrandr
          libxi
          libxcb

          # Applet
          gtk3
          xdotool
          libappindicator-gtk3
        ];

        mkPackage =
          {
            pname,
            bin,
            description,
            features ? [ ],
            extraBinPath ? [ ],
          }:
          pkgs.rustPlatform.buildRustPackage {
            inherit pname;
            version = "0.1.0";

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            nativeBuildInputs = with pkgs; [
              pkg-config
              rust-toolchain
              makeWrapper
            ];

            buildInputs = commonBuildInputs;

            nativeCheckInputs = with pkgs; [
              exiftool
            ];

            cargoBuildFlags = [
              "--bin"
              bin
            ]
            ++ (pkgs.lib.optionals (features != [ ]) ([ "--features" ] ++ features));

            postInstall = ''
              wrapProgram $out/bin/${bin} \
                  --prefix PATH : "${pkgs.lib.makeBinPath extraBinPath}" \
                  --set LD_LIBRARY_PATH "${builtins.toString (pkgs.lib.makeLibraryPath commonBuildInputs)}";
            '';

            PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";

            meta = with pkgs.lib; {
              inherit description;
              license = licenses.mit;
              maintainers = [ painerp ];
            };
          };

        apod-wallpaper = mkPackage {
          pname = "apod-wallpaper";
          bin = "apod-wallpaper";
          description = "Tool to update wallpapers to newest apod";
          features = [ "cli" ];
          extraBinPath = [ pkgs.exiftool ];
        };

        apod-wallpaper-switcher = mkPackage {
          pname = "apod-wallpaper-switcher";
          bin = "apod-wallpaper-switcher";
          description = "GUI wallpaper switcher for apod";
          features = [ "gui" ];
        };

        apod-wallpaper-applet = mkPackage {
          pname = "apod-wallpaper-applet";
          bin = "apod-wallpaper-applet";
          description = "Applet for apod wallpaper switcher";
          features = [ "applet" ];
          extraBinPath = with pkgs; [
            gtk3
            xdotool
            libappindicator-gtk3
          ];
        };

        apod-wallpaper-all = pkgs.buildEnv {
          name = "apod-wallpaper-all";
          paths = [
            apod-wallpaper
            apod-wallpaper-switcher
          ];
        };
      in
      {
        packages = {
          default = apod-wallpaper-all;
          apod-wallpaper = apod-wallpaper;
          apod-wallpaper-switcher = apod-wallpaper-switcher;
          apod-wallpaper-applet = apod-wallpaper-applet;
          apod-wallpaper-all = apod-wallpaper-all;
        };

        devShells.default = pkgs.mkShell rec {
          buildInputs =
            commonBuildInputs
            ++ (with pkgs; [
              pkg-config
              autoconf
              libtool
              automake
              clippy
              exiftool
              mesa
              cargo-bloat
            ])
            ++ [ rust-toolchain ];

          RUST_SRC_PATH = "${rust-toolchain}/lib/rustlib/src/rust";
          shellHook = ''
            export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${builtins.toString (pkgs.lib.makeLibraryPath buildInputs)}";
          '';
        };
      }
    );
}
