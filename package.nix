{
  lib,
  stdenv,
  rustPlatform,
  pkg-config,
  lld,
  makeWrapper,
  autoPatchelfHook,
  dbus,
  fontconfig,
  libxkbcommon,
  wayland,
  libglvnd,
  libx11,
  libxcursor,
  libxi,
  libxrandr,
  python3,
}:
let
  runtimeLibraries = lib.optionals stdenv.hostPlatform.isLinux [
    dbus fontconfig libxkbcommon wayland libglvnd libx11 libxcursor libxi libxrandr
  ];
in
rustPlatform.buildRustPackage {
  pname = "nearlane-slint";
  version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
  src = lib.cleanSourceWith {
    src = ./.;
    filter = path: type:
      let name = baseNameOf path; in
      !(builtins.elem name [ "work" "target" ".git" "__pycache__" ])
      && lib.cleanSourceFilter path type;
  };
  cargoLock.lockFile = ./Cargo.lock;
  cargoBuildFlags = [ "-p" "nearlane-slint" "-p" "nearlane-cli" ];
  cargoTestFlags = [ "--workspace" "--exclude" "nearlane-slint" ];
  nativeBuildInputs = [ pkg-config lld makeWrapper ]
    ++ lib.optionals stdenv.hostPlatform.isLinux [ autoPatchelfHook ];
  buildInputs = [ stdenv.cc.cc.lib ] ++ runtimeLibraries;
  passthru.runtimeLibraries = runtimeLibraries;
  postInstall = ''
    ln -s nearlane-slint "$out/bin/nearlane-desktop"
    install -Dm644 tools/import_google_oauth.py "$out/libexec/nearlane/import-google-oauth.py"
    makeWrapper "${python3}/bin/python3" "$out/bin/nearlane-import-google-oauth" \
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
  postFixup = lib.optionalString stdenv.hostPlatform.isLinux ''
    wrapProgram "$out/bin/nearlane-slint" --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath runtimeLibraries}"
  '' + lib.optionalString stdenv.hostPlatform.isDarwin ''
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
    license = (with lib.licenses; [ mit asl20 ]) ++ [{
      shortName = "Slint-Royalty-free-2.0";
      fullName = "Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0";
      url = "https://github.com/slint-ui/slint/blob/v1.19.0/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md";
      free = false;
      redistributable = true;
    }];
    mainProgram = "nearlane-slint";
  };
}
