//! Viewport auto-expose policy for active Editor gestures.

use std::time::Duration;

use novadraw_geometry::{Point, Rectangle, Vec2};
use novadraw_scene::{Figure, FigureId};

use crate::{EditPartFactory, GraphicalViewer, ModelAdapter, ViewerError};

const EDGE_THRESHOLD: f64 = 18.0;
const SCROLL_SPEED_PER_SECOND: f64 = 1_000.0 / 3.0;
const MAX_STEP_ELAPSED: Duration = Duration::from_millis(50);

struct RangeReserveFigure {
    bounds: Rectangle,
}

impl Figure for RangeReserveFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "AutoexposeRangeReserve"
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Result of one host-driven viewport auto-expose step.
pub struct AutoexposeTick {
    scrolled: bool,
    continue_requested: bool,
}

impl AutoexposeTick {
    pub(crate) const fn new(scrolled: bool, continue_requested: bool) -> Self {
        Self {
            scrolled,
            continue_requested,
        }
    }

    /// Returns whether this step changed the Viewport origin.
    pub const fn scrolled(self) -> bool {
        self.scrolled
    }

    /// Returns whether the host should schedule another step.
    pub const fn continue_requested(self) -> bool {
        self.continue_requested
    }
}

pub(crate) fn detects<A, F>(
    viewer: &GraphicalViewer<A, F>,
    pointer: Point,
) -> Result<bool, ViewerError>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    Ok(edge_direction(viewer.viewport_bounds_in_surface()?, pointer).is_some())
}

pub(crate) fn add_range_reserve<A, F>(
    viewer: &mut GraphicalViewer<A, F>,
    pointer: Point,
    feedback: &mut Vec<FigureId>,
) -> Result<(), ViewerError>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    let Some(direction) = edge_direction(viewer.viewport_bounds_in_surface()?, pointer) else {
        return Ok(());
    };
    let reserve_distance =
        EDGE_THRESHOLD + SCROLL_SPEED_PER_SECOND * MAX_STEP_ELAPSED.as_secs_f64();
    let outer = Point::new(
        pointer.x() + direction.x() * reserve_distance,
        pointer.y() + direction.y() * reserve_distance,
    );
    let start = viewer.model_point_from_surface(pointer)?;
    let end = viewer.model_point_from_surface(outer)?;
    let (_, figure) = viewer.add_feedback_visual(
        None,
        true,
        Box::new(RangeReserveFigure {
            bounds: Rectangle::from_corners(start, end),
        }),
    )?;
    feedback.push(figure);
    Ok(())
}

pub(crate) fn step<A, F>(
    viewer: &mut GraphicalViewer<A, F>,
    pointer: Point,
    elapsed: Duration,
) -> Result<AutoexposeTick, ViewerError>
where
    A: ModelAdapter,
    F: EditPartFactory<A>,
{
    let Some(direction) = edge_direction(viewer.viewport_bounds_in_surface()?, pointer) else {
        return Ok(AutoexposeTick::default());
    };
    viewer.runtime_mut().stabilize_for_query()?;
    let (horizontal, vertical) = viewer.viewport_ranges()?;
    if !can_scroll(direction, horizontal, vertical) {
        return Ok(AutoexposeTick::default());
    }

    let distance = scroll_distance(elapsed);
    let scrolled = distance > 0.0
        && viewer.scroll_viewport_by_surface_delta(Vec2::new(
            direction.x() * distance,
            direction.y() * distance,
        ))?;
    let (horizontal, vertical) = viewer.viewport_ranges()?;
    Ok(AutoexposeTick::new(
        scrolled,
        can_scroll(direction, horizontal, vertical),
    ))
}

fn scroll_distance(elapsed: Duration) -> f64 {
    SCROLL_SPEED_PER_SECOND * elapsed.min(MAX_STEP_ELAPSED).as_secs_f64()
}

fn edge_direction(bounds: Rectangle, pointer: Point) -> Option<Vec2> {
    if !pointer.x().is_finite()
        || !pointer.y().is_finite()
        || pointer.x() < bounds.x
        || pointer.x() > bounds.x + bounds.width
        || pointer.y() < bounds.y
        || pointer.y() > bounds.y + bounds.height
    {
        return None;
    }
    let horizontal_threshold = EDGE_THRESHOLD.min(bounds.width / 2.0);
    let vertical_threshold = EDGE_THRESHOLD.min(bounds.height / 2.0);
    let dx = if pointer.x() < bounds.x + horizontal_threshold {
        -1.0
    } else if pointer.x() > bounds.x + bounds.width - horizontal_threshold {
        1.0
    } else {
        0.0
    };
    let dy = if pointer.y() < bounds.y + vertical_threshold {
        -1.0
    } else if pointer.y() > bounds.y + bounds.height - vertical_threshold {
        1.0
    } else {
        0.0
    };
    (dx != 0.0 || dy != 0.0).then(|| Vec2::new(dx, dy))
}

fn can_scroll(
    direction: Vec2,
    horizontal: novadraw_scene::RangeModelSnapshot,
    vertical: novadraw_scene::RangeModelSnapshot,
) -> bool {
    axis_can_scroll(direction.x(), horizontal) || axis_can_scroll(direction.y(), vertical)
}

fn axis_can_scroll(direction: f64, range: novadraw_scene::RangeModelSnapshot) -> bool {
    let maximum = range.minimum.max(range.maximum - range.extent);
    (direction < 0.0 && range.value > range.minimum) || (direction > 0.0 && range.value < maximum)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Rectangle = Rectangle {
        x: 10.0,
        y: 20.0,
        width: 200.0,
        height: 100.0,
    };

    #[test]
    fn edge_detection_requires_an_inside_edge_band() {
        assert_eq!(
            edge_direction(VIEWPORT, Point::new(11.0, 21.0)),
            Some(Vec2::new(-1.0, -1.0))
        );
        assert_eq!(
            edge_direction(VIEWPORT, Point::new(209.0, 70.0)),
            Some(Vec2::new(1.0, 0.0))
        );
        assert_eq!(edge_direction(VIEWPORT, Point::new(100.0, 70.0)), None);
        assert_eq!(edge_direction(VIEWPORT, Point::new(211.0, 70.0)), None);
    }

    #[test]
    fn blocked_axis_does_not_hide_a_scrollable_corner_axis() {
        let blocked = novadraw_scene::RangeModelSnapshot {
            minimum: 0.0,
            maximum: 100.0,
            extent: 100.0,
            value: 0.0,
        };
        let scrollable = novadraw_scene::RangeModelSnapshot {
            minimum: 0.0,
            maximum: 300.0,
            extent: 100.0,
            value: 0.0,
        };
        assert!(can_scroll(Vec2::new(-1.0, 1.0), blocked, scrollable));
        assert!(!can_scroll(Vec2::new(-1.0, 0.0), blocked, scrollable));
    }

    #[test]
    fn long_host_stalls_are_clamped_to_one_bounded_step() {
        assert_eq!(
            scroll_distance(Duration::from_secs(5)),
            scroll_distance(MAX_STEP_ELAPSED)
        );
    }
}
