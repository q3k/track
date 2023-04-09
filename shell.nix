with import <nixpkgs> {}; with pkgs;
let
  x11deps = with xorg; [libX11 libXcursor libXrandr libXi];
  in mkShell {
    buildInputs = [
      cargo rustc
      cmake pkg-config fontconfig
      alsa-lib
    ] ++ x11deps;
    shellHook = ''
      export LD_LIBRARY_PATH="${lib.makeLibraryPath ([ libGL ] ++ x11deps)}:$LD_LBIRARY_PATH"
      export RUST_SRC_PATH="${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    '';
  }
