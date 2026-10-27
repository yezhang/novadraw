//! 几何精度工具。

use super::{Affine2D, Point, Rectangle};

/// 默认几何比较精度。
pub const DEFAULT_EPSILON: f64 = 1.0e-9;

/// 几何精度配置。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Precision {
    epsilon: f64,
}

impl Precision {
    /// 使用指定 epsilon 创建精度配置。
    #[inline]
    pub const fn new(epsilon: f64) -> Self {
        Self { epsilon }
    }

    /// 默认精度配置。
    pub const DEFAULT: Self = Self {
        epsilon: DEFAULT_EPSILON,
    };

    /// 返回 epsilon。
    #[inline]
    pub const fn epsilon(self) -> f64 {
        self.epsilon
    }

    /// 比较两个浮点数是否在当前精度内相等。
    #[inline]
    pub fn eq(self, a: f64, b: f64) -> bool {
        (a - b).abs() <= self.epsilon
    }

    /// 判断浮点数是否接近 0。
    #[inline]
    pub fn is_zero(self, value: f64) -> bool {
        value.abs() <= self.epsilon
    }

    /// 将接近 0 的浮点数吸附为 0。
    #[inline]
    pub fn snap_zero(self, value: f64) -> f64 {
        if self.is_zero(value) { 0.0 } else { value }
    }
}

impl Default for Precision {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// 支持几何近似相等比较的类型。
pub trait ApproxEq<Rhs = Self> {
    /// 使用指定精度比较两个值是否近似相等。
    fn approx_eq(self, other: Rhs, precision: Precision) -> bool;

    /// 使用默认精度比较两个值是否近似相等。
    #[inline]
    fn approx_eq_default(self, other: Rhs) -> bool
    where
        Self: Sized,
    {
        self.approx_eq(other, Precision::DEFAULT)
    }
}

impl ApproxEq for f64 {
    #[inline]
    fn approx_eq(self, other: f64, precision: Precision) -> bool {
        precision.eq(self, other)
    }
}

impl ApproxEq for Point {
    #[inline]
    fn approx_eq(self, other: Point, precision: Precision) -> bool {
        self.x().approx_eq(other.x(), precision) && self.y().approx_eq(other.y(), precision)
    }
}

impl ApproxEq for Rectangle {
    #[inline]
    fn approx_eq(self, other: Rectangle, precision: Precision) -> bool {
        self.x.approx_eq(other.x, precision)
            && self.y.approx_eq(other.y, precision)
            && self.width.approx_eq(other.width, precision)
            && self.height.approx_eq(other.height, precision)
    }
}

impl ApproxEq for Affine2D {
    #[inline]
    fn approx_eq(self, other: Affine2D, precision: Precision) -> bool {
        self.coeffs()
            .into_iter()
            .zip(other.coeffs())
            .all(|(left, right)| left.approx_eq(right, precision))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precision_compares_floats_within_epsilon() {
        let precision = Precision::new(0.001);

        assert!(precision.eq(10.0, 10.0005));
        assert!(!precision.eq(10.0, 10.002));
    }

    #[test]
    fn precision_snaps_near_zero_values() {
        let precision = Precision::new(0.001);

        assert_eq!(precision.snap_zero(0.0005), 0.0);
        assert_eq!(precision.snap_zero(0.002), 0.002);
    }

    #[test]
    fn point_rectangle_and_transform_support_approx_eq() {
        let precision = Precision::new(0.001);

        assert!(Point::new(1.0, 2.0).approx_eq(Point::new(1.0005, 1.9995), precision));
        assert!(
            Rectangle::new(1.0, 2.0, 3.0, 4.0)
                .approx_eq(Rectangle::new(1.0005, 1.9995, 3.0004, 4.0004), precision,)
        );
        assert!(Affine2D::from_translation(10.0, 20.0).approx_eq(
            Affine2D::new(1.0, 0.0, 0.0, 1.0, 10.0005, 19.9995),
            precision,
        ));
    }
}
