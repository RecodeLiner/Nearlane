{
  description = "Nearlane: private Quick Share interoperability research, Rust + Qt";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/d4f25a3e562008648d45bcf2d70bcc7422cad84e";
  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      each = nixpkgs.lib.genAttrs systems;
      packageFor = system: let
        pkgs = import nixpkgs { inherit system; };
        qtEnv = pkgs.qt6.env "nearlane-qt" [ pkgs.qt6.qtdeclarative ];
      in
        pkgs.rustPlatform.buildRustPackage {
          pname = "nearlane";
          version = "0.1.0";
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              let name = baseNameOf path; in
              !(builtins.elem name [ "work" "target" ".git" "__pycache__" ".qmlls.ini" ])
              && pkgs.lib.cleanSourceFilter path type;
          };
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "nearlane-desktop" "-p" "nearlane-cli" ];
          cargoTestFlags = [ "--workspace" "--exclude" "nearlane-desktop" ];
          nativeBuildInputs = [ pkgs.pkg-config pkgs.lld pkgs.qt6.wrapQtAppsHook ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.autoPatchelfHook ];
          buildInputs = [ pkgs.qt6.qtbase pkgs.qt6.qtdeclarative pkgs.stdenv.cc.cc.lib ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.kdePackages.kirigami ];
          env.QMAKE = "${qtEnv}/bin/qmake";
          preBuild = ''export QMAKE="${qtEnv}/bin/qmake"'';
          # Kirigami's StyleSelector reads QML_IMPORT_PATH itself, rather than
          # Nix's patched engine import paths. Give it the complete module before
          # the session's partial /run/current-system/sw tree (often styles only).
          preFixup = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            qtWrapperArgs+=(--prefix QML_IMPORT_PATH : "${pkgs.kdePackages.kirigami.unwrapped}/${pkgs.qt6.qtbase.qtQmlPrefix}")
          '';
          postInstall = ''
            install -Dm644 packaging/org.nearlane.Nearlane.desktop "$out/share/applications/org.nearlane.Nearlane.desktop"
            install -Dm644 LICENSE-MIT "$out/share/licenses/nearlane/LICENSE-MIT"
            install -Dm644 LICENSE-APACHE "$out/share/licenses/nearlane/LICENSE-APACHE"
            install -Dm644 NOTICE "$out/share/licenses/nearlane/NOTICE"
            install -Dm644 third_party/beto/LICENSE "$out/share/licenses/nearlane/beto-LICENSE"
            install -Dm644 third_party/beto/README.md "$out/share/licenses/nearlane/beto-PROVENANCE.md"
          '';
          meta = {
            description = "Independent Quick Share client research prototype";
            license = with pkgs.lib.licenses; [ mit asl20 ];
            mainProgram = "nearlane-desktop";
          };
        };
    in {
      packages = each (system: { default = packageFor system; });
      devShells = each (system: let
        pkgs = import nixpkgs { inherit system; };
        qtEnv = pkgs.qt6.env "nearlane-qt" [ pkgs.qt6.qtdeclarative ];
      in {
        default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt clippy pkg-config cmake ninja lld qt6.qtbase qt6.qtdeclarative qt6.wrapQtAppsHook ]
            ++ lib.optionals stdenv.isLinux [ kdePackages.kirigami ];
          env.QMAKE = "${qtEnv}/bin/qmake";
          shellHook = ''
            export QMAKE="${qtEnv}/bin/qmake"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ qtEnv pkgs.stdenv.cc.cc.lib ]}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
            export DYLD_FALLBACK_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ qtEnv pkgs.stdenv.cc.cc.lib ]}''${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}"
            export CARGO_HOME="$PWD/work/cargo-home"
            export XDG_CACHE_HOME="$PWD/work/cache"
            export QT_PLUGIN_PATH="${pkgs.qt6.qtbase}/${pkgs.qt6.qtbase.qtPluginPrefix}''${QT_PLUGIN_PATH:+:$QT_PLUGIN_PATH}"
            export QML_IMPORT_PATH="${pkgs.qt6.qtdeclarative}/${pkgs.qt6.qtbase.qtQmlPrefix}''${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
          '' + pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            export QML_IMPORT_PATH="${pkgs.kdePackages.kirigami.unwrapped}/${pkgs.qt6.qtbase.qtQmlPrefix}:$QML_IMPORT_PATH"
          '';
        };
      });
    };
}
