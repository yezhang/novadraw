//! Checked, backend-neutral stroke values.

use std::{borrow::Cow, fmt, sync::Arc};

use super::command::{DEFAULT_STROKE_MITER_LIMIT, LineCap, LineJoin, LineStyle};

const DEFAULT_STROKE_WIDTH: f64 = 1.0;
const DASH_WIDTH_FACTORS: [f64; 2] = [3.0, 1.0];
const DOT_WIDTH_FACTORS: [f64; 2] = [1.0, 1.0];

/// Invalid input rejected before modifying Graphics state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphicsInputError {
    NonFinite {
        field: &'static str,
        index: Option<usize>,
    },
    NegativeStrokeWidth,
    InvalidMiterLimit,
    EmptyDashPattern,
    InvalidDashLength {
        index: usize,
    },
    DashPeriodOverflow,
    StrokeExtentOverflow,
    InvalidPath {
        index: usize,
    },
    GradientStopOutOfRange,
    DegenerateGradient,
    InsufficientGradientStops,
    GradientEndpointStops,
    GradientStopOrder {
        index: usize,
    },
    DeviceValueOverflow {
        field: &'static str,
    },
    DeviceValueUnderflow {
        field: &'static str,
    },
    InvalidDeviceScale,
    DegenerateDeviceGradient,
}

impl fmt::Display for GraphicsInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { field, index } => write!(f, "{field} at {index:?} must be finite"),
            Self::NegativeStrokeWidth => f.write_str("stroke width must be non-negative"),
            Self::InvalidMiterLimit => f.write_str("miter limit must be at least one"),
            Self::EmptyDashPattern => f.write_str("custom dash pattern must not be empty"),
            Self::InvalidDashLength { index } => {
                write!(f, "dash length at {index} must be positive")
            }
            Self::DashPeriodOverflow => f.write_str("dash period must be finite"),
            Self::StrokeExtentOverflow => f.write_str("stroke extent must be finite"),
            Self::InvalidPath { index } => write!(f, "invalid path operation at {index}"),
            Self::GradientStopOutOfRange => f.write_str("gradient offset must be in [0, 1]"),
            Self::DegenerateGradient => f.write_str("gradient endpoints must differ"),
            Self::InsufficientGradientStops => f.write_str("gradient needs at least two stops"),
            Self::GradientEndpointStops => {
                f.write_str("gradient stops must start at 0 and end at 1")
            }
            Self::GradientStopOrder { index } => {
                write!(f, "gradient stop at {index} is out of order")
            }
            Self::DeviceValueOverflow { field } => {
                write!(f, "{field} exceeds device numeric range")
            }
            Self::DeviceValueUnderflow { field } => {
                write!(f, "{field} collapses in device precision")
            }
            Self::InvalidDeviceScale => {
                f.write_str("device scale must be positive and representable")
            }
            Self::DegenerateDeviceGradient => f.write_str("gradient collapses in device precision"),
        }
    }
}

impl std::error::Error for GraphicsInputError {}

pub(super) fn finite(value: f64, field: &'static str) -> Result<(), GraphicsInputError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(GraphicsInputError::NonFinite { field, index: None })
    }
}

/// An owned, even-length dash period in Canvas logical units.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomDash {
    lengths: Arc<[f64]>,
    period: f64,
}

impl CustomDash {
    pub fn try_new(lengths: &[f64]) -> Result<Self, GraphicsInputError> {
        if lengths.is_empty() {
            return Err(GraphicsInputError::EmptyDashPattern);
        }
        let mut period = 0.0;
        for (index, &length) in lengths.iter().enumerate() {
            if !length.is_finite() {
                return Err(GraphicsInputError::NonFinite {
                    field: "dash length",
                    index: Some(index),
                });
            }
            if length <= 0.0 {
                return Err(GraphicsInputError::InvalidDashLength { index });
            }
            period += length;
        }
        let odd = !lengths.len().is_multiple_of(2);
        if odd {
            period *= 2.0;
        }
        if !period.is_finite() {
            return Err(GraphicsInputError::DashPeriodOverflow);
        }
        let lengths = if odd {
            lengths
                .iter()
                .chain(lengths)
                .copied()
                .collect::<Arc<[f64]>>()
        } else {
            Arc::from(lengths)
        };
        Ok(Self { lengths, period })
    }

    pub fn lengths(&self) -> &[f64] {
        &self.lengths
    }
    pub fn period(&self) -> f64 {
        self.period
    }
}

/// Builtin patterns scale with width; custom lengths do not.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum DashPattern {
    #[default]
    Solid,
    Dash,
    Dot,
    Custom(CustomDash),
}

impl From<LineStyle> for DashPattern {
    fn from(style: LineStyle) -> Self {
        match style {
            LineStyle::Solid => Self::Solid,
            LineStyle::Dash => Self::Dash,
            LineStyle::Dot => Self::Dot,
        }
    }
}

/// Complete immutable stroke configuration. Construction validates all numeric fields.
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    width: f64,
    cap: LineCap,
    join: LineJoin,
    dash: DashPattern,
    dash_offset: f64,
    miter_limit: f64,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            width: DEFAULT_STROKE_WIDTH,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dash: DashPattern::Solid,
            dash_offset: 0.0,
            miter_limit: DEFAULT_STROKE_MITER_LIMIT,
        }
    }
}

impl StrokeStyle {
    pub fn try_new(
        width: f64,
        cap: LineCap,
        join: LineJoin,
        dash: DashPattern,
        dash_offset: f64,
        miter_limit: f64,
    ) -> Result<Self, GraphicsInputError> {
        finite(width, "stroke width")?;
        finite(dash_offset, "dash offset")?;
        finite(miter_limit, "miter limit")?;
        if width < 0.0 {
            return Err(GraphicsInputError::NegativeStrokeWidth);
        }
        if miter_limit < 1.0 {
            return Err(GraphicsInputError::InvalidMiterLimit);
        }
        // Validate even inactive values so later infallible cap/join/pattern changes remain safe.
        if !(width * DASH_WIDTH_FACTORS.iter().sum::<f64>()).is_finite() {
            return Err(GraphicsInputError::DashPeriodOverflow);
        }
        if !(width / 2.0 * miter_limit.max(std::f64::consts::SQRT_2)).is_finite() {
            return Err(GraphicsInputError::StrokeExtentOverflow);
        }
        Ok(Self {
            width,
            cap,
            join,
            dash,
            dash_offset,
            miter_limit,
        })
    }

    pub fn width(&self) -> f64 {
        self.width
    }
    pub fn cap(&self) -> LineCap {
        self.cap
    }
    pub fn join(&self) -> LineJoin {
        self.join
    }
    pub fn dash_pattern(&self) -> &DashPattern {
        &self.dash
    }
    pub fn dash_offset(&self) -> f64 {
        self.dash_offset
    }
    pub fn miter_limit(&self) -> f64 {
        self.miter_limit
    }

    pub fn with_width(&self, width: f64) -> Result<Self, GraphicsInputError> {
        Self::try_new(
            width,
            self.cap,
            self.join,
            self.dash.clone(),
            self.dash_offset,
            self.miter_limit,
        )
    }

    pub fn with_miter_limit(&self, miter_limit: f64) -> Result<Self, GraphicsInputError> {
        Self::try_new(
            self.width,
            self.cap,
            self.join,
            self.dash.clone(),
            self.dash_offset,
            miter_limit,
        )
    }

    pub fn with_dash_offset(&self, dash_offset: f64) -> Result<Self, GraphicsInputError> {
        finite(dash_offset, "dash offset")?;
        let mut result = self.clone();
        result.dash_offset = dash_offset;
        Ok(result)
    }

    pub fn with_cap(mut self, cap: LineCap) -> Self {
        self.cap = cap;
        self
    }
    pub fn with_join(mut self, join: LineJoin) -> Self {
        self.join = join;
        self
    }
    pub fn with_dash_pattern(mut self, dash: DashPattern) -> Self {
        self.dash = dash;
        self
    }

    /// Effective dash lengths, before applying the drawing transform and DPI.
    pub fn dash_lengths(&self) -> Cow<'_, [f64]> {
        match &self.dash {
            DashPattern::Solid => Cow::Borrowed(&[]),
            DashPattern::Dash => Cow::Owned(DASH_WIDTH_FACTORS.map(|n| n * self.width).to_vec()),
            DashPattern::Dot => Cow::Owned(DOT_WIDTH_FACTORS.map(|n| n * self.width).to_vec()),
            DashPattern::Custom(dash) => Cow::Borrowed(dash.lengths()),
        }
    }

    pub fn normalized_dash_offset(&self) -> f64 {
        let period = match &self.dash {
            DashPattern::Solid => 0.0,
            DashPattern::Dash => self.width * DASH_WIDTH_FACTORS.iter().sum::<f64>(),
            DashPattern::Dot => self.width * DOT_WIDTH_FACTORS.iter().sum::<f64>(),
            DashPattern::Custom(dash) => dash.period(),
        };
        if period > 0.0 {
            self.dash_offset.rem_euclid(period)
        } else {
            0.0
        }
    }

    /// Conservative axis-aligned expansion around the centerline.
    pub fn visual_outset(&self) -> f64 {
        let join_factor = match self.join {
            LineJoin::Miter => self.miter_limit,
            LineJoin::Round | LineJoin::Bevel => 1.0,
        };
        let cap_factor = match self.cap {
            LineCap::Square => std::f64::consts::SQRT_2,
            LineCap::Butt | LineCap::Round => 1.0,
        };
        self.width / 2.0 * join_factor.max(cap_factor)
    }

    pub fn requires_custom_strokes(&self) -> bool {
        self.miter_limit != DEFAULT_STROKE_MITER_LIMIT
            || matches!(self.dash, DashPattern::Custom(_))
            || self.normalized_dash_offset() != 0.0
    }
}
