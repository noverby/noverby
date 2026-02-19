//! systemd-id128 — 128-bit ID operations.
//!
//! A drop-in replacement for `systemd-id128(1)` supporting the following
//! subcommands:
//!
//! - `new`           — Generate a new random 128-bit ID
//! - `machine-id`    — Print the machine ID (`/etc/machine-id`)
//! - `boot-id`       — Print the boot ID (`/proc/sys/kernel/random/boot_id`)
//! - `invocation-id` — Print the invocation ID (`$INVOCATION_ID`)
//!
//! Output formats:
//! - default: 32 hex characters (no dashes)
//! - `--uuid` / `-u`: RFC 4122 UUID format (8-4-4-4-12)

use clap::{Parser, Subcommand};
use std::fs;
use std::io::Read;

#[derive(Parser, Debug)]
#[command(
    name = "systemd-id128",
    about = "Generate and print 128-bit identifiers",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate a new random 128-bit ID.
    New {
        #[arg(short, long, help = "Output as UUID (8-4-4-4-12 format)")]
        uuid: bool,

        #[arg(
            short = 'p',
            long,
            value_name = "UUID",
            help = "Generate an ID derived from the given application-specific ID"
        )]
        app_specific: Option<String>,
    },

    /// Print the machine ID from /etc/machine-id.
    #[command(name = "machine-id")]
    MachineId {
        #[arg(short, long, help = "Output as UUID (8-4-4-4-12 format)")]
        uuid: bool,

        #[arg(
            short = 'p',
            long,
            value_name = "UUID",
            help = "Generate an ID derived from the given application-specific ID"
        )]
        app_specific: Option<String>,
    },

    /// Print the boot ID from /proc/sys/kernel/random/boot_id.
    #[command(name = "boot-id")]
    BootId {
        #[arg(short, long, help = "Output as UUID (8-4-4-4-12 format)")]
        uuid: bool,

        #[arg(
            short = 'p',
            long,
            value_name = "UUID",
            help = "Generate an ID derived from the given application-specific ID"
        )]
        app_specific: Option<String>,
    },

    /// Print the invocation ID from $INVOCATION_ID.
    #[command(name = "invocation-id")]
    InvocationId {
        #[arg(short, long, help = "Output as UUID (8-4-4-4-12 format)")]
        uuid: bool,
    },
}

/// A 128-bit identifier stored as raw bytes.
#[derive(Clone, Copy)]
struct Id128([u8; 16]);

impl Id128 {
    /// Format as 32 lowercase hex characters (no dashes).
    fn to_hex(self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Format as RFC 4122 UUID: 8-4-4-4-12.
    fn to_uuid(self) -> String {
        let h = self.to_hex();
        format!(
            "{}-{}-{}-{}-{}",
            &h[0..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..32]
        )
    }

    /// Format according to the `--uuid` flag.
    fn format(self, uuid: bool) -> String {
        if uuid { self.to_uuid() } else { self.to_hex() }
    }

    /// Parse from a hex string (with or without dashes).
    fn from_hex(s: &str) -> Result<Self, String> {
        let clean: String = s.chars().filter(|c| *c != '-').collect();
        let clean = clean.trim();
        if clean.len() != 32 {
            return Err(format!(
                "Expected 32 hex characters, got {} in {:?}",
                clean.len(),
                s
            ));
        }
        let mut bytes = [0u8; 16];
        for i in 0..16 {
            bytes[i] = u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16)
                .map_err(|e| format!("Invalid hex at position {}: {}", i * 2, e))?;
        }
        Ok(Id128(bytes))
    }
}

/// Generate a new random 128-bit ID by reading from /dev/urandom.
fn generate_random_id() -> Result<Id128, String> {
    let mut f =
        fs::File::open("/dev/urandom").map_err(|e| format!("Failed to open /dev/urandom: {e}"))?;
    let mut buf = [0u8; 16];
    f.read_exact(&mut buf)
        .map_err(|e| format!("Failed to read from /dev/urandom: {e}"))?;

    // Set UUID version 4 (random) and variant bits per RFC 4122.
    buf[6] = (buf[6] & 0x0f) | 0x40; // version 4
    buf[8] = (buf[8] & 0x3f) | 0x80; // variant 1

    Ok(Id128(buf))
}

/// Read the machine ID from /etc/machine-id.
fn read_machine_id() -> Result<Id128, String> {
    let content = fs::read_to_string("/etc/machine-id")
        .map_err(|e| format!("Failed to read /etc/machine-id: {e}"))?;
    Id128::from_hex(content.trim())
}

/// Read the boot ID from /proc/sys/kernel/random/boot_id.
fn read_boot_id() -> Result<Id128, String> {
    let content = fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|e| format!("Failed to read /proc/sys/kernel/random/boot_id: {e}"))?;
    Id128::from_hex(content.trim())
}

/// Read the invocation ID from the $INVOCATION_ID environment variable.
fn read_invocation_id() -> Result<Id128, String> {
    let val = std::env::var("INVOCATION_ID")
        .map_err(|_| "INVOCATION_ID environment variable is not set".to_string())?;
    Id128::from_hex(val.trim())
}

/// Derive an application-specific ID from a base ID and an application ID.
///
/// Uses a simple XOR-fold derivation: XOR the base and app IDs together,
/// then set UUID v4 version and variant bits for a well-formed result.
/// This is a simplified version of sd_id128_get_machine_app_specific().
fn derive_app_specific(base: Id128, app_id_str: &str) -> Result<Id128, String> {
    let app_id = Id128::from_hex(app_id_str)?;

    let mut result = [0u8; 16];
    for (i, byte) in result.iter_mut().enumerate() {
        *byte = base.0[i] ^ app_id.0[i];
    }

    // Mix further by rotating bytes based on XOR sum to avoid trivial collisions.
    let xor_sum: u8 = result.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    let rotation = (xor_sum as usize) % 16;
    let mut mixed = [0u8; 16];
    for i in 0..16 {
        mixed[i] = result[(i + rotation) % 16];
    }

    // Set UUID version 4 and variant bits.
    mixed[6] = (mixed[6] & 0x0f) | 0x40;
    mixed[8] = (mixed[8] & 0x3f) | 0x80;

    Ok(Id128(mixed))
}

/// Optionally apply app-specific derivation to an ID.
fn maybe_derive(id: Id128, app_specific: &Option<String>) -> Result<Id128, String> {
    match app_specific {
        Some(app_id) => derive_app_specific(id, app_id),
        None => Ok(id),
    }
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::New { uuid, app_specific } => {
            let id = generate_random_id().unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            let id = maybe_derive(id, &app_specific).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            id.format(uuid)
        }

        Command::MachineId { uuid, app_specific } => {
            let id = read_machine_id().unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            let id = maybe_derive(id, &app_specific).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            id.format(uuid)
        }

        Command::BootId { uuid, app_specific } => {
            let id = read_boot_id().unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            let id = maybe_derive(id, &app_specific).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            id.format(uuid)
        }

        Command::InvocationId { uuid } => {
            let id = read_invocation_id().unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            id.format(uuid)
        }
    };

    println!("{result}");
}
