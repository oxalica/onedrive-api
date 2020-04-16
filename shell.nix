with import <nixpkgs> {};
pkgs.mkShell {
  buildInputs = [
    pkg-config
    openssl
  ];
}
