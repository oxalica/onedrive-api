with import <nixpkgs> {};
pkgs.mkShell {
  buildInputs = [
    pkgconfig
    openssl
  ];
}
