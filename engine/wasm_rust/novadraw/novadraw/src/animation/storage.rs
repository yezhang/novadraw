use std::sync::{Arc, Mutex};

use super::{AnimationBudgetKind, AnimationError};

/// Independent retention responsibility. A capture cannot borrow the next-frame reservation.
#[derive(Clone)]
pub(crate) struct CaptureStorage {
    accounting: Arc<Mutex<Accounting>>,
}

struct Accounting {
    limit: usize,
    live: usize,
}

pub(crate) struct CaptureStorageLease {
    accounting: Arc<Mutex<Accounting>>,
    bytes: usize,
}

impl CaptureStorage {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            accounting: Arc::new(Mutex::new(Accounting { limit, live: 0 })),
        }
    }

    pub(crate) fn acquire(&self, bytes: usize) -> Result<CaptureStorageLease, AnimationError> {
        let mut accounting = self.accounting.lock().expect("capture storage lock");
        let live = accounting
            .live
            .checked_add(bytes)
            .ok_or(AnimationError::BudgetExceeded(
                AnimationBudgetKind::SnapshotBytes,
            ))?;
        if live > accounting.limit {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::SnapshotBytes,
            ));
        }
        accounting.live = live;
        Ok(CaptureStorageLease {
            accounting: self.accounting.clone(),
            bytes,
        })
    }

    pub(crate) fn set_limit(&self, limit: usize) -> Result<(), AnimationError> {
        let mut accounting = self.accounting.lock().expect("capture storage lock");
        if accounting.live > limit {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::SnapshotBytes,
            ));
        }
        accounting.limit = limit;
        Ok(())
    }

    pub(crate) fn live_bytes(&self) -> usize {
        self.accounting.lock().expect("capture storage lock").live
    }

    pub(crate) fn available_bytes(&self) -> usize {
        let accounting = self.accounting.lock().expect("capture storage lock");
        accounting.limit - accounting.live
    }
}

impl CaptureStorageLease {
    pub(crate) fn shrink_to(&mut self, bytes: usize) -> Result<(), AnimationError> {
        let released = self
            .bytes
            .checked_sub(bytes)
            .ok_or(AnimationError::BudgetExceeded(
                AnimationBudgetKind::SnapshotBytes,
            ))?;
        let mut accounting = self.accounting.lock().expect("capture storage lock");
        accounting.live -= released;
        self.bytes = bytes;
        Ok(())
    }
}

impl Drop for CaptureStorageLease {
    fn drop(&mut self) {
        let mut accounting = self.accounting.lock().expect("capture storage lock");
        accounting.live = accounting
            .live
            .checked_sub(self.bytes)
            .expect("capture storage lease released once");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_is_shared_and_released_only_with_its_owner() {
        let storage = CaptureStorage::new(8);
        let mut lease = storage.acquire(8).unwrap();
        assert_eq!(storage.clone().live_bytes(), 8);
        assert!(storage.acquire(1).is_err());
        assert!(storage.set_limit(7).is_err());
        assert_eq!(storage.live_bytes(), 8);
        assert_eq!(storage.available_bytes(), 0);
        lease.shrink_to(6).unwrap();
        assert_eq!(storage.live_bytes(), 6);
        assert_eq!(storage.available_bytes(), 2);
        assert!(lease.shrink_to(7).is_err());
        assert_eq!(storage.live_bytes(), 6);
        drop(lease);
        assert_eq!(storage.live_bytes(), 0);
        storage.set_limit(7).unwrap();
        assert!(storage.acquire(8).is_err());
    }
}
