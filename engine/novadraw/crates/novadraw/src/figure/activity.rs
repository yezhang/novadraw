use super::CapabilityKey;
use crate::runtime::MonotonicTime;
use crate::{EventContext, Figure};

/// Work performed at Runtime input, stabilization and clock boundaries.
///
/// Callbacks observe the committed scene and submit ordinary deferred effects.
/// `active` is false when an ancestor is hidden or disabled. Removal drops the
/// registration; there are no background callbacks after detach.
pub trait FigureActivity {
    fn poll(&self, now: MonotonicTime, active: bool, context: &mut EventContext<'_>);

    fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        None
    }
}

#[derive(Clone, Copy)]
pub struct ActivityCapability {
    resolve: for<'a> fn(&'a dyn Figure) -> Option<&'a dyn FigureActivity>,
}

impl ActivityCapability {
    pub fn of<T: Figure + FigureActivity + 'static>() -> Self {
        Self {
            resolve: |figure| {
                figure
                    .as_any()
                    .downcast_ref::<T>()
                    .map(|figure| figure as &dyn FigureActivity)
            },
        }
    }

    pub(crate) fn resolve(self, figure: &dyn Figure) -> Option<&dyn FigureActivity> {
        (self.resolve)(figure)
    }
}

pub const ACTIVITY: CapabilityKey<ActivityCapability> = CapabilityKey::new("novadraw.activity");
