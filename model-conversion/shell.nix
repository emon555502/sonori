{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "whisper-model-conversion";

  # Build inputs for the shell environment
  buildInputs = with pkgs; [
    # Python and required packages
    (python3.withPackages (ps: with ps; [
      ctranslate2
      huggingface-hub
      torch
      transformers
    ]))
  ];

  # Shell hook to provide instructions and set up the environment
  shellHook = ''
    echo "Whisper model conversion environment ready!"
  '';
}
