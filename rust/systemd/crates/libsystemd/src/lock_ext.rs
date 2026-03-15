//! Poison-recovering lock extension traits.
//!
//! When a thread panics while holding a `Mutex` or `RwLock`, the lock becomes
//! "poisoned" — all subsequent `.lock()` / `.read()` / `.write()` calls return
//! `Err(PoisonError)`. In a service manager (PID 1) we must **never** cascade
//! one thread's panic into every other thread, so we recover the inner data
//! from the `PoisonError` and continue.
//!
//! Usage:
//! ```ignore
//! use crate::lock_ext::LockExt;
//!
//! let data = my_mutex.lock_poisoned();
//! let data = my_rwlock.read_poisoned();
//! let data = my_rwlock.write_poisoned();
//! ```

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Extension trait that adds poison-recovering methods to `Mutex`.
pub trait MutexExt<T> {
    /// Acquire the mutex, recovering from a poisoned state.
    ///
    /// If the mutex was poisoned (a thread panicked while holding it),
    /// the data is recovered and a warning is logged once.
    fn lock_poisoned(&self) -> MutexGuard<'_, T>;
}

/// Extension trait that adds poison-recovering methods to `RwLock`.
pub trait RwLockExt<T> {
    /// Acquire a read lock, recovering from a poisoned state.
    fn read_poisoned(&self) -> RwLockReadGuard<'_, T>;

    /// Acquire a write lock, recovering from a poisoned state.
    fn write_poisoned(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_poisoned(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| {
            log::warn!("Recovered poisoned Mutex (a thread panicked while holding this lock)");
            e.into_inner()
        })
    }
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn read_poisoned(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|e| {
            log::warn!(
                "Recovered poisoned RwLock (read) (a thread panicked while holding this lock)"
            );
            e.into_inner()
        })
    }

    fn write_poisoned(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|e| {
            log::warn!(
                "Recovered poisoned RwLock (write) (a thread panicked while holding this lock)"
            );
            e.into_inner()
        })
    }
}
