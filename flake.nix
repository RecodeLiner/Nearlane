{
  description = "Nearlane: private Quick Share interoperability with a Rust and Slint desktop client";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/d4f25a3e562008648d45bcf2d70bcc7422cad84e";
  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      each = nixpkgs.lib.genAttrs systems;
      nativeLibraries = pkgs: pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
        pkgs.dbus pkgs.fontconfig pkgs.libxkbcommon pkgs.wayland pkgs.libglvnd
        pkgs.libx11 pkgs.libxcursor pkgs.libxi pkgs.libxrandr
      ];
      packageFor = system: let
        pkgs = import nixpkgs { inherit system; };
      in pkgs.rustPlatform.buildRustPackage {
        pname = "nearlane-slint";
        version = "0.1.0";
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            let name = baseNameOf path; in
            !(builtins.elem name [ "work" "target" ".git" "__pycache__" ])
            && pkgs.lib.cleanSourceFilter path type;
        };
        cargoLock.lockFile = ./Cargo.lock;
        cargoBuildFlags = [ "-p" "nearlane-slint" "-p" "nearlane-cli" ];
        cargoTestFlags = [ "--workspace" "--exclude" "nearlane-slint" ];
        nativeBuildInputs = [ pkgs.pkg-config pkgs.lld pkgs.makeWrapper ]
          ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.autoPatchelfHook ];
        buildInputs = [ pkgs.stdenv.cc.cc.lib ] ++ nativeLibraries pkgs;
        postInstall = ''
          ln -s nearlane-slint "$out/bin/nearlane-desktop"
          install -Dm644 tools/import_google_oauth.py "$out/libexec/nearlane/import-google-oauth.py"
          makeWrapper "${pkgs.python3}/bin/python3" "$out/bin/nearlane-import-google-oauth" \
            --add-flags "$out/libexec/nearlane/import-google-oauth.py"
          install -Dm644 packaging/org.nearlane.Nearlane.desktop "$out/share/applications/org.nearlane.Nearlane.desktop"
          install -Dm644 packaging/icons/org.nearlane.Nearlane.svg "$out/share/icons/hicolor/scalable/apps/org.nearlane.Nearlane.svg"
          install -Dm644 LICENSE-MIT "$out/share/licenses/nearlane/LICENSE-MIT"
          install -Dm644 LICENSE-APACHE "$out/share/licenses/nearlane/LICENSE-APACHE"
          install -Dm644 NOTICE "$out/share/licenses/nearlane/NOTICE"
          install -Dm644 third_party/beto/LICENSE "$out/share/licenses/nearlane/beto-LICENSE"
          install -Dm644 third_party/beto/README.md "$out/share/licenses/nearlane/beto-PROVENANCE.md"
          install -Dm644 third_party/slint-material/LICENSE.md "$out/share/licenses/nearlane/slint-material-MIT.md"
          install -Dm644 third_party/slint-material/LICENSE-Slint-Royalty-free-2.0.md "$out/share/licenses/nearlane/slint-Royalty-free-2.0.md"
          install -Dm644 third_party/slint-material/PROVENANCE.md "$out/share/licenses/nearlane/slint-material-PROVENANCE.md"
        '';
        postFixup = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
          wrapProgram "$out/bin/nearlane-slint" --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath (nativeLibraries pkgs)}"
        '' + pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isDarwin ''
          app="$out/Applications/Nearlane.app/Contents"
          mkdir -p "$app/MacOS"
          install -m644 packaging/macos/Info.plist "$app/Info.plist"
          for language in en ru; do
            install -Dm644 "packaging/macos/$language.lproj/InfoPlist.strings" "$app/Resources/$language.lproj/InfoPlist.strings"
          done
          cp "$out/bin/nearlane-slint" "$app/MacOS/Nearlane"
        '';
        meta = {
          description = "Independent Quick Share client research prototype";
          license = (with pkgs.lib.licenses; [ mit asl20 ]) ++ [{
            shortName = "Slint-Royalty-free-2.0";
            fullName = "Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0";
            url = "https://github.com/slint-ui/slint/blob/v1.19.0/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md";
            free = false;
            redistributable = true;
          }];
          mainProgram = "nearlane-slint";
        };
      };
      shellFor = system: let
        pkgs = import nixpkgs { inherit system; };
        libraries = [ pkgs.stdenv.cc.cc.lib ] ++ nativeLibraries pkgs;
      in pkgs.mkShell {
        packages = with pkgs; [ cargo rustc rustfmt clippy pkg-config cmake ninja lld ]
          ++ nativeLibraries pkgs;
        shellHook = ''
          export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath libraries}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          export DYLD_FALLBACK_LIBRARY_PATH="${pkgs.lib.makeLibraryPath libraries}''${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}"
          export CARGO_HOME="$PWD/work/cargo-home"
          export XDG_CACHE_HOME="$PWD/work/cache"
        '';
      };
    in {
      packages = each (system: rec { default = slint; slint = packageFor system; });
      devShells = each (system: rec { default = slint; slint = shellFor system; });
    };
}
