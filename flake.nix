{
  description = "Nearlane: private Quick Share interoperability, Rust application with Qt and Slint frontends";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/d4f25a3e562008648d45bcf2d70bcc7422cad84e";
  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      each = nixpkgs.lib.genAttrs systems;
      nativeLibraries = pkgs: pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
        pkgs.dbus pkgs.fontconfig pkgs.libxkbcommon pkgs.wayland pkgs.libglvnd
        pkgs.libx11 pkgs.libxcursor pkgs.libxi pkgs.libxrandr
      ];
      packageFor = system: frontend: let
        pkgs = import nixpkgs { inherit system; };
        isQt = frontend == "qt";
        executable = if isQt then "nearlane-desktop" else "nearlane-slint";
        qtEnv = pkgs.qt6.env "nearlane-qt" [ pkgs.qt6.qtdeclarative pkgs.qt6.qtsvg ];
      in pkgs.rustPlatform.buildRustPackage {
        pname = if isQt then "nearlane" else "nearlane-slint";
        version = "0.1.0";
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            let name = baseNameOf path; in
            !(builtins.elem name [ "work" "target" ".git" "__pycache__" ".qmlls.ini" ])
            && pkgs.lib.cleanSourceFilter path type;
        };
        cargoLock.lockFile = ./Cargo.lock;
        cargoBuildFlags = [ "-p" executable "-p" "nearlane-cli" ];
        cargoTestFlags = [ "--workspace" "--exclude" "nearlane-desktop" "--exclude" "nearlane-slint" ];
        nativeBuildInputs = [ pkgs.pkg-config pkgs.lld pkgs.makeWrapper ]
          ++ pkgs.lib.optionals isQt [ pkgs.qt6.qttools pkgs.qt6.wrapQtAppsHook ]
          ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.autoPatchelfHook ];
        buildInputs = [ pkgs.stdenv.cc.cc.lib ] ++ nativeLibraries pkgs
          ++ pkgs.lib.optionals isQt [ pkgs.qt6.qtbase pkgs.qt6.qtdeclarative pkgs.qt6.qtsvg ]
          ++ pkgs.lib.optionals (isQt && pkgs.stdenv.hostPlatform.isLinux) [ pkgs.kdePackages.kirigami ];
        env = pkgs.lib.optionalAttrs isQt { QMAKE = "${qtEnv}/bin/qmake"; };
        preBuild = pkgs.lib.optionalString isQt ''export QMAKE="${qtEnv}/bin/qmake"'';
        qtWrapperArgs = pkgs.lib.optionals isQt [ "--set" "NEARLANE_QT_TRANSLATIONS" "${pkgs.qt6.qttranslations}/translations" ];
        # Kirigami's selector reads QML_IMPORT_PATH before Qt's patched imports.
        preFixup = pkgs.lib.optionalString (isQt && pkgs.stdenv.hostPlatform.isLinux) ''
          qtWrapperArgs+=(--prefix QML_IMPORT_PATH : "${pkgs.kdePackages.kirigami.unwrapped}/${pkgs.qt6.qtbase.qtQmlPrefix}")
        '';
        postInstall = ''
          install -Dm644 tools/import_google_oauth.py "$out/libexec/nearlane/import-google-oauth.py"
          makeWrapper "${pkgs.python3}/bin/python3" "$out/bin/nearlane-import-google-oauth" \
            --add-flags "$out/libexec/nearlane/import-google-oauth.py"
          install -Dm644 packaging/org.nearlane.Nearlane.desktop "$out/share/applications/org.nearlane.Nearlane.desktop"
          substituteInPlace "$out/share/applications/org.nearlane.Nearlane.desktop" \
            --replace-fail 'Exec=nearlane-desktop' 'Exec=${executable}'
          install -Dm644 packaging/icons/org.nearlane.Nearlane.svg "$out/share/icons/hicolor/scalable/apps/org.nearlane.Nearlane.svg"
          install -Dm644 LICENSE-MIT "$out/share/licenses/nearlane/LICENSE-MIT"
          install -Dm644 LICENSE-APACHE "$out/share/licenses/nearlane/LICENSE-APACHE"
          install -Dm644 NOTICE "$out/share/licenses/nearlane/NOTICE"
          install -Dm644 third_party/beto/LICENSE "$out/share/licenses/nearlane/beto-LICENSE"
          install -Dm644 third_party/beto/README.md "$out/share/licenses/nearlane/beto-PROVENANCE.md"
        '' + pkgs.lib.optionalString (!isQt) ''
          install -Dm644 third_party/slint-material/LICENSE.md "$out/share/licenses/nearlane/slint-material-MIT.md"
          install -Dm644 third_party/slint-material/LICENSE-Slint-Royalty-free-2.0.md "$out/share/licenses/nearlane/slint-Royalty-free-2.0.md"
          install -Dm644 third_party/slint-material/PROVENANCE.md "$out/share/licenses/nearlane/slint-material-PROVENANCE.md"
        '';
        postFixup = pkgs.lib.optionalString (!isQt && pkgs.stdenv.hostPlatform.isLinux) ''
          wrapProgram "$out/bin/${executable}" --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath (nativeLibraries pkgs)}"
        '' + pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isDarwin ''
          app="$out/Applications/Nearlane.app/Contents"
          mkdir -p "$app/MacOS"
          install -m644 packaging/macos/Info.plist "$app/Info.plist"
          for language in en ru; do
            install -Dm644 "packaging/macos/$language.lproj/InfoPlist.strings" "$app/Resources/$language.lproj/InfoPlist.strings"
          done
          if [ -f "$out/bin/.${executable}-wrapped" ]; then
            cp "$out/bin/.${executable}-wrapped" "$app/MacOS/Nearlane"
          else
            cp "$out/bin/${executable}" "$app/MacOS/Nearlane"
          fi
        '' + pkgs.lib.optionalString (isQt && pkgs.stdenv.hostPlatform.isDarwin) ''
          wrapQtApp "$app/MacOS/Nearlane"
        '';
        meta = {
          description = "Independent Quick Share client research prototype (${frontend})";
          license = (with pkgs.lib.licenses; [ mit asl20 ]) ++ pkgs.lib.optionals (!isQt) [{
            shortName = "Slint-Royalty-free-2.0";
            fullName = "Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0";
            url = "https://github.com/slint-ui/slint/blob/v1.19.0/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md";
            free = false;
            redistributable = true;
          }];
          mainProgram = executable;
        };
      };
      shellFor = system: frontend: let
        pkgs = import nixpkgs { inherit system; };
        isQt = frontend == "qt";
        qtEnv = pkgs.qt6.env "nearlane-qt" [ pkgs.qt6.qtdeclarative pkgs.qt6.qtsvg ];
        libraries = [ pkgs.stdenv.cc.cc.lib ] ++ nativeLibraries pkgs ++ pkgs.lib.optionals isQt [ qtEnv ];
      in pkgs.mkShell {
        packages = with pkgs; [ cargo rustc rustfmt clippy pkg-config cmake ninja lld ]
          ++ nativeLibraries pkgs
          ++ lib.optionals isQt [ qt6.qtbase qt6.qtdeclarative qt6.qttools qt6.wrapQtAppsHook ]
          ++ lib.optionals (isQt && stdenv.hostPlatform.isLinux) [ kdePackages.kirigami ];
        env = pkgs.lib.optionalAttrs isQt {
          QMAKE = "${qtEnv}/bin/qmake";
          NEARLANE_QT_TRANSLATIONS = "${pkgs.qt6.qttranslations}/translations";
        };
        shellHook = ''
          export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath libraries}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          export DYLD_FALLBACK_LIBRARY_PATH="${pkgs.lib.makeLibraryPath libraries}''${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}"
          export CARGO_HOME="$PWD/work/cargo-home"
          export XDG_CACHE_HOME="$PWD/work/cache"
        '' + pkgs.lib.optionalString isQt ''
          export QMAKE="${qtEnv}/bin/qmake"
          export QT_PLUGIN_PATH="${pkgs.qt6.qtbase}/${pkgs.qt6.qtbase.qtPluginPrefix}''${QT_PLUGIN_PATH:+:$QT_PLUGIN_PATH}"
          export QML_IMPORT_PATH="${pkgs.qt6.qtdeclarative}/${pkgs.qt6.qtbase.qtQmlPrefix}''${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
        '' + pkgs.lib.optionalString (isQt && pkgs.stdenv.hostPlatform.isLinux) ''
          export QML_IMPORT_PATH="${pkgs.kdePackages.kirigami.unwrapped}/${pkgs.qt6.qtbase.qtQmlPrefix}:$QML_IMPORT_PATH"
        '';
      };
    in {
      packages = each (system: rec { default = qt; qt = packageFor system "qt"; slint = packageFor system "slint"; });
      devShells = each (system: { default = shellFor system "qt"; slint = shellFor system "slint"; });
    };
}
