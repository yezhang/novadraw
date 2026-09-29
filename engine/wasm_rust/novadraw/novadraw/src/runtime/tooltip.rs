//! Runtime-owned tooltip timing and platform-neutral presentation updates.

use std::{collections::VecDeque, fmt, time::Duration};

use crate::geometry::{Point, Rectangle};

use crate::FigureId;

pub const DEFAULT_TOOLTIP_SHOW_DELAY: Duration = Duration::from_millis(500);
pub const DEFAULT_TOOLTIP_HIDE_DELAY: Duration = Duration::from_millis(5_000);
pub const DEFAULT_TOOLTIP_GAP: f64 = 8.0;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct MonotonicTime(u64);

impl MonotonicTime {
    pub const ZERO: Self = Self(0);

    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    pub const fn as_micros(self) -> u64 {
        self.0
    }

    fn checked_add(self, duration_micros: u64) -> Option<Self> {
        self.0.checked_add(duration_micros).map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TooltipTiming {
    show_delay_micros: u64,
    hide_delay_micros: u64,
}

impl TooltipTiming {
    pub fn new(show_delay: Duration, hide_delay: Duration) -> Result<Self, TimeError> {
        Ok(Self {
            show_delay_micros: duration_micros(show_delay)?,
            hide_delay_micros: duration_micros(hide_delay)?,
        })
    }

    pub fn show_delay(self) -> Duration {
        Duration::from_micros(self.show_delay_micros)
    }

    pub fn hide_delay(self) -> Duration {
        Duration::from_micros(self.hide_delay_micros)
    }
}

impl Default for TooltipTiming {
    fn default() -> Self {
        Self {
            show_delay_micros: DEFAULT_TOOLTIP_SHOW_DELAY.as_micros() as u64,
            hide_delay_micros: DEFAULT_TOOLTIP_HIDE_DELAY.as_micros() as u64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeError {
    NonMonotonic {
        previous: MonotonicTime,
        next: MonotonicTime,
    },
    Overflow,
}

impl fmt::Display for TimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonMonotonic { previous, next } => write!(
                formatter,
                "monotonic time moved backwards from {}us to {}us",
                previous.as_micros(),
                next.as_micros()
            ),
            Self::Overflow => formatter.write_str("tooltip deadline exceeds monotonic time range"),
        }
    }
}

impl std::error::Error for TimeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TooltipSide {
    Below,
    Above,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooltipPlacement {
    pub preferred_side: TooltipSide,
    pub gap: f64,
}

impl Default for TooltipPlacement {
    fn default() -> Self {
        Self {
            preferred_side: TooltipSide::Below,
            gap: DEFAULT_TOOLTIP_GAP,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TooltipSnapshot {
    pub revision: u64,
    pub source: FigureId,
    pub text: String,
    pub anchor: Point,
    pub placement: TooltipPlacement,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TooltipUpdate {
    Show(TooltipSnapshot),
    Replace(TooltipSnapshot),
    Hide { revision: u64 },
}

#[derive(Clone, Debug, PartialEq)]
enum TooltipState {
    Hidden,
    Waiting {
        source: FigureId,
        text: String,
        anchor: Point,
        show_at: MonotonicTime,
    },
    Visible {
        snapshot: TooltipSnapshot,
        hide_at: MonotonicTime,
    },
}

pub(crate) struct TooltipController {
    now: MonotonicTime,
    timing: TooltipTiming,
    state: TooltipState,
    pointer_position: Option<Point>,
    revision: u64,
    updates: VecDeque<TooltipUpdate>,
}

impl Default for TooltipController {
    fn default() -> Self {
        Self {
            now: MonotonicTime::ZERO,
            timing: TooltipTiming::default(),
            state: TooltipState::Hidden,
            pointer_position: None,
            revision: 0,
            updates: VecDeque::new(),
        }
    }
}

impl TooltipController {
    pub(crate) fn set_timing(&mut self, timing: TooltipTiming) -> Result<bool, TimeError> {
        self.validate_deadline_capacity(timing)?;
        if self.timing == timing {
            return Ok(false);
        }
        self.timing = timing;
        self.rebase_deadline();
        Ok(true)
    }

    pub(crate) fn set_pointer_position(&mut self, point: Point) {
        self.pointer_position = Some(point);
    }

    pub(crate) fn pointer_position(&self) -> Option<Point> {
        self.pointer_position
    }

    pub(crate) fn clear_pointer_position(&mut self) {
        self.pointer_position = None;
        self.dismiss();
    }

    pub(crate) fn reconcile(&mut self, source: Option<(FigureId, String)>) {
        let Some(anchor) = self.pointer_position else {
            self.dismiss();
            return;
        };
        let Some((source, text)) = source.filter(|(_, text)| !text.is_empty()) else {
            self.dismiss();
            return;
        };

        match self.state.clone() {
            TooltipState::Hidden => {
                self.state = TooltipState::Waiting {
                    source,
                    text,
                    anchor,
                    show_at: self.show_deadline(),
                };
            }
            TooltipState::Waiting {
                source: current_source,
                text: current_text,
                show_at,
                ..
            } if current_source == source && current_text == text => {
                self.state = TooltipState::Waiting {
                    source,
                    text,
                    anchor,
                    show_at,
                };
            }
            TooltipState::Waiting { .. } => {
                self.state = TooltipState::Waiting {
                    source,
                    text,
                    anchor,
                    show_at: self.show_deadline(),
                };
            }
            TooltipState::Visible {
                snapshot,
                hide_at: _,
            } => {
                if snapshot.source == source && snapshot.text == text && snapshot.anchor == anchor {
                    return;
                }
                let next = self.next_snapshot(source, text, anchor);
                self.state = TooltipState::Visible {
                    snapshot: next.clone(),
                    hide_at: self.hide_deadline(),
                };
                self.updates.push_back(TooltipUpdate::Replace(next));
            }
        }
    }

    pub(crate) fn dismiss(&mut self) {
        if matches!(self.state, TooltipState::Hidden) {
            return;
        }
        let was_visible = matches!(self.state, TooltipState::Visible { .. });
        self.state = TooltipState::Hidden;
        if was_visible {
            let revision = self.next_revision();
            self.updates.push_back(TooltipUpdate::Hide { revision });
        }
    }

    pub(crate) fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, TimeError> {
        if now < self.now {
            return Err(TimeError::NonMonotonic {
                previous: self.now,
                next: now,
            });
        }
        self.validate_time(now)?;
        self.now = now;

        let previous_revision = self.revision;
        match self.state.clone() {
            TooltipState::Waiting {
                source,
                text,
                anchor,
                show_at,
            } if now >= show_at => {
                let snapshot = self.next_snapshot(source, text, anchor);
                self.state = TooltipState::Visible {
                    snapshot: snapshot.clone(),
                    hide_at: self.hide_deadline(),
                };
                self.updates.push_back(TooltipUpdate::Show(snapshot));
            }
            TooltipState::Visible { hide_at, .. } if now >= hide_at => {
                self.state = TooltipState::Hidden;
                let revision = self.next_revision();
                self.updates.push_back(TooltipUpdate::Hide { revision });
            }
            _ => {}
        }
        Ok(self.revision != previous_revision)
    }

    pub(crate) fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        match self.state {
            TooltipState::Hidden => None,
            TooltipState::Waiting { show_at, .. } => Some(show_at),
            TooltipState::Visible { hide_at, .. } => Some(hide_at),
        }
    }

    pub(crate) fn visible_snapshot(&self) -> Option<&TooltipSnapshot> {
        match &self.state {
            TooltipState::Visible { snapshot, .. } => Some(snapshot),
            TooltipState::Hidden | TooltipState::Waiting { .. } => None,
        }
    }

    pub(crate) fn take_updates(&mut self) -> Vec<TooltipUpdate> {
        self.updates.drain(..).collect()
    }

    fn rebase_deadline(&mut self) {
        let show_at = self.show_deadline();
        let hide_at = self.hide_deadline();
        match &mut self.state {
            TooltipState::Waiting {
                show_at: deadline, ..
            } => *deadline = show_at,
            TooltipState::Visible {
                hide_at: deadline, ..
            } => *deadline = hide_at,
            TooltipState::Hidden => {}
        }
    }

    fn next_snapshot(&mut self, source: FigureId, text: String, anchor: Point) -> TooltipSnapshot {
        TooltipSnapshot {
            revision: self.next_revision(),
            source,
            text,
            anchor,
            placement: TooltipPlacement::default(),
        }
    }

    fn next_revision(&mut self) -> u64 {
        self.revision = self.revision.wrapping_add(1);
        self.revision
    }

    fn show_deadline(&self) -> MonotonicTime {
        self.now
            .checked_add(self.timing.show_delay_micros)
            .expect("accepted monotonic time must leave room for tooltip deadlines")
    }

    fn hide_deadline(&self) -> MonotonicTime {
        self.now
            .checked_add(self.timing.hide_delay_micros)
            .expect("accepted monotonic time must leave room for tooltip deadlines")
    }

    fn validate_deadline_capacity(&self, timing: TooltipTiming) -> Result<(), TimeError> {
        self.now
            .checked_add(timing.show_delay_micros)
            .and_then(|_| self.now.checked_add(timing.hide_delay_micros))
            .ok_or(TimeError::Overflow)
            .map(|_| ())
    }

    fn validate_time(&self, now: MonotonicTime) -> Result<(), TimeError> {
        now.checked_add(self.timing.show_delay_micros)
            .and_then(|_| now.checked_add(self.timing.hide_delay_micros))
            .ok_or(TimeError::Overflow)
            .map(|_| ())
    }
}

pub fn place_tooltip(
    anchor: Point,
    popup_size: (f64, f64),
    surface_bounds: Rectangle,
    placement: TooltipPlacement,
) -> Option<Rectangle> {
    let (width, height) = popup_size;
    if !anchor.x().is_finite()
        || !anchor.y().is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || !placement.gap.is_finite()
        || width < 0.0
        || height < 0.0
        || placement.gap < 0.0
        || surface_bounds.width < 0.0
        || surface_bounds.height < 0.0
    {
        return None;
    }

    let below = anchor.y() + placement.gap;
    let above = anchor.y() - placement.gap - height;
    let preferred_y = match placement.preferred_side {
        TooltipSide::Below => below,
        TooltipSide::Above => above,
    };
    let fallback_y = match placement.preferred_side {
        TooltipSide::Below => above,
        TooltipSide::Above => below,
    };
    let surface_bottom = surface_bounds.y + surface_bounds.height;
    let preferred_fits = preferred_y >= surface_bounds.y && preferred_y + height <= surface_bottom;
    let fallback_fits = fallback_y >= surface_bounds.y && fallback_y + height <= surface_bottom;
    let y = if preferred_fits {
        preferred_y
    } else if fallback_fits {
        fallback_y
    } else {
        preferred_y.clamp(
            surface_bounds.y,
            (surface_bottom - height).max(surface_bounds.y),
        )
    };
    let surface_right = surface_bounds.x + surface_bounds.width;
    let x = anchor.x().clamp(
        surface_bounds.x,
        (surface_right - width).max(surface_bounds.x),
    );
    Some(Rectangle::new(x, y, width, height))
}

fn duration_micros(duration: Duration) -> Result<u64, TimeError> {
    u64::try_from(duration.as_micros()).map_err(|_| TimeError::Overflow)
}

#[cfg(test)]
mod tests {
    use slotmap::KeyData;

    use super::*;

    fn id(value: u64) -> FigureId {
        FigureId::from(KeyData::from_ffi(value))
    }

    #[test]
    fn waits_replaces_and_hides_on_deadlines() {
        let mut controller = TooltipController::default();
        controller
            .set_timing(
                TooltipTiming::new(Duration::from_micros(10), Duration::from_micros(20)).unwrap(),
            )
            .unwrap();
        controller.set_pointer_position(Point::new(40.0, 30.0));
        controller.reconcile(Some((id(1), "first".to_string())));

        assert_eq!(
            controller.next_wake_deadline(),
            Some(MonotonicTime::from_micros(10))
        );
        assert!(
            !controller
                .advance_time(MonotonicTime::from_micros(9))
                .unwrap()
        );
        assert!(
            controller
                .advance_time(MonotonicTime::from_micros(10))
                .unwrap()
        );
        assert!(matches!(
            controller.take_updates().as_slice(),
            [TooltipUpdate::Show(_)]
        ));

        controller.reconcile(Some((id(2), "second".to_string())));
        assert!(matches!(
            controller.take_updates().as_slice(),
            [TooltipUpdate::Replace(snapshot)] if snapshot.text == "second"
        ));
        assert_eq!(
            controller.next_wake_deadline(),
            Some(MonotonicTime::from_micros(30))
        );

        assert!(
            controller
                .advance_time(MonotonicTime::from_micros(30))
                .unwrap()
        );
        assert!(matches!(
            controller.take_updates().as_slice(),
            [TooltipUpdate::Hide { .. }]
        ));
    }

    #[test]
    fn rejects_clock_rollback_without_changing_deadline() {
        let mut controller = TooltipController::default();
        controller
            .advance_time(MonotonicTime::from_micros(100))
            .unwrap();
        controller.set_pointer_position(Point::new(1.0, 2.0));
        controller.reconcile(Some((id(1), "tip".to_string())));
        let deadline = controller.next_wake_deadline();

        assert!(matches!(
            controller.advance_time(MonotonicTime::from_micros(99)),
            Err(TimeError::NonMonotonic { .. })
        ));
        assert_eq!(controller.next_wake_deadline(), deadline);
    }

    #[test]
    fn placement_flips_and_clamps_to_surface() {
        let surface = Rectangle::new(0.0, 0.0, 100.0, 80.0);
        let placement = TooltipPlacement::default();

        assert_eq!(
            place_tooltip(Point::new(90.0, 70.0), (30.0, 20.0), surface, placement),
            Some(Rectangle::new(70.0, 42.0, 30.0, 20.0))
        );
        assert_eq!(
            place_tooltip(Point::new(-5.0, 2.0), (120.0, 100.0), surface, placement),
            Some(Rectangle::new(0.0, 0.0, 120.0, 100.0))
        );
    }
}
