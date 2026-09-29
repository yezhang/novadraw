//! 颜色类型

use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// Identifies one RGBA component in a validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorError {
    /// A component was NaN or infinite.
    NonFiniteComponent {
        /// Component name.
        component: &'static str,
    },
    /// A component was outside the inclusive `[0, 1]` range.
    OutOfRangeComponent {
        /// Component name.
        component: &'static str,
    },
}

impl fmt::Display for ColorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteComponent { component } => {
                write!(formatter, "{component} color component must be finite")
            }
            Self::OutOfRangeComponent { component } => {
                write!(
                    formatter,
                    "{component} color component must be within [0, 1]"
                )
            }
        }
    }
}

impl Error for ColorError {}

/// A hexadecimal color string could not be parsed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseColorError {
    /// The string did not contain exactly six or eight hexadecimal digits.
    InvalidLength(usize),
    /// One color component contained a non-hexadecimal digit.
    InvalidDigit {
        /// Component whose two digits were invalid.
        component: &'static str,
    },
}

impl fmt::Display for ParseColorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength(length) => {
                write!(
                    formatter,
                    "hex color must contain 6 or 8 digits, got {length}"
                )
            }
            Self::InvalidDigit { component } => {
                write!(formatter, "invalid hexadecimal {component} component")
            }
        }
    }
}

impl Error for ParseColorError {}

#[derive(Deserialize)]
struct ColorComponents {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

/// RGBA 颜色类型.
///
/// Components always remain in the inclusive `[0, 1]` range.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ColorComponents")]
pub struct Color {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl Color {
    /// Parses a six- or eight-digit hexadecimal color.
    ///
    /// A leading `#` is optional. Eight-digit input uses `RRGGBBAA` order.
    ///
    /// # 示例
    ///
    /// ```
    /// let red = novadraw::Color::from_hex("#ff0000")?;
    /// let with_alpha = novadraw::Color::from_hex("#ff000080")?;
    /// # Ok::<(), novadraw::ParseColorError>(())
    /// ```
    #[inline]
    pub fn from_hex(hex: &str) -> Result<Self, ParseColorError> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() != 6 && hex.len() != 8 {
            return Err(ParseColorError::InvalidLength(hex.len()));
        }
        let component = |range: std::ops::Range<usize>, name| {
            u8::from_str_radix(&hex[range], 16)
                .map_err(|_| ParseColorError::InvalidDigit { component: name })
        };
        let r = component(0..2, "red")?;
        let g = component(2..4, "green")?;
        let b = component(4..6, "blue")?;
        let a = if hex.len() == 8 {
            component(6..8, "alpha")?
        } else {
            u8::MAX
        };
        Ok(Self::rgba8(r, g, b, a))
    }

    /// Creates a color and saturates every component into `[0, 1]`.
    ///
    /// NaN is normalized to zero. Use [`Self::try_rgba`] when invalid input
    /// must be rejected instead of normalized.
    #[inline]
    pub const fn rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self {
            r: saturate_component(r),
            g: saturate_component(g),
            b: saturate_component(b),
            a: saturate_component(a),
        }
    }

    /// Creates a color from validated floating-point components.
    pub fn try_rgba(r: f64, g: f64, b: f64, a: f64) -> Result<Self, ColorError> {
        validate_component("red", r)?;
        validate_component("green", g)?;
        validate_component("blue", b)?;
        validate_component("alpha", a)?;
        Ok(Self { r, g, b, a })
    }

    /// Creates a color from eight-bit RGBA components.
    #[inline]
    pub const fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        const SCALE: f64 = 1.0 / u8::MAX as f64;
        Self {
            r: r as f64 * SCALE,
            g: g as f64 * SCALE,
            b: b as f64 * SCALE,
            a: a as f64 * SCALE,
        }
    }

    /// Returns the red component.
    #[inline]
    pub const fn red(self) -> f64 {
        self.r
    }

    /// Returns the green component.
    #[inline]
    pub const fn green(self) -> f64 {
        self.g
    }

    /// Returns the blue component.
    #[inline]
    pub const fn blue(self) -> f64 {
        self.b
    }

    /// Returns the alpha component.
    #[inline]
    pub const fn alpha(self) -> f64 {
        self.a
    }

    /// Returns the RGBA components in channel order.
    #[inline]
    pub const fn to_rgba_array(self) -> [f64; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// 红色
    pub const RED: Color = Color::rgba8(255, 0, 0, 255);

    /// 绿色
    pub const GREEN: Color = Color::rgba8(0, 255, 0, 255);

    /// 蓝色
    pub const BLUE: Color = Color::rgba8(0, 0, 255, 255);

    /// 白色
    pub const WHITE: Color = Color::rgba8(255, 255, 255, 255);

    /// 黑色
    pub const BLACK: Color = Color::rgba8(0, 0, 0, 255);

    /// 透明
    pub const TRANSPARENT: Color = Color::rgba8(0, 0, 0, 0);

    /// Returns this color with a saturated alpha component.
    #[inline]
    pub const fn with_alpha(self, alpha: f64) -> Self {
        Self {
            a: saturate_component(alpha),
            ..self
        }
    }

    /// 检查是否完全透明
    #[inline]
    pub fn is_transparent(self) -> bool {
        self.a <= 0.0
    }

    /// 检查是否完全不透明
    #[inline]
    pub fn is_opaque(self) -> bool {
        self.a >= 1.0
    }
}

impl TryFrom<ColorComponents> for Color {
    type Error = ColorError;

    fn try_from(value: ColorComponents) -> Result<Self, Self::Error> {
        Self::try_rgba(value.r, value.g, value.b, value.a)
    }
}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_hex(value)
    }
}

impl Default for Color {
    #[inline]
    fn default() -> Self {
        Color::BLACK
    }
}

const fn saturate_component(value: f64) -> f64 {
    if value.is_nan() || value < 0.0 {
        0.0
    } else if value > 1.0 {
        1.0
    } else {
        value
    }
}

fn validate_component(component: &'static str, value: f64) -> Result<(), ColorError> {
    if !value.is_finite() {
        return Err(ColorError::NonFiniteComponent { component });
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(ColorError::OutOfRangeComponent { component });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_6_digits() {
        let red = Color::from_hex("#ff0000").unwrap();
        assert!((red.red() - 1.0).abs() < 1e-10);
        assert!((red.green() - 0.0).abs() < 1e-10);
        assert!((red.blue() - 0.0).abs() < 1e-10);
        assert!((red.alpha() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_hex_8_digits() {
        let red_half = Color::from_hex("#ff000080").unwrap();
        assert!((red_half.red() - 1.0).abs() < 1e-10);
        assert!((red_half.alpha() - 128.0 / 255.0).abs() < 1e-10);
    }

    #[test]
    fn test_rgba() {
        let color = Color::rgba(0.5, 0.25, 0.75, 0.8);
        assert!((color.red() - 0.5).abs() < 1e-10);
        assert!((color.green() - 0.25).abs() < 1e-10);
        assert!((color.blue() - 0.75).abs() < 1e-10);
        assert!((color.alpha() - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_with_alpha() {
        let red = Color::from_hex("#ff0000").unwrap();
        let red_half = red.with_alpha(0.5);
        assert!((red_half.alpha() - 0.5).abs() < 1e-10);
        assert!((red_half.red() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_constants() {
        assert_eq!(Color::RED.red(), 1.0);
        assert_eq!(Color::GREEN.green(), 1.0);
        assert_eq!(Color::BLUE.blue(), 1.0);
        assert_eq!(Color::WHITE.red(), 1.0);
        assert_eq!(Color::BLACK.red(), 0.0);
    }

    #[test]
    fn invalid_hex_is_rejected_without_panicking() {
        assert_eq!(
            Color::from_hex("#12345"),
            Err(ParseColorError::InvalidLength(5))
        );
        assert_eq!(
            Color::from_hex("#12xx56"),
            Err(ParseColorError::InvalidDigit { component: "green" })
        );
    }

    #[test]
    fn rgba_clamps_every_component_to_the_public_invariant() {
        let color = Color::rgba(-1.0, 2.0, f64::NAN, f64::INFINITY);

        assert_eq!(color.to_rgba_array(), [0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn try_rgba_rejects_non_finite_and_out_of_range_components() {
        assert_eq!(
            Color::try_rgba(f64::NAN, 0.0, 0.0, 1.0),
            Err(ColorError::NonFiniteComponent { component: "red" })
        );
        assert_eq!(
            Color::try_rgba(0.0, 0.0, 0.0, 1.1),
            Err(ColorError::OutOfRangeComponent { component: "alpha" })
        );
    }

    #[test]
    fn serde_rejects_components_outside_the_public_invariant() {
        let result = serde_json::from_str::<Color>(r#"{"r":0.0,"g":0.5,"b":2.0,"a":1.0}"#);

        assert!(result.is_err());
    }
}
