//! Journal core data structures and storage.
//!
//! This module provides the journal entry model (`entry`) and the
//! on-disk storage engine (`storage`) used by both `systemd-journald`
//! (for writing) and `journalctl` (for reading).

pub mod entry;
pub mod storage;
