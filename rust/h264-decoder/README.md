# h264-decoder-rs

A pure Rust H.264 (AVC) decoder library and CLI tool.

Parses H.264 Annex B bytestreams using the [`h264-reader`](https://crates.io/crates/h264-reader) crate and decodes frames into raw YUV/RGB output, suitable for use by downstream video encoders such as [rav1e](https://github.com/xiph/rav1e) (AV1) or display pipelines.

## Project structure

```text
h264-decoder-rs/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── h264-decode/            # Core decoder library
│   │   └── src/
│   │       ├── lib.rs          # Public API surface
│   │       ├── decode.rs       # Decoder + DecoderConfig
│   │       ├── dpb.rs          # Decoded Picture Buffer (reference management + reordering)
│   │       ├── error.rs        # Error and warning types
│   │       ├── frame.rs        # DecodedFrame, FramePlane, PictureType
│   │       ├── nal_handler.rs  # NAL unit parsing bridge (h264-reader integration)
│   │       ├── pixel.rs        # Pixel formats and YUV↔RGB conversion
│   │       └── transform.rs    # Inverse transforms (4×4, 8×8 IDCT, Hadamard) and dequantization
│   └── h264-decode-cli/        # CLI binary
│       └── src/
│           └── main.rs
├── default.nix                 # Nix package and devShell
├── justfile                    # Development commands
└── README.md
```

## Library usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
h264-decode = { path = "crates/h264-decode" }
```

### Decode to YUV 4:2:0 (native output)

```rust
use h264_decode::{Decoder, DecoderConfig, PixelFormat};

let config = DecoderConfig::new().pixel_format(PixelFormat::Yuv420p);
let mut decoder = Decoder::new(config);

// Feed raw H.264 Annex B bytestream data (can be called incrementally)
let h264_data: &[u8] = &[ /* ... */ ];
let frames = decoder.decode(h264_data).expect("decode failed");

for frame in &frames {
    // Access individual YUV planes
    let y = frame.y_plane().data();
    let u = frame.u_plane().data();
    let v = frame.v_plane().data();

    println!(
        "decoded {}x{} frame, poc={}, type={:?}",
        frame.width(),
        frame.height(),
        frame.pic_order_cnt(),
        frame.picture_type(),
    );
}

// Flush remaining frames at end of stream
let trailing = decoder.flush().expect("flush failed");
```

### Decode to RGB24 (automatic conversion)

```rust
use h264_decode::{Decoder, DecoderConfig, PixelFormat};

let config = DecoderConfig::new()
    .pixel_format(PixelFormat::Rgb24)
    .colour_matrix(h264_decode::pixel::ColourMatrix::Bt709);

let mut decoder = Decoder::new(config);
let frames = decoder.decode(h264_data).unwrap();

for frame in &frames {
    // Single packed RGB plane: R, G, B, R, G, B, ...
    let rgb_data = frame.plane(0).data();
    assert_eq!(rgb_data.len(), (frame.width() * frame.height() * 3) as usize);
}
```

### Convert between formats on demand

```rust
use h264_decode::{Decoder, DecoderConfig, PixelFormat};

// Decode as native YUV 4:2:0
let mut decoder = Decoder::with_defaults();
let frames = decoder.decode(h264_data).unwrap();

// Convert individual frames as needed
for frame in &frames {
    let rgb_frame = frame.to_rgb24();
    let rgba_frame = frame.to_rgba32();
    let nv12_frame = frame.to_nv12();
}
```

### Feeding a downstream encoder (e.g. rav1e)

The decoder's native YUV 4:2:0 planar output maps directly to what video encoders expect:

```rust
use h264_decode::{Decoder, DecoderConfig, PixelFormat};

let mut decoder = Decoder::new(DecoderConfig::new().pixel_format(PixelFormat::Yuv420p));
let frames = decoder.decode(h264_data).unwrap();

for frame in &frames {
    let y_plane = frame.y_plane();
    let u_plane = frame.u_plane();
    let v_plane = frame.v_plane();

    // Feed planes to rav1e, x264, or any other encoder:
    //   encoder.send_frame(y_plane.data(), u_plane.data(), v_plane.data(),
    //                      frame.width(), frame.height());
}
```

## Supported output pixel formats

| Format | Description | Planes | Use case |
|--------|-------------|--------|----------|
| `Yuv420p` | YUV 4:2:0 planar (I420) | 3 (Y, U, V) | Video encoders (rav1e, x264), default |
| `Yuv422p` | YUV 4:2:2 planar | 3 (Y, U, V) | Professional video |
| `Yuv444p` | YUV 4:4:4 planar | 3 (Y, U, V) | Lossless workflows |
| `Nv12` | NV12 semi-planar | 2 (Y, UV interleaved) | Hardware APIs (VA-API, DXVA) |
| `Rgb24` | Packed 24-bit RGB | 1 | Image processing, display |
| `Rgba32` | Packed 32-bit RGBA | 1 | Compositing, UI rendering |

## CLI usage

```bash
# Build and install
cargo build --release -p h264-decode-cli

# Decode H.264 to raw YUV 4:2:0
h264-decode input.264 -o output.yuv

# Decode to RGB24
h264-decode input.264 -o output.rgb -f rgb24

# Decode to NV12 with BT.709 matrix
h264-decode input.264 -o output.nv12 -f nv12 -m bt709

# Print stream info with verbose logging
h264-decode input.264 -v

# Decode only the first 10 frames
h264-decode input.264 -o output.yuv --max-frames 10

# Write raw frames to stdout (for piping)
h264-decode input.264 -o - | ffplay -f rawvideo -pix_fmt yuv420p -s 1920x1080 -
```

## Architecture

### Decoder pipeline

```text
Annex B bytestream
        │
        ▼
┌───────────────────┐
│  Start-code scan  │  Split bytestream on 00 00 01 / 00 00 00 01
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  NAL unit parse   │  Classify NAL type, remove emulation prevention bytes
│  (h264-reader)    │  Parse SPS, PPS, slice headers
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  Entropy decode   │  CAVLC / CABAC → transform coefficients
│                   │  (Exp-Golomb reader for headers)
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  Dequantize +     │  Level-scale matrices, 4×4 / 8×8 inverse integer DCT,
│  Inverse transform│  Hadamard for DC coefficients
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  Prediction +     │  Intra prediction / motion-compensated inter prediction
│  Reconstruction   │  Add residual, clip to pixel range
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  Deblocking filter│  In-loop filter to reduce blocking artifacts
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  DPB management   │  Reference frame storage, sliding-window / MMCO marking,
│                   │  picture reordering by POC for display output
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│  Format convert   │  Optional YUV→RGB / NV12 conversion
│  (pixel.rs)       │  BT.601 or BT.709 colour matrix
└───────┬───────────┘
        │
        ▼
    DecodedFrame
```

### Key components

- **`Decoder`** — Top-level entry point. Accepts incremental Annex B data and produces decoded frames in display order.
- **`DecoderConfig`** — Builder for configuring output format, colour matrix, deblocking, and reference frame limits.
- **`DecodedFrame`** — Decoded picture with per-plane accessors, metadata (POC, picture type, frame number), and format conversion methods.
- **`Dpb`** — Decoded Picture Buffer implementing H.264 reference frame management (sliding window, MMCO, short/long-term marking) and display reordering by picture order count.
- **`transform`** — Normative 4×4 and 8×8 inverse integer DCT, Hadamard transforms for DC coefficients, dequantization with level-scale matrices, and zig-zag scan orders.
- **`pixel`** — Pixel format definitions and BT.601/BT.709 YUV↔RGB conversion routines.
- **`nal_handler`** — NAL unit classification, SPS/PPS storage, slice header parsing with an exp-Golomb reader, and event-driven accumulator for the decoder.

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

# Open documentation
just doc
```

### Nix

```bash
# Enter the devShell
direnv allow  # or: nix develop .#h264-decoder-rs

# Build the package
nix build .#h264-decoder-rs
```

## Current status

The project provides a complete, compiling decoder scaffold with:

- ✅ Annex B bytestream parsing and start-code scanning
- ✅ NAL unit classification and emulation prevention byte removal
- ✅ SPS parsing (profile, level, dimensions, chroma format, POC type, reference frame limits)
- ✅ PPS parsing (bootstrap)
- ✅ Slice header parsing (slice type, frame_num, PPS reference)
- ✅ Picture order count computation (types 0 and 2)
- ✅ Decoded Picture Buffer with full reference management (sliding window, MMCO, short/long-term marking)
- ✅ Reference picture list construction (list 0 and list 1)
- ✅ Display reordering by POC
- ✅ 4×4 and 8×8 inverse integer DCT transforms
- ✅ 4×4 and 2×2 inverse Hadamard transforms for DC coefficients
- ✅ Dequantization with H.264 level-scale matrices
- ✅ Zig-zag scan orders (4×4 and 8×8)
- ✅ Residual reconstruction with clipping
- ✅ YUV 4:2:0 → RGB24 / RGBA32 / NV12 conversion (BT.601 and BT.709)
- ✅ Full `DecodedFrame` type with per-plane access and format conversion
- ✅ CLI tool with clap argument parsing
- ✅ Comprehensive test suite

### Planned work

- 🔲 Full CAVLC entropy decoding of macroblock residual data
- 🔲 CABAC entropy decoding
- 🔲 Intra prediction modes (4×4, 8×8, 16×16 luma + chroma)
- 🔲 Motion-compensated inter prediction (P and B slices)
- 🔲 Sub-pixel interpolation (quarter-pel luma, eighth-pel chroma)
- 🔲 In-loop deblocking filter
- 🔲 Weighted prediction
- 🔲 High profile 8×8 transform support
- 🔲 Multiple slice groups
- 🔲 Field/MBAFF coding
- 🔲 10-bit and higher bit-depth support

## License

MIT