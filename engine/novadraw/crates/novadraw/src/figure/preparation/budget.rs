use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

/// Work and auxiliary collection entries allowed in one preparation session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparationBudget {
    pub work: u64,
    pub allocation: u64,
}

impl PreparationBudget {
    pub const fn new(work: u64, allocation: u64) -> Self {
        Self { work, allocation }
    }
}

impl Default for PreparationBudget {
    fn default() -> Self {
        // Bounds the whole stabilization, including repeated consumer callbacks.
        Self::new(10_000_000, 2_000_000)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparationBudgetError {
    pub stage: &'static str,
    /// Attempted cumulative usage, saturated at `u64::MAX` on arithmetic overflow.
    pub used: u64,
    pub limit: u64,
    pub allocation: bool,
}

impl fmt::Display for PreparationBudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "preparation budget exceeded at {}: usage {}, limit {} (allocation: {})",
            self.stage, self.used, self.limit, self.allocation
        )
    }
}
impl Error for PreparationBudgetError {}

struct Accounting {
    used: PreparationBudget,
    failure: Option<PreparationBudgetError>,
}

/// Shared accounting, never reset by a measurement or algorithm callback.
/// Allocation charges both work and allocation units before the operation.
/// Once exhausted, the session stays failed even if a callback ignores the error.
#[derive(Clone)]
pub struct PreparationBudgetHandle {
    limit: PreparationBudget,
    accounting: Arc<Mutex<Accounting>>,
    timing: crate::runtime::timing::TimingRecorder,
}

impl PreparationBudgetHandle {
    pub fn new(limit: PreparationBudget) -> Self {
        Self {
            limit,
            accounting: Arc::new(Mutex::new(Accounting {
                used: PreparationBudget::new(0, 0),
                failure: None,
            })),
            timing: crate::runtime::timing::TimingRecorder::default(),
        }
    }

    pub(crate) fn with_timing(mut self, timing: crate::runtime::timing::TimingRecorder) -> Self {
        self.timing = timing;
        self
    }

    pub(crate) fn time(&self, stage: &'static str) -> crate::runtime::timing::TimingScope {
        self.timing.start(stage)
    }

    pub fn limit(&self) -> PreparationBudget {
        self.limit
    }

    pub fn used(&self) -> PreparationBudget {
        self.accounting.lock().unwrap().used
    }

    pub fn check(&self) -> Result<(), PreparationBudgetError> {
        self.accounting
            .lock()
            .unwrap()
            .failure
            .clone()
            .map_or(Ok(()), Err)
    }

    pub fn consume(&self, stage: &'static str, units: u64) -> Result<(), PreparationBudgetError> {
        self.charge(stage, units, 0)
    }

    pub fn allocate(&self, stage: &'static str, units: u64) -> Result<(), PreparationBudgetError> {
        self.charge(stage, units, units)
    }

    fn charge(
        &self,
        stage: &'static str,
        work: u64,
        allocation: u64,
    ) -> Result<(), PreparationBudgetError> {
        let mut accounting = self.accounting.lock().unwrap();
        if let Some(error) = &accounting.failure {
            return Err(error.clone());
        }
        let next = PreparationBudget::new(
            accounting.used.work.saturating_add(work),
            accounting.used.allocation.saturating_add(allocation),
        );
        for (used, increment, current, limit, allocation) in [
            (
                next.work,
                work,
                accounting.used.work,
                self.limit.work,
                false,
            ),
            (
                next.allocation,
                allocation,
                accounting.used.allocation,
                self.limit.allocation,
                true,
            ),
        ] {
            if used > limit || current.checked_add(increment).is_none() {
                let error = PreparationBudgetError {
                    stage,
                    used,
                    limit,
                    allocation,
                };
                accounting.failure = Some(error.clone());
                return Err(error);
            }
        }
        accounting.used = next;
        Ok(())
    }
}

impl Default for PreparationBudgetHandle {
    fn default() -> Self {
        Self::new(PreparationBudget::default())
    }
}

impl From<PreparationBudgetError> for crate::text::TextError {
    fn from(error: PreparationBudgetError) -> Self {
        Self::PreparationDependency(Box::new(super::PreparationDependencyError::Budget(error)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_cannot_replenish_or_partially_charge_the_shared_counter() {
        let budget = PreparationBudgetHandle::new(PreparationBudget::new(u64::MAX, u64::MAX));
        budget.allocate("payload", u64::MAX).unwrap();
        let error = budget.clone().allocate("next payload", 1).unwrap_err();
        assert_eq!(error.stage, "next payload");
        assert_eq!(budget.used(), PreparationBudget::new(u64::MAX, u64::MAX));
        assert_eq!(budget.consume("ignored failure", 0), Err(error));
    }
}
