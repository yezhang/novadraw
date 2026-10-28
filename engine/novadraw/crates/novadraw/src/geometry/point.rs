//! 2D 点类型。

use serde::{Deserialize, Serialize};

use crate::Vec2;

/// 2D 位置。
///
/// 点表示仿射空间中的位置，与表示位移的 [`Vec2`] 具有不同语义。
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "PointSerde", into = "PointSerde")]
pub struct Point {
    x: f64,
    y: f64,
}

impl Point {
    /// 原点。
    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0 };

    /// 坐标均为零的点。
    pub const ZERO: Self = Self::ORIGIN;

    /// 创建点。
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// X 坐标。
    #[inline]
    pub const fn x(self) -> f64 {
        self.x
    }

    /// Y 坐标。
    #[inline]
    pub const fn y(self) -> f64 {
        self.y
    }

    /// 到另一点的距离。
    #[inline]
    pub fn distance(self, other: Self) -> f64 {
        (self - other).length()
    }

    /// 在两点之间线性插值。
    #[inline]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        self + (other - self) * t
    }
}

impl std::ops::Sub for Point {
    type Output = Vec2;

    #[inline]
    fn sub(self, other: Self) -> Self::Output {
        Vec2::new(self.x - other.x, self.y - other.y)
    }
}

impl std::ops::Add<Vec2> for Point {
    type Output = Point;

    #[inline]
    fn add(self, vector: Vec2) -> Self::Output {
        Point::new(self.x + vector.x(), self.y + vector.y())
    }
}

impl std::ops::Sub<Vec2> for Point {
    type Output = Point;

    #[inline]
    fn sub(self, vector: Vec2) -> Self::Output {
        Point::new(self.x - vector.x(), self.y - vector.y())
    }
}

impl std::ops::AddAssign<Vec2> for Point {
    #[inline]
    fn add_assign(&mut self, vector: Vec2) {
        self.x += vector.x();
        self.y += vector.y();
    }
}

impl std::ops::SubAssign<Vec2> for Point {
    #[inline]
    fn sub_assign(&mut self, vector: Vec2) {
        self.x -= vector.x();
        self.y -= vector.y();
    }
}

impl From<(f64, f64)> for Point {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl From<[f64; 2]> for Point {
    #[inline]
    fn from([x, y]: [f64; 2]) -> Self {
        Self::new(x, y)
    }
}

impl From<Point> for (f64, f64) {
    #[inline]
    fn from(point: Point) -> Self {
        (point.x, point.y)
    }
}

impl std::fmt::Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Point({:.4}, {:.4})", self.x, self.y)
    }
}

#[derive(Serialize, Deserialize)]
struct PointSerde(f64, f64);

impl From<PointSerde> for Point {
    fn from(value: PointSerde) -> Self {
        Self::new(value.0, value.1)
    }
}

impl From<Point> for PointSerde {
    fn from(value: Point) -> Self {
        Self(value.x, value.y)
    }
}
