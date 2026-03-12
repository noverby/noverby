//! `tangled-spindle` — Rust reimplementation of the Tangled Spindle CI runner.
//!
//! This is the main entry point for the tangled-spindle-nix binary. It wires
//! together all subsystems (HTTP server, Jetstream consumer, knot event consumer,
//! engine, database, RBAC, secrets, job queue) and runs them concurrently.
//!
//! See PLAN.md for the full architecture and phase plan.

fn main() {
    println!("tangled-spindle-nix v0.1.0");
    println!("🚧 Early development — see PLAN.md for implementation status.");
}
