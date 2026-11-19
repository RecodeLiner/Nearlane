{
  description = "Nearlane: private Quick Share interoperability with a Rust and Slint desktop client";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/d4f25a3e562008648d45bcf2d70bcc7422cad84e";
  outputs = { nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      each = nixpkgs.lib.genAttrs systems;
      pkgsFor = system: import nixpkgs {
        inherit system;
        config.allowUnfreePredicate = pkg: nixpkgs.lib.getName pkg == "nearlane-slint";
      };
      packageFor = system: (pkgsFor system).callPackage ./package.nix { };
      shellFor = system: let
        pkgs = pkgsFor system;
        inherit (packageFor system) runtimeLibraries;
        libraries = [ pkgs.stdenv.cc.cc.lib ] ++ runtimeLibraries;
      in pkgs.mkShell {
        packages = with pkgs; [ cargo rustc rustfmt clippy pkg-config cmake ninja lld ]
          ++ runtimeLibraries;
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
