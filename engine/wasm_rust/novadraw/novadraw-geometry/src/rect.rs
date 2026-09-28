//! 矩形、点、尺寸类型

use serde::{Deserialize, Serialize};

use super::Point;

/// 尺寸类型
///
/// 对应 draw2d: org.eclipse.draw2d.geometry.Dimension。
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "DimensionSerde", into = "DimensionSerde")]
pub struct Dimension {
    /// 宽度
    pub width: f64,
    /// 高度
    pub height: f64,
}

impl Dimension {
    /// 创建新尺寸
    #[inline]
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    /// 空尺寸
    pub const ZERO: Dimension = Dimension {
        width: 0.0,
        height: 0.0,
    };

    /// 检查是否为空
    #[inline]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// 面积
    #[inline]
    pub fn area(self) -> f64 {
        self.width * self.height
    }
}

impl From<(f64, f64)> for Dimension {
    #[inline]
    fn from((width, height): (f64, f64)) -> Self {
        Dimension::new(width, height)
    }
}

impl From<Dimension> for (f64, f64) {
    #[inline]
    fn from(dimension: Dimension) -> Self {
        (dimension.width, dimension.height)
    }
}

impl std::fmt::Display for Dimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dimension({:.4} × {:.4})", self.width, self.height)
    }
}

#[derive(Serialize, Deserialize)]
struct DimensionSerde(f64, f64);

impl From<DimensionSerde> for Dimension {
    fn from(val: DimensionSerde) -> Self {
        Dimension::new(val.0, val.1)
    }
}

impl From<Dimension> for DimensionSerde {
    fn from(val: Dimension) -> Self {
        DimensionSerde(val.width, val.height)
    }
}

/// 矩形类型
///
/// 使用左上角坐标 (x, y) 和宽高 (width, height) 定义。
/// 对应 draw2d: org.eclipse.draw2d.geometry.Rectangle
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "RectangleSerde", into = "RectangleSerde")]
pub struct Rectangle {
    /// 左上角 X 坐标
    pub x: f64,
    /// 左上角 Y 坐标
    pub y: f64,
    /// 宽度
    pub width: f64,
    /// 高度
    pub height: f64,
}

impl Rectangle {
    /// 创建新矩形
    #[inline]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 从两个角点创建
    ///
    /// 自动处理任意角点顺序，返回包含两点的最小矩形。
    #[inline]
    pub fn from_corners(corner1: Point, corner2: Point) -> Self {
        let x = corner1.x().min(corner2.x());
        let y = corner1.y().min(corner2.y());
        let width = (corner2.x() - corner1.x()).abs();
        let height = (corner2.y() - corner1.y()).abs();
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 从中心点和尺寸创建
    #[inline]
    pub fn from_center(center: Point, width: f64, height: f64) -> Self {
        Self {
            x: center.x() - width / 2.0,
            y: center.y() - height / 2.0,
            width,
            height,
        }
    }

    /// 空矩形
    pub const ZERO: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    /// 左上角
    #[inline]
    pub fn top_left(self) -> Point {
        Point::new(self.x, self.y)
    }

    /// 右上角
    #[inline]
    pub fn top_right(self) -> Point {
        Point::new(self.x + self.width, self.y)
    }

    /// 左下角
    #[inline]
    pub fn bottom_left(self) -> Point {
        Point::new(self.x, self.y + self.height)
    }

    /// 右下角
    #[inline]
    pub fn bottom_right(self) -> Point {
        Point::new(self.x + self.width, self.y + self.height)
    }

    /// 中心点
    #[inline]
    pub fn center(self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// 检查是否为空
    #[inline]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// 检查点是否在矩形内（包含边界）
    #[inline]
    pub fn contains(self, point: Point) -> bool {
        point.x() >= self.x
            && point.x() <= self.x + self.width
            && point.y() >= self.y
            && point.y() <= self.y + self.height
    }

    /// 检查与另一个矩形是否相交
    #[inline]
    pub fn intersects(self, other: Rectangle) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// 合并两个矩形
    #[inline]
    pub fn union(self, other: Rectangle) -> Rectangle {
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Rectangle::new(left, top, right - left, bottom - top)
    }

    /// 与另一个矩形求交集
    #[inline]
    pub fn intersection(self, other: Rectangle) -> Option<Rectangle> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);

        if right > left && bottom > top {
            Some(Rectangle::new(left, top, right - left, bottom - top))
        } else {
            None
        }
    }

    /// 膨胀矩形（扩大或缩小）
    #[inline]
    pub fn inflate(self, dx: f64, dy: f64) -> Rectangle {
        Rectangle {
            x: self.x - dx,
            y: self.y - dy,
            width: self.width + 2.0 * dx,
            height: self.height + 2.0 * dy,
        }
    }
}

impl From<(f64, f64, f64, f64)> for Rectangle {
    #[inline]
    fn from((x, y, width, height): (f64, f64, f64, f64)) -> Self {
        Rectangle::new(x, y, width, height)
    }
}

impl From<Rectangle> for (f64, f64, f64, f64) {
    #[inline]
    fn from(rect: Rectangle) -> Self {
        (rect.x, rect.y, rect.width, rect.height)
    }
}

impl std::fmt::Display for Rectangle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Rectangle({:.4}, {:.4}, {:.4} × {:.4})",
            self.x, self.y, self.width, self.height
        )
    }
}

#[derive(Serialize, Deserialize)]
struct RectangleSerde(f64, f64, f64, f64);

impl From<RectangleSerde> for Rectangle {
    fn from(val: RectangleSerde) -> Self {
        Rectangle::new(val.0, val.1, val.2, val.3)
    }
}

impl From<Rectangle> for RectangleSerde {
    fn from(val: Rectangle) -> Self {
        RectangleSerde(val.x, val.y, val.width, val.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains() {
        let rect = Rectangle::new(0.0, 0.0, 10.0, 10.0);
        assert!(rect.contains(Point::new(5.0, 5.0)));
        assert!(rect.contains(Point::new(0.0, 0.0)));
        assert!(rect.contains(Point::new(10.0, 10.0))); // 边界上
        assert!(!rect.contains(Point::new(11.0, 11.0))); // 边界外
    }

    #[test]
    fn test_intersects() {
        let a = Rectangle::new(0.0, 0.0, 10.0, 10.0);
        let b = Rectangle::new(5.0, 5.0, 10.0, 10.0);
        assert!(a.intersects(b));

        let c = Rectangle::new(20.0, 20.0, 10.0, 10.0);
        assert!(!a.intersects(c));
    }

    #[test]
    fn test_union() {
        let a = Rectangle::new(0.0, 0.0, 10.0, 10.0);
        let b = Rectangle::new(20.0, 20.0, 10.0, 10.0);
        let union = a.union(b);
        assert_eq!(union.x, 0.0);
        assert_eq!(union.y, 0.0);
        assert_eq!(union.width, 30.0);
        assert_eq!(union.height, 30.0);
    }

    #[test]
    fn test_from_corners() {
        let p1 = Point::new(10.0, 10.0);
        let p2 = Point::new(0.0, 0.0);
        let rect = Rectangle::from_corners(p1, p2);
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, 10.0);
        assert_eq!(rect.height, 10.0);
    }
}
