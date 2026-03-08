# h264toav1-rs

A CLI tool for transcoding H.264 (AVC) video to AV1, built entirely in Rust.

Uses [`h264-decode`](../h264-decoder-rs) for H.264 decoding and [rav1e](https://github.com/xiph/rav1e) for AV1 encoding. Output is written as an [IVF](https://wiki.multimedia.cx/index.php/IVF) container.

## Project structure

```text
h264toav1-rs/
├── Cargo.toml          # Package manifest
├── src/
│   ├── main.rs         # CLI entry point, decode/encode pipeline
│   └── ivf.rs          # IVF container writer
├── default.nix         # Nix package and devShell
├── justfile            # Development commands
└── README.md
```

## Usage

```bash
# Build
cargo build --release

# Basic transcode (H.264 Annex B → AV1 IVF)
h264toav1 input.264 -o output.ivf

# Specify speed preset and quantizer
h264toav1 input.264 -o output.ivf -s 4 -q 80

# Fast preview encode
h264toav1 input.264 -o output.ivf -s 10 -q 128

# High quality encode
h264toav1 input.264 -o output.ivf -s 3 -q 60

# Low-latency mode (no frame reordering)
h264toav1 input.264 -o output.ivf --low-latency

# Limit to first 100 frames with verbose logging
h264toav1 input.264 -o output.ivf --max-frames 100 -v

# Write to stdout (e.g. pipe to ffplay)
h264toav1 input.264 -o - | ffplay -i -
```

## CLI options

```text
h264toav1 [OPTIONS] <INPUT> -o <OUTPUT>

Arguments:
  <INPUT>                  Path to the input H.264 Annex B file (.264 / .h264)

Options:
  -o, --output <OUTPUT>    Output AV1 file path (.ivf), or "-" for stdout
  -s, --speed <SPEED>      rav1e speed preset (0=slowest/best .. 10=fastest) [default: 6]
  -q, --quantizer <QP>     Quantizer (0=lossless .. 255=worst) [default: 100]
      --threads <N>        Number of encoding threads (0 = auto) [default: 0]
      --fps <FPS>          Frame rate to assume for the output [default: 30]
      --keyint <N>         Maximum keyframe interval [default: 240]
      --low-latency        Enable low-latency mode (single-pass, no reordering)
      --max-frames <N>     Maximum number of frames to transcode (0 = unlimited) [default: 0]
  -v, --verbose            Enable verbose logging
  -h, --help               Print help
  -V, --version            Print version
```

## Pipeline

```text
H.264 Annex B file
        │
        ▼
┌───────────────────┐
│  h264-decode      │  Pure Rust H.264 decoder
│  (YUV 4:2:0 out)  │  Parses NAL units, decodes frames
└───────┬───────────┘
        │  DecodedFrame (Y, U, V planes)
        ▼
┌───────────────────┐
│  rav1e            │  AV1 encoder
│  (AV1 packets)    │  Encodes YUV frames to AV1
└───────┬───────────┘
        │  Encoded packets
        ▼
┌───────────────────┐
│  IVF writer       │  Minimal container format
│  (.ivf output)    │  32-byte header + per-frame headers
└───────────────────┘
```

## Playing the output

The IVF container is widely supported:

```bash
# Play with ffplay
ffplay output.ivf

# Play with mpv
mpv output.ivf

# Remux to MP4 with ffmpeg
ffmpeg -i output.ivf -c copy output.mp4

# Remux to WebM
ffmpeg -i output.ivf -c copy output.webm
```

## Speed vs quality tradeoffs

| Preset | Speed | Quality | Use case |
|--------|-------|---------|----------|
| 0 | Slowest | Best | Archival, final encode |
| 3 | Slow | Very good | High-quality distribution |
| 6 | Medium | Good | General purpose (default) |
| 8 | Fast | Fair | Quick preview |
| 10 | Fastest | Lower | Real-time / testing |

Lower quantizer values produce higher quality (and larger files). A quantizer of 0 is lossless.

## Development

```bash
# Run all checks
just all

# Build
just build

# Run tests
just test

# Run clippy
just clippy

# Format code
just fmt

# Quick transcode
just transcode input.264

# High-quality transcode
just transcode-hq input.264 output.ivf
```

### Nix

```bash
# Enter the devShell
direnv allow  # or: nix develop .#h264toav1-rs

# Build the package
nix build .#h264toav1-rs
```

## License

MIT