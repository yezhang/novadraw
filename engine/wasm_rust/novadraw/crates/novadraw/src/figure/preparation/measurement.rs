use std::{error::Error, fmt};

use crate::{
    Dimension, FigureId, FigureMeasurement, MeasureConstraints, layout::LayoutError,
    text::TextError,
};

/// Content metrics computed for explicit constraints, without preparing drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparationMeasurement {
    pub preferred: FigureMeasurement,
    pub minimum: Dimension,
}

/// Metric rejected by the authoritative measurement entry point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationMetric {
    Preferred,
    Minimum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MeasurementSelection {
    Both,
    Preferred,
    Minimum,
}

impl MeasurementSelection {
    pub(crate) fn preferred(self) -> bool {
        self != Self::Minimum
    }

    pub(crate) fn minimum(self) -> bool {
        self != Self::Preferred
    }
}

/// Checked geometry stage at which finite input overflowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationMeasureStage {
    ConstraintShrink,
    ConstraintInverseProjection,
    BorderExpansion,
    MeasurementProjection,
}

/// Failure reason, retaining the original callback or layout error.
#[derive(Clone, Debug)]
pub enum PreparationMeasureErrorKind {
    UnsupportedConstraintMeasurement,
    InvalidSize {
        metric: PreparationMetric,
        value: Dimension,
    },
    InvalidBaseline {
        baseline: f64,
        height: f64,
    },
    DerivedOverflow {
        stage: PreparationMeasureStage,
    },
    Layout(Box<LayoutError>),
    Text(Box<TextError>),
    Dependency(Box<PreparationDependencyError>),
}

/// Ordered measurement dependency path rejected before invoking a callback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparationDependencyError {
    Budget(super::PreparationBudgetError),
    MeasurementCycle {
        path: Vec<FigureId>,
    },
    DependencyCycle {
        path: Vec<crate::connection::DependencySubject>,
    },
    ForbiddenRouteOutputDependency {
        owner: FigureId,
        connection: crate::connection::ConnectionId,
        subject: crate::connection::DependencySubject,
    },
    DepthExceeded {
        limit: usize,
    },
}

impl fmt::Display for PreparationDependencyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "preparation dependency rejected: {self:?}")
    }
}
impl Error for PreparationDependencyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            _ => None,
        }
    }
}

impl PartialEq for PreparationMeasureErrorKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::UnsupportedConstraintMeasurement, Self::UnsupportedConstraintMeasurement) => {
                true
            }
            (
                Self::InvalidSize {
                    metric: a,
                    value: av,
                },
                Self::InvalidSize {
                    metric: b,
                    value: bv,
                },
            ) => {
                a == b
                    && av.width.to_bits() == bv.width.to_bits()
                    && av.height.to_bits() == bv.height.to_bits()
            }
            (
                Self::InvalidBaseline {
                    baseline: a,
                    height: ah,
                },
                Self::InvalidBaseline {
                    baseline: b,
                    height: bh,
                },
            ) => a.to_bits() == b.to_bits() && ah.to_bits() == bh.to_bits(),
            (Self::DerivedOverflow { stage: a }, Self::DerivedOverflow { stage: b }) => a == b,
            (Self::Layout(a), Self::Layout(b)) => a == b,
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Dependency(a), Self::Dependency(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for PreparationMeasureErrorKind {}

/// Error at the actual failing Figure, with its outer and content constraints.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparationMeasureError {
    pub figure: FigureId,
    pub outer_constraints: MeasureConstraints,
    pub content_constraints: Option<MeasureConstraints>,
    pub kind: PreparationMeasureErrorKind,
}

// MeasureConstraints admits only finite non-negative limits or unbounded axes.
impl Eq for PreparationMeasureError {}

impl fmt::Display for PreparationMeasureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "constraint measurement failed for {:?} under {:?}: {:?}",
            self.figure, self.outer_constraints, self.kind
        )
    }
}

impl Error for PreparationMeasureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            PreparationMeasureErrorKind::Layout(error) => Some(error.as_ref()),
            PreparationMeasureErrorKind::Text(error) => Some(error.as_ref()),
            PreparationMeasureErrorKind::Dependency(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<PreparationMeasureError> for TextError {
    fn from(error: PreparationMeasureError) -> Self {
        Self::PreparationMeasurement(Box::new(error))
    }
}

impl PreparationMeasurement {
    pub(crate) fn validate(
        self,
        figure: FigureId,
        outer_constraints: MeasureConstraints,
        content_constraints: Option<MeasureConstraints>,
        preferred: bool,
        minimum: bool,
    ) -> Result<Self, PreparationMeasureError> {
        let error = |kind| PreparationMeasureError {
            figure,
            outer_constraints,
            content_constraints,
            kind,
        };
        for (metric, value) in [
            (PreparationMetric::Preferred, self.preferred.size()),
            (PreparationMetric::Minimum, self.minimum),
        ] {
            let selected = match metric {
                PreparationMetric::Preferred => preferred,
                PreparationMetric::Minimum => minimum,
            };
            if selected
                && (!value.width.is_finite()
                    || !value.height.is_finite()
                    || value.width < 0.0
                    || value.height < 0.0)
            {
                return Err(error(PreparationMeasureErrorKind::InvalidSize {
                    metric,
                    value,
                }));
            }
        }
        if preferred
            && let Some(baseline) = self.preferred.baseline
            && (!baseline.is_finite() || baseline < 0.0 || baseline > self.preferred.height)
        {
            return Err(error(PreparationMeasureErrorKind::InvalidBaseline {
                baseline,
                height: self.preferred.height,
            }));
        }
        Ok(self)
    }
}
