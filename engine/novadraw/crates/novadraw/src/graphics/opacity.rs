use super::GraphicsInputError;

/// Checked opacity shared by Figure styles, Graphics and animation channels.
///
/// The scalar cannot bypass validation:
/// ```compile_fail
/// let opacity = novadraw::graphics::Opacity(2.0);
/// ```
/// Styles require the checked value:
/// ```compile_fail
/// let style = novadraw::figure::FigureStyle {
///     alpha: Some(0.5),
///     ..Default::default()
/// };
/// ```
/// The canonical import belongs to Graphics:
/// ```compile_fail
/// use novadraw::animation::Opacity;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Opacity(f64);

impl Opacity {
    /// Fully transparent.
    pub const TRANSPARENT: Self = Self(0.0);
    /// Fully opaque.
    pub const OPAQUE: Self = Self(1.0);

    /// Rejects non-finite values and values outside the inclusive range `[0, 1]`.
    pub fn try_new(value: f64) -> Result<Self, GraphicsInputError> {
        if !value.is_finite() {
            return Err(GraphicsInputError::NonFinite {
                field: "opacity",
                index: None,
            });
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(GraphicsInputError::OpacityOutOfRange);
        }
        Ok(Self(value))
    }

    /// Returns the validated scalar.
    pub const fn get(self) -> f64 {
        self.0
    }
}
