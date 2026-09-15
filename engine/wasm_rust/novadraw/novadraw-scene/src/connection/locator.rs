use std::{error::Error, fmt};

use novadraw_geometry::{Point, PointList};

/// Position and direction reference produced by a Connection Locator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocatorPlacement {
    /// Position on the committed local route.
    pub point: Point,
    /// Neighboring point used to orient endpoint decorations.
    pub reference: Point,
}

/// Pure strategy for placing a child on committed Connection points.
pub trait ConnectionLocatorStrategy {
    /// Resolves one placement from node-local route points.
    fn locate(&self, points: &PointList) -> Result<LocatorPlacement, LocatorError>;
}

/// Draw2D-compatible source, target, and topological middle locator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionLocator {
    /// First route point.
    Source,
    /// Last route point.
    Target,
    /// Central point for odd counts, central segment midpoint for even counts.
    Middle,
}

impl ConnectionLocatorStrategy for ConnectionLocator {
    fn locate(&self, points: &PointList) -> Result<LocatorPlacement, LocatorError> {
        require_points(points, 2)?;
        match self {
            Self::Source => Ok(LocatorPlacement {
                point: points.get(0).expect("validated point count"),
                reference: points.get(1).expect("validated point count"),
            }),
            Self::Target => {
                let last = points.len() - 1;
                Ok(LocatorPlacement {
                    point: points.get(last).expect("validated point count"),
                    reference: points.get(last - 1).expect("validated point count"),
                })
            }
            Self::Middle if points.len() % 2 == 1 => {
                let middle = points.len() / 2;
                let point = points.get(middle).expect("validated point count");
                let reference = points
                    .get(middle.saturating_sub(1))
                    .unwrap_or_else(|| points.get(middle + 1).expect("validated point count"));
                Ok(LocatorPlacement { point, reference })
            }
            Self::Middle => {
                let right = points.len() / 2;
                segment_midpoint(points, right - 1)
            }
        }
    }
}

/// Midpoint of one indexed route segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidpointLocator {
    segment: usize,
}

impl MidpointLocator {
    /// Creates a Locator for `points[segment] -> points[segment + 1]`.
    pub const fn new(segment: usize) -> Self {
        Self { segment }
    }
}

impl ConnectionLocatorStrategy for MidpointLocator {
    fn locate(&self, points: &PointList) -> Result<LocatorPlacement, LocatorError> {
        segment_midpoint(points, self.segment)
    }
}

/// Arc-length fraction along a polyline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathFractionLocator {
    fraction: f64,
}

impl PathFractionLocator {
    /// Creates a Locator for a finite fraction in `[0, 1]`.
    pub fn new(fraction: f64) -> Result<Self, LocatorError> {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err(LocatorError::InvalidFraction);
        }
        Ok(Self { fraction })
    }
}

impl ConnectionLocatorStrategy for PathFractionLocator {
    fn locate(&self, points: &PointList) -> Result<LocatorPlacement, LocatorError> {
        require_points(points, 2)?;
        let lengths: Vec<_> = points
            .as_slice()
            .windows(2)
            .map(|segment| (segment[1] - segment[0]).length())
            .collect();
        let total: f64 = lengths.iter().sum();
        if total <= f64::EPSILON {
            return Err(LocatorError::DegenerateRoute);
        }
        let target = total * self.fraction;
        let mut traversed = 0.0;
        for (index, length) in lengths.iter().copied().enumerate() {
            if target <= traversed + length || index == lengths.len() - 1 {
                let start = points.get(index).expect("validated segment");
                let end = points.get(index + 1).expect("validated segment");
                let local = if length <= f64::EPSILON {
                    0.0
                } else {
                    ((target - traversed) / length).clamp(0.0, 1.0)
                };
                return Ok(LocatorPlacement {
                    point: start + (end - start) * local,
                    reference: start,
                });
            }
            traversed += length;
        }
        Err(LocatorError::DegenerateRoute)
    }
}

/// Locator calculation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocatorError {
    /// Route has fewer than the required number of points.
    TooFewPoints { point_count: usize },
    /// Indexed segment does not exist.
    SegmentOutOfRange { segment: usize, point_count: usize },
    /// Fraction is non-finite or outside `[0, 1]`.
    InvalidFraction,
    /// Route has no non-zero length.
    DegenerateRoute,
    /// Custom Locator returned a non-finite point or reference.
    NonFinitePlacement,
}

impl fmt::Display for LocatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewPoints { point_count } => {
                write!(formatter, "locator requires two points, got {point_count}")
            }
            Self::SegmentOutOfRange {
                segment,
                point_count,
            } => write!(
                formatter,
                "segment {segment} is outside a route with {point_count} points"
            ),
            Self::InvalidFraction => write!(formatter, "path fraction must be in [0, 1]"),
            Self::DegenerateRoute => write!(formatter, "route has no non-zero length"),
            Self::NonFinitePlacement => write!(formatter, "locator placement must be finite"),
        }
    }
}

impl Error for LocatorError {}

fn require_points(points: &PointList, minimum: usize) -> Result<(), LocatorError> {
    if points.len() < minimum {
        Err(LocatorError::TooFewPoints {
            point_count: points.len(),
        })
    } else {
        Ok(())
    }
}

fn segment_midpoint(points: &PointList, segment: usize) -> Result<LocatorPlacement, LocatorError> {
    require_points(points, 2)?;
    let Some(start) = points.get(segment) else {
        return Err(LocatorError::SegmentOutOfRange {
            segment,
            point_count: points.len(),
        });
    };
    let Some(end) = points.get(segment + 1) else {
        return Err(LocatorError::SegmentOutOfRange {
            segment,
            point_count: points.len(),
        });
    };
    Ok(LocatorPlacement {
        point: Point::new((start.x() + end.x()) / 2.0, (start.y() + end.y()) / 2.0),
        reference: start,
    })
}
