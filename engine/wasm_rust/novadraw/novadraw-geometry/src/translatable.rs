//! 可变形类型 Trait
//!
//! 提供几何对象的平移和缩放能力。
//! 对应 draw2d: org.eclipse.draw2d.geometry.Translatable

use super::{Point, Rectangle, Size};

/// 内边距类型
///
/// 表示矩形的四个方向的内边距值。
/// 对应 draw2d: org.eclipse.draw2d.geometry.Insets
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    /// 上边距
    pub top: f64,
    /// 左边距
    pub left: f64,
    /// 下边距
    pub bottom: f64,
    /// 右边距
    pub right: f64,
}

impl Insets {
    /// 创建内边距
    ///
    /// # Arguments
    ///
    /// * `top` - 上边距
    /// * `left` - 左边距
    /// * `bottom` - 下边距
    /// * `right` - 右边距
    #[inline]
    pub fn new(top: f64, left: f64, bottom: f64, right: f64) -> Self {
        Self {
            left,
            top,
            bottom,
            right,
        }
    }

    /// 等值内边距
    ///
    /// 所有方向使用相同的边距值。
    #[inline]
    pub fn uniform(value: f64) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    /// 零内边距
    pub const ZERO: Insets = Insets {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };

    /// 内边距高度
    ///
    /// 返回 top + bottom
    #[inline]
    pub fn height(&self) -> f64 {
        self.top + self.bottom
    }

    /// 内边距宽度
    ///
    /// 返回 left + right
    #[inline]
    pub fn width(&self) -> f64 {
        self.left + self.right
    }
}

impl std::fmt::Display for Insets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Insets(t={:.4}, l={:.4}, b={:.4}, r={:.4})",
            self.top, self.left, self.bottom, self.right
        )
    }
}

/// 可变形 Trait
///
/// 支持平移和缩放操作的几何类型。
/// 对应 draw2d: org.eclipse.draw2d.geometry.Translatable
pub trait Translatable {
    /// 水平移动 `dx`，垂直移动 `dy`
    fn translate(&mut self, dx: f64, dy: f64);

    /// 按比例缩放
    fn scale(&mut self, factor: f64);

    /// 应用二维仿射变换。
    fn transform(&mut self, transform: crate::Affine2D);

    /// 通过 Point 平移
    ///
    /// 默认实现调用 `translate(point.x(), point.y())`
    #[inline]
    fn translate_by_point(&mut self, point: Point) {
        self.translate(point.x(), point.y());
    }

    /// 通过 Size 平移
    ///
    /// 默认实现调用 `translate(size.width, size.height)`
    #[inline]
    fn translate_by_size(&mut self, size: Size) {
        self.translate(size.width, size.height);
    }

    /// 通过 Insets 平移
    ///
    /// 默认实现调用 `translate(insets.left, insets.top)`
    #[inline]
    fn translate_by_insets(&mut self, insets: Insets) {
        self.translate(insets.left, insets.top);
    }
}

impl Translatable for Point {
    #[inline]
    fn translate(&mut self, dx: f64, dy: f64) {
        self.0.x += dx;
        self.0.y += dy;
    }

    #[inline]
    fn scale(&mut self, factor: f64) {
        self.0.x *= factor;
        self.0.y *= factor;
    }

    #[inline]
    fn transform(&mut self, transform: crate::Affine2D) {
        let (x, y) = transform.transform_point(self.x(), self.y());
        self.0.x = x;
        self.0.y = y;
    }
}

impl Translatable for Rectangle {
    #[inline]
    fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }

    #[inline]
    fn scale(&mut self, factor: f64) {
        self.x *= factor;
        self.y *= factor;
        self.width *= factor;
        self.height *= factor;
    }

    #[inline]
    fn transform(&mut self, transform: crate::Affine2D) {
        let corners = [
            transform.transform_point(self.x, self.y),
            transform.transform_point(self.x + self.width, self.y),
            transform.transform_point(self.x, self.y + self.height),
            transform.transform_point(self.x + self.width, self.y + self.height),
        ];
        let min_x = corners
            .iter()
            .map(|point| point.0)
            .fold(f64::INFINITY, f64::min);
        let min_y = corners
            .iter()
            .map(|point| point.1)
            .fold(f64::INFINITY, f64::min);
        let max_x = corners
            .iter()
            .map(|point| point.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = corners
            .iter()
            .map(|point| point.1)
            .fold(f64::NEG_INFINITY, f64::max);
        *self = Rectangle::new(min_x, min_y, max_x - min_x, max_y - min_y);
    }
}

impl Translatable for (f64, f64) {
    #[inline]
    fn translate(&mut self, dx: f64, dy: f64) {
        self.0 += dx;
        self.1 += dy;
    }

    #[inline]
    fn scale(&mut self, factor: f64) {
        self.0 *= factor;
        self.1 *= factor;
    }

    #[inline]
    fn transform(&mut self, transform: crate::Affine2D) {
        *self = transform.transform_point(self.0, self.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_translate() {
        let mut p = Point::new(10.0, 20.0);
        p.translate(5.0, 10.0);
        assert_eq!(p.x(), 15.0);
        assert_eq!(p.y(), 30.0);
    }

    #[test]
    fn test_point_scale() {
        let mut p = Point::new(10.0, 20.0);
        p.scale(2.0);
        assert_eq!(p.x(), 20.0);
        assert_eq!(p.y(), 40.0);
    }

    #[test]
    fn test_point_translate_by_point() {
        let mut p = Point::new(10.0, 20.0);
        p.translate_by_point(Point::new(5.0, 10.0));
        assert_eq!(p.x(), 15.0);
        assert_eq!(p.y(), 30.0);
    }

    #[test]
    fn test_rectangle_translate() {
        let mut r = Rectangle::new(10.0, 20.0, 100.0, 50.0);
        r.translate(5.0, 10.0);
        assert_eq!(r.x, 15.0);
        assert_eq!(r.y, 30.0);
        assert_eq!(r.width, 100.0);
        assert_eq!(r.height, 50.0);
    }

    #[test]
    fn test_rectangle_scale() {
        let mut r = Rectangle::new(10.0, 20.0, 100.0, 50.0);
        r.scale(2.0);
        assert_eq!(r.x, 20.0);
        assert_eq!(r.y, 40.0);
        assert_eq!(r.width, 200.0);
        assert_eq!(r.height, 100.0);
    }

    #[test]
    fn test_rectangle_translate_by_size() {
        let mut r = Rectangle::new(10.0, 20.0, 100.0, 50.0);
        r.translate_by_size(Size::new(5.0, 10.0));
        assert_eq!(r.x, 15.0);
        assert_eq!(r.y, 30.0);
    }

    #[test]
    fn rectangle_affine_transform_returns_conservative_aabb() {
        let mut rect = Rectangle::new(0.0, 0.0, 20.0, 10.0);
        rect.transform(crate::Affine2D::from_rotation(std::f64::consts::FRAC_PI_2));

        assert!((rect.x + 10.0).abs() < 1e-10);
        assert!(rect.y.abs() < 1e-10);
        assert!((rect.width - 10.0).abs() < 1e-10);
        assert!((rect.height - 20.0).abs() < 1e-10);
    }
}
