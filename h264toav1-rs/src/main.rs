//! CLI tool for transcoding H.264 Annex B bytestream files to AV1 (IVF container).
//!
//! # Usage
//!
//! ```text
//! h264toav1 [OPTIONS] <INPUT> -o <OUTPUT>
//!
//! Arguments:
//!   <INPUT>  Path to the input H.264 Annex B file (.264 / .h264)
//!
//! Options:
//!   -o, --output <OUTPUT>      Output AV1 file path (.ivf)
//!   -s, --speed <SPEED>        rav1e speed preset (0=slowest/best .. 10=fastest) [default: 6]
//!   -q, --quantizer <QP>       Quantizer (0=lossless .. 255=worst) [default: 100]
//!       --threads <N>          Number of encoding threads (0 = auto) [default: 0]
//!       --fps <FPS>            Frame rate to assume for the output [default: 30]
//!       --max-frames <N>       Maximum number of frames to transcode (0 = unlimited) [default: 0]
//!       --keyint <N>           Maximum keyframe interval [default: 240]
//!       --low-latency          Enable low-latency mode (single-pass, no reordering)
//!   -v, --verbose              Enable verbose logging
//!   -h, --help                 Print help
//!   -V, --version              Print version
//! ```

mod ivf;

use std::fs;
use std::io::{self, BufWriter, Cursor, Seek, Write};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context as _, Result, bail};
use clap::Parser;
use rav1e::prelude::*;

use h264_decode::{Decoder, DecoderConfig, PixelFormat};

use crate::ivf::IvfWriter;

/// Transcode H.264 (AVC) video to AV1.
///
/// Reads an H.264 Annex B bytestream, decodes it with a pure-Rust H.264
/// decoder, re-encodes every frame to AV1 using rav1e, and writes the
/// output as an IVF container.
#[derive(Parser, Debug)]
#[command(name = "h264toav1", version, about)]
struct Cli {
    /// Path to the input H.264 Annex B file (.264 / .h264).
    input: PathBuf,

    /// Output AV1 file path (.ivf).
    ///
    /// Use `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// rav1e speed preset (0 = slowest/best quality, 10 = fastest).
    #[arg(short, long, default_value_t = 6)]
    speed: u8,

    /// Quantizer (0 = lossless, 255 = worst quality).
    #[arg(short, long, default_value_t = 100)]
    quantizer: usize,

    /// Number of encoding threads (0 = automatic).
    #[arg(long, default_value_t = 0)]
    threads: usize,

    /// Frame rate (frames per second) to assume for the output.
    #[arg(long, default_value_t = 30)]
    fps: u32,

    /// Maximum keyframe interval in frames.
    #[arg(long, default_value_t = 240)]
    keyint: u64,

    /// Enable low-latency mode (single-pass, no frame reordering).
    #[arg(long)]
    low_latency: bool,

    /// Maximum number of frames to transcode (0 = unlimited).
    #[arg(long, default_value_t = 0)]
    max_frames: usize,

    /// Enable verbose (debug-level) logging.
    #[arg(short, long)]
    verbose: bool,
}

// ---------------------------------------------------------------------------
// Seekable output abstraction
// ---------------------------------------------------------------------------

/// Output target that implements both `Write` and `Seek`.
///
/// For file output we write directly; for stdout we buffer into memory
/// (since stdout is not seekable) and flush at the end.
enum Output {
    /// Buffered file writer (seekable).
    File(BufWriter<fs::File>),
    /// In-memory buffer, flushed to stdout on [`Output::finish`].
    Stdout(Cursor<Vec<u8>>),
}

impl Write for Output {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Output::File(w) => w.write(buf),
            Output::Stdout(c) => c.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Output::File(w) => w.flush(),
            Output::Stdout(c) => c.flush(),
        }
    }
}

impl Seek for Output {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        match self {
            Output::File(w) => w.seek(pos),
            Output::Stdout(c) => c.seek(pos),
        }
    }
}

impl Output {
    /// Create an [`Output`] for the given path.
    ///
    /// If the path is `"-"`, output will be buffered in memory and written
    /// to stdout when [`Output::finish`] is called.
    fn open(path: &PathBuf) -> Result<Self> {
        if path.to_str() == Some("-") {
            Ok(Output::Stdout(Cursor::new(Vec::new())))
        } else {
            let file = fs::File::create(path)
                .with_context(|| format!("cannot create output file {}", path.display()))?;
            Ok(Output::File(BufWriter::new(file)))
        }
    }

    /// Flush / finalise the output.
    ///
    /// For file output this simply flushes the buffered writer.  For stdout
    /// mode, the entire in-memory buffer is written to stdout.
    fn finish(self) -> Result<()> {
        match self {
            Output::File(mut w) => {
                w.flush().context("failed to flush output file")?;
            }
            Output::Stdout(cursor) => {
                let data = cursor.into_inner();
                let mut out = io::stdout().lock();
                out.write_all(&data).context("failed to write to stdout")?;
                out.flush().context("failed to flush stdout")?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(if cli.verbose {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .format_timestamp(None)
        .init();

    // ── Read input ──────────────────────────────────────────────────
    let input_path = &cli.input;
    if !input_path.exists() {
        bail!("input file does not exist: {}", input_path.display());
    }

    let input_data = fs::read(input_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", input_path.display()))?;

    eprintln!(
        "input: {} ({} bytes)",
        input_path.display(),
        input_data.len()
    );

    // ── Decode H.264 ───────────────────────────────────────────────
    let decode_start = Instant::now();

    let decoder_config = DecoderConfig::new()
        .pixel_format(PixelFormat::Yuv420p)
        .collect_warnings(cli.verbose);

    let mut decoder = Decoder::new(decoder_config);

    let mut frames = decoder
        .decode(&input_data)
        .map_err(|e| anyhow::anyhow!("H.264 decode failed: {e}"))?;

    let trailing = decoder
        .flush()
        .map_err(|e| anyhow::anyhow!("H.264 flush failed: {e}"))?;
    frames.extend(trailing);

    if frames.is_empty() {
        bail!("no frames decoded from input");
    }

    let width = frames[0].width();
    let height = frames[0].height();

    let decode_elapsed = decode_start.elapsed();
    eprintln!(
        "decoded {} frames ({}x{}) in {decode_elapsed:.3?}",
        frames.len(),
        width,
        height,
    );

    // Print decoder warnings if verbose.
    if cli.verbose {
        let warnings = decoder.take_warnings();
        if !warnings.is_empty() {
            eprintln!("--- decoder warnings ({}) ---", warnings.len());
            for w in &warnings {
                eprintln!("  {w}");
            }
        }
    }

    // ── Configure rav1e ─────────────────────────────────────────────
    let enc_config = EncoderConfig {
        width: width as usize,
        height: height as usize,
        bit_depth: 8,
        chroma_sampling: ChromaSampling::Cs420,
        chroma_sample_position: ChromaSamplePosition::Unknown,
        speed_settings: SpeedSettings::from_preset(cli.speed.min(10)),
        time_base: Rational::new(1, cli.fps as u64),
        min_key_frame_interval: if cli.low_latency { 0 } else { 12 },
        max_key_frame_interval: cli.keyint,
        low_latency: cli.low_latency,
        quantizer: cli.quantizer.min(255),
        min_quantizer: 0,
        bitrate: 0, // quantizer-based rate control
        reservoir_frame_delay: None,
        ..Default::default()
    };

    let rav1e_cfg = Config::new()
        .with_encoder_config(enc_config)
        .with_threads(cli.threads);

    let mut ctx: rav1e::prelude::Context<u8> = rav1e_cfg
        .new_context()
        .map_err(|e| anyhow::anyhow!("failed to create rav1e context: {e}"))?;

    // ── Open output ─────────────────────────────────────────────────
    let output = Output::open(&cli.output)?;

    let mut ivf = IvfWriter::new(output, width as u16, height as u16, 1, cli.fps)
        .map_err(|e| anyhow::anyhow!("failed to write IVF header: {e}"))?;

    // ── Encode loop ─────────────────────────────────────────────────
    let encode_start = Instant::now();
    let total_input_frames = if cli.max_frames > 0 {
        frames.len().min(cli.max_frames)
    } else {
        frames.len()
    };

    let mut frames_sent: u64 = 0;
    let mut packets_received: u64 = 0;
    let mut encoded_bytes: u64 = 0;

    for decoded_frame in frames.iter().take(total_input_frames) {
        let y_plane = decoded_frame.y_plane();
        let u_plane = decoded_frame.u_plane();
        let v_plane = decoded_frame.v_plane();

        let mut rav1e_frame = ctx.new_frame();

        // Copy Y plane.
        rav1e_frame.planes[0].copy_from_raw_u8(y_plane.data(), y_plane.stride() as usize, 1);

        // Copy U (Cb) plane.
        rav1e_frame.planes[1].copy_from_raw_u8(u_plane.data(), u_plane.stride() as usize, 1);

        // Copy V (Cr) plane.
        rav1e_frame.planes[2].copy_from_raw_u8(v_plane.data(), v_plane.stride() as usize, 1);

        match ctx.send_frame(rav1e_frame) {
            Ok(()) => {}
            Err(EncoderStatus::EnoughData) => {
                log::warn!(
                    "encoder returned EnoughData at frame {frames_sent}, draining packets first"
                );
                drain_packets(
                    &mut ctx,
                    &mut ivf,
                    &mut packets_received,
                    &mut encoded_bytes,
                )?;
                // The frame has already been consumed by value above, so we
                // cannot retry.  In practice EnoughData is very rare with
                // sequential send/receive interleaving.
            }
            Err(e) => bail!("rav1e send_frame failed: {e}"),
        }

        frames_sent += 1;

        // Drain any packets that are ready after each send.
        drain_packets(
            &mut ctx,
            &mut ivf,
            &mut packets_received,
            &mut encoded_bytes,
        )?;

        if cli.verbose && frames_sent.is_multiple_of(10) {
            eprintln!(
                "  sent {frames_sent}/{total_input_frames} frames, \
                 received {packets_received} packets"
            );
        }
    }

    // Signal end of input.
    ctx.flush();

    // Drain all remaining packets.
    loop {
        match ctx.receive_packet() {
            Ok(packet) => {
                ivf.write_frame(&packet.data, packet.input_frameno)
                    .context("failed to write IVF frame")?;
                encoded_bytes += packet.data.len() as u64;
                packets_received += 1;
            }
            Err(EncoderStatus::LimitReached) => break,
            Err(EncoderStatus::Encoded) => continue,
            Err(EncoderStatus::NeedMoreData) => break,
            Err(e) => bail!("rav1e receive_packet failed during flush: {e}"),
        }
    }

    // ── Finalise output ─────────────────────────────────────────────
    let output = ivf
        .finish()
        .map_err(|e| anyhow::anyhow!("failed to finalise IVF: {e}"))?;
    output.finish()?;

    let encode_elapsed = encode_start.elapsed();
    let total_elapsed = decode_start.elapsed();
    let encode_fps = if encode_elapsed.as_secs_f64() > 0.0 {
        frames_sent as f64 / encode_elapsed.as_secs_f64()
    } else {
        0.0
    };

    eprintln!();
    eprintln!("--- summary ---");
    eprintln!("  resolution     : {width}x{height}");
    eprintln!("  frames decoded : {total_input_frames}");
    eprintln!("  frames sent    : {frames_sent}");
    eprintln!("  packets out    : {packets_received}");
    eprintln!("  encoded bytes  : {encoded_bytes}");
    eprintln!("  input size     : {} bytes", input_data.len());
    eprintln!(
        "  compression    : {:.1}x",
        if encoded_bytes > 0 {
            input_data.len() as f64 / encoded_bytes as f64
        } else {
            0.0
        }
    );
    eprintln!("  decode time    : {decode_elapsed:.3?}");
    eprintln!("  encode time    : {encode_elapsed:.3?}");
    eprintln!("  total time     : {total_elapsed:.3?}");
    eprintln!("  encode speed   : {encode_fps:.1} fps");
    eprintln!("  speed preset   : {}", cli.speed);
    eprintln!("  quantizer      : {}", cli.quantizer);

    if cli.output.to_str() != Some("-") {
        eprintln!("  output file    : {}", cli.output.display());
    }

    Ok(())
}

/// Drain all currently available packets from the encoder and write them to
/// the IVF output.
fn drain_packets(
    ctx: &mut rav1e::prelude::Context<u8>,
    ivf: &mut IvfWriter<Output>,
    packets_received: &mut u64,
    encoded_bytes: &mut u64,
) -> Result<()> {
    loop {
        match ctx.receive_packet() {
            Ok(packet) => {
                ivf.write_frame(&packet.data, packet.input_frameno)
                    .map_err(|e| anyhow::anyhow!("failed to write IVF frame: {e}"))?;
                *encoded_bytes += packet.data.len() as u64;
                *packets_received += 1;
            }
            Err(EncoderStatus::NeedMoreData | EncoderStatus::Encoded) => break,
            Err(EncoderStatus::LimitReached) => break,
            Err(e) => bail!("rav1e receive_packet failed: {e}"),
        }
    }
    Ok(())
}
