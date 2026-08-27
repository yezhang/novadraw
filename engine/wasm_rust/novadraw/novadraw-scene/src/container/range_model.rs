use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeModelSnapshot {
    pub minimum: f64,
    pub maximum: f64,
    pub extent: f64,
    pub value: f64,
}

impl RangeModelSnapshot {
    pub fn is_enabled(self) -> bool {
        self.maximum - self.minimum > self.extent
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RangeProperty {
    Minimum,
    Maximum,
    Extent,
    Value,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeChange {
    pub property: RangeProperty,
    pub old_value: f64,
    pub new_value: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RangeChangeSet {
    changes: Vec<RangeChange>,
}

impl RangeChangeSet {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn changes(&self) -> &[RangeChange] {
        &self.changes
    }

    fn push(&mut self, property: RangeProperty, old_value: f64, new_value: f64) {
        if old_value != new_value {
            self.changes.push(RangeChange {
                property,
                old_value,
                new_value,
            });
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct RangeListenerId(u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RangeModelError {
    NonFiniteValue,
    InvalidBounds,
}

impl fmt::Display for RangeModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteValue => write!(f, "range values must be finite"),
            Self::InvalidBounds => write!(f, "range maximum must not be less than minimum"),
        }
    }
}

impl Error for RangeModelError {}

pub trait RangeListener: Send + Sync {
    fn range_changed(&self, change: RangeChange);
}

pub trait RangeModel: Send + Sync {
    fn snapshot(&self) -> RangeModelSnapshot;

    fn set_all(
        &self,
        minimum: f64,
        extent: f64,
        maximum: f64,
    ) -> Result<RangeChangeSet, RangeModelError>;

    fn set_minimum(&self, minimum: f64) -> Result<RangeChangeSet, RangeModelError>;

    fn set_maximum(&self, maximum: f64) -> Result<RangeChangeSet, RangeModelError>;

    fn set_extent(&self, extent: f64) -> Result<RangeChangeSet, RangeModelError>;

    fn set_value(&self, value: f64) -> Result<RangeChangeSet, RangeModelError>;

    fn add_listener(&self, listener: Arc<dyn RangeListener>) -> RangeListenerId;

    fn remove_listener(&self, id: RangeListenerId) -> bool;

    fn minimum(&self) -> f64 {
        self.snapshot().minimum
    }

    fn maximum(&self) -> f64 {
        self.snapshot().maximum
    }

    fn extent(&self) -> f64 {
        self.snapshot().extent
    }

    fn value(&self) -> f64 {
        self.snapshot().value
    }

    fn is_enabled(&self) -> bool {
        self.snapshot().is_enabled()
    }
}

struct RangeModelInner {
    state: RangeModelSnapshot,
    listeners: Vec<(RangeListenerId, Arc<dyn RangeListener>)>,
    next_listener_id: u64,
}

pub struct DefaultRangeModel {
    inner: Mutex<RangeModelInner>,
}

impl DefaultRangeModel {
    pub fn new(minimum: f64, extent: f64, maximum: f64) -> Result<Self, RangeModelError> {
        let state = normalize_range(minimum, extent, maximum, minimum)?;
        Ok(Self {
            inner: Mutex::new(RangeModelInner {
                state,
                listeners: Vec::new(),
                next_listener_id: 1,
            }),
        })
    }

    fn mutate(
        &self,
        update: impl FnOnce(RangeModelSnapshot) -> Result<RangeModelSnapshot, RangeModelError>,
    ) -> Result<RangeChangeSet, RangeModelError> {
        let (changes, listeners) = {
            let mut inner = lock_unpoisoned(&self.inner);
            let old = inner.state;
            let new = update(old)?;
            let changes = changes_between(old, new);
            if changes.is_empty() {
                return Ok(changes);
            }
            inner.state = new;
            let listeners = inner
                .listeners
                .iter()
                .map(|(_, listener)| Arc::clone(listener))
                .collect::<Vec<_>>();
            (changes, listeners)
        };

        for change in changes.changes() {
            for listener in &listeners {
                listener.range_changed(*change);
            }
        }
        Ok(changes)
    }
}

impl Default for DefaultRangeModel {
    fn default() -> Self {
        Self::new(0.0, 20.0, 100.0).expect("default range is valid")
    }
}

impl RangeModel for DefaultRangeModel {
    fn snapshot(&self) -> RangeModelSnapshot {
        lock_unpoisoned(&self.inner).state
    }

    fn set_all(
        &self,
        minimum: f64,
        extent: f64,
        maximum: f64,
    ) -> Result<RangeChangeSet, RangeModelError> {
        self.mutate(|old| normalize_range(minimum, extent, maximum, old.value))
    }

    fn set_minimum(&self, minimum: f64) -> Result<RangeChangeSet, RangeModelError> {
        self.mutate(|old| normalize_range(minimum, old.extent, old.maximum, old.value))
    }

    fn set_maximum(&self, maximum: f64) -> Result<RangeChangeSet, RangeModelError> {
        self.mutate(|old| normalize_range(old.minimum, old.extent, maximum, old.value))
    }

    fn set_extent(&self, extent: f64) -> Result<RangeChangeSet, RangeModelError> {
        self.mutate(|old| normalize_range(old.minimum, extent, old.maximum, old.value))
    }

    fn set_value(&self, value: f64) -> Result<RangeChangeSet, RangeModelError> {
        self.mutate(|old| normalize_range(old.minimum, old.extent, old.maximum, value))
    }

    fn add_listener(&self, listener: Arc<dyn RangeListener>) -> RangeListenerId {
        let mut inner = lock_unpoisoned(&self.inner);
        let id = RangeListenerId(inner.next_listener_id);
        inner.next_listener_id = inner
            .next_listener_id
            .checked_add(1)
            .expect("range listener id space exhausted");
        inner.listeners.push((id, listener));
        id
    }

    fn remove_listener(&self, id: RangeListenerId) -> bool {
        let mut inner = lock_unpoisoned(&self.inner);
        let old_len = inner.listeners.len();
        inner
            .listeners
            .retain(|(listener_id, _)| *listener_id != id);
        old_len != inner.listeners.len()
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn normalize_range(
    minimum: f64,
    extent: f64,
    maximum: f64,
    value: f64,
) -> Result<RangeModelSnapshot, RangeModelError> {
    if !minimum.is_finite() || !extent.is_finite() || !maximum.is_finite() || !value.is_finite() {
        return Err(RangeModelError::NonFiniteValue);
    }
    if maximum < minimum {
        return Err(RangeModelError::InvalidBounds);
    }
    let span = maximum - minimum;
    if !span.is_finite() {
        return Err(RangeModelError::NonFiniteValue);
    }
    let extent = extent.clamp(0.0, span);
    let value = value.clamp(minimum, maximum - extent);
    Ok(RangeModelSnapshot {
        minimum,
        maximum,
        extent,
        value,
    })
}

fn changes_between(old: RangeModelSnapshot, new: RangeModelSnapshot) -> RangeChangeSet {
    let mut changes = RangeChangeSet::default();
    // Keep Draw2D's DefaultRangeModel notification order.
    changes.push(RangeProperty::Maximum, old.maximum, new.maximum);
    changes.push(RangeProperty::Extent, old.extent, new.extent);
    changes.push(RangeProperty::Minimum, old.minimum, new.minimum);
    changes.push(RangeProperty::Value, old.value, new.value);
    changes
}
