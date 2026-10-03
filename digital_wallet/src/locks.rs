use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

// Poisoned locks carry a value that is still usable; recovering it is safer
// for a long-lived service than unwrap-panicking.
#[inline]
pub(crate) fn rd<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[inline]
pub(crate) fn wr<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
