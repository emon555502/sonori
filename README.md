# Sonori

A lightweight, transparent overlay application that displays real-time transcriptions of your speech using Whisper AI models on Linux.

The application is currently in very early development and might be unstable, buggy and/or crash.

Contributions are welcome. There are no guidelines yet. Just check the planned features, known issues and make sure your changes work on NixOS and other distros!

## Features

### Current

- **Real-Time Transcription**: Transcribes your speech in real-time using OpenAI's Whisper models
- **Voice Activity Detection**: Uses Silero VAD for accurate speech detection
- **Transparent Overlay**: Non-intrusive overlay that sits at the bottom of your screen
- **Audio Visualization**: Visual feedback when speaking with a spectrogram display
- **Copy/Paste Functionality**: Easily copy transcribed text to clipboard
- **Auto-Start Recording**: Begins recording as soon as the application launches
- **Scroll Controls**: Navigate through longer transcripts
- **Configurable**: Configure the model, language, and other settings in the config file (config.json)
- **Automatic Model Download**: Both Whisper and Silero VAD models are downloaded automatically

### Planned

- **Fix keyboard shortcuts**: Currently, the application does not respond to any keyboard shortcuts
- **Better error handling**: Handle errors gracefully and provide useful error messages
- **Improve performance**: Lower CPU usage, lower latency, better multi-threaded code
- **Better UI**: A better UI with a focus on more usability
- **VSYNC**: Add VSYNC support for optionally reducing rendered frames
- **Input field detection**: Automatically detect input fields and transcribe text into them (might be a bit tricky to implement)
- **CUDA support**: Add support for CUDA to speed up inference on supported GPUs
- **Other backends**: I want to add other optional backends like Whisper.cpp or even an API (which would greatly increase speed/accuracy at the cost of some latency and maybe your privacy)

### NOT Planned

- **Using a GUI framework**: I want to learn more about wgpu and wgsl and think a GUI written from scratch is perfectly fine for this application
- **Support for Windows/macOS**: Not planned by me personally but if anyone wants to give it a shot feel free

## Requirements

### Dependencies

DISCLAIMER: Building from source, installing dependencies and running the application has only been tested on NixOS and I'm unsure if it will work on other distributions.

For Debian/Ubuntu-based distributions:

```bash
sudo apt install build-essential portaudio19-dev libclang-dev pkg-config wl-copy \
  libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev \
  libasound2-dev libssl-dev libfftw3-dev curl cmake
```

For Fedora/RHEL-based distributions:

```bash
sudo dnf install gcc gcc-c++ portaudio-devel clang-devel pkg-config wl-copy \
  libxkbcommon-devel wayland-devel libX11-devel libXcursor-devel libXi-devel libXrandr-devel \
  alsa-lib-devel openssl-devel fftw-devel curl cmake
```

For Arch-based distributions:

```bash
sudo pacman -S base-devel portaudio clang pkgconf wl-copy \
  libxkbcommon wayland libx11 libxcursor libxi libxrandr alsa-lib openssl fftw curl cmake
```

For NixOS:

Simply use the provided flake.nix by running

```bash
nix develop
```

while in the root directory of the repository

### Required Models

Sonori needs two types of models to function properly:

1. **Whisper Model** - Configured in the `config.json` file and downloaded automatically on first run
2. **Silero VAD Model** - Also downloaded automatically on first run

   Note: If you need to download the Silero model manually for any reason, you should head to the repo and download the model yourself:

   https://github.com/snakers4/silero-vad/

   And then place it in `~/.cache/sonori/models/`

### Additional Requirements

- **ONNX Runtime**: Required for the Silero VAD model.
- **CTranslate2**: Used for Whisper model inference.

## Installation

### Building from Source

1. Install Rust and Cargo (https://rustup.rs/) and make sure the dependencies are installed
2. Clone this repository
3. Build the application:
   ```bash
   cargo build --release
   ```
4. The executable will be in `target/release/sonori`

## Usage

1. Launch the application:
   ```bash
   ./target/release/sonori
   ```
2. A transparent overlay will appear at the bottom of your screen
3. Recording starts automatically
4. Speak naturally - your speech will be transcribed in real-time or near real-time (based on the model and hardware)
5. Use the buttons on the overlay to:
   - Copy text to clipboard
   - Clear transcript history
   - Exit the application

## Configuration

Sonori uses a `config.json` file in the same directory as the executable. If not present, a default configuration is used.

Example configuration:

```json
{
  "model": "openai/whisper-base.en",
  "language": "en",
  "compute_type": "INT8",
  "log_stats_enabled": false,
  "buffer_size": 1024,
  "sample_rate": 16000,
  "whisper_options": {
    "beam_size": 5,
    "patience": 1.0,
    "repetition_penalty": 1.25
  },
  "vad_config": {
    "threshold": 0.2,
    "hangbefore_frames": 1,
    "hangover_frames": 15,
    "max_buffer_duration_sec": 30.0,
    "max_segment_count": 20
  },
  "audio_processor_config": {
    "max_vis_samples": 1024
  }
}
```

### Model Options

Recommended Local Whisper models:

- `openai/whisper-tiny.en` - Tiny model, English only (for low-end CPUs)
- `openai/whisper-base.en` - Base model, English only (default, for low to mid-range CPUs)
- `distil-whisper/distil-small.en` - Small model, English only (for mid to high-range CPUs)
- `distil-whisper/distil-medium.en` - Medium model, English only (for high-end CPUs only)
- any other bigger whisper model - probably too slow to run on CPU only in real-time

For non-English languages, use the multilingual models (without `.en` suffix) and set the appropriate language code in the configuration.

## Known Issues

- The application might not work with all Wayland compositors (I only tested it with KDE Plasma and KWin).
- The transcriptions are not 100% accurate and might contain errors. This is closely related to the whisper model that is used.
- Sometimes the last word of a "segment" is cut off. This is probably an issue with processing the audio data.
- The CPU usage is too high, even when idle. This might be related to bad code on my side or some overhead of the models. I already identified that changing the buffer size will help (or make it worse).

## Troubleshooting

### Wayland Support

Sonori uses layer shell protocol for Wayland compositors. If you experience issues:

- Make sure you are in a wayland session and your compositor supports the layer shell protocol

### Model Conversion Issues

If you encounter issues with automatic model conversion:

For NixOS:

```bash
nix-shell model-conversion/shell.nix
ct2-transformers-converter --model your-model --output_dir ~/.cache/whisper/your-model --copy_files preprocessor_config.json tokenizer.json
```

For other distributions:

```bash
pip install -U ctranslate2 huggingface_hub torch transformers
ct2-transformers-converter --model your-model --output_dir ~/.cache/whisper/your-model --copy_files preprocessor_config.json tokenizer.json
```

## Platform Support

- **Linux**: Supported (tested on Wayland using KDE Plasma and KWin)
- **Windows/macOS**: Not officially supported or tested

## Credits

- [Rust](https://www.rust-lang.org/)
- [CTranslate2](https://github.com/OpenNMT/CTranslate2) and [Faster Whisper](https://github.com/SYSTRAN/faster-whisper)
- [Onnx Runtime](https://github.com/microsoft/onnxruntime)
- [OpenAI Whisper](https://github.com/openai/whisper)
- [Silero VAD](https://github.com/snakers4/silero-vad)
- [Winit Fork](https://github.com/SergioRibera/winit)
- [WGPU](https://github.com/gfx-rs/wgpu)

## License

[MIT](LICENSE)
