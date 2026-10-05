//! 2D 向量类型
//!
//! 纯数学类型，用于表示 2D 位移和方向。

use serde::{Deserialize, Serialize};

/// 2D 向量类型
///
/// 使用 `f64` 精度，遵循标准数学运算语义。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec2Serde", into = "Vec2Serde")]
pub struct Vec2 {
    x: f64,
    y: f64,
}

impl Vec2 {
    /// 零向量
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// X 轴单位向量
    pub const X: Self = Self { x: 1.0, y: 0.0 };

    /// Y 轴单位向量
    pub const Y: Self = Self { x: 0.0, y: 1.0 };

    /// 创建向量
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// X 分量
    #[inline]
    pub const fn x(self) -> f64 {
        self.x
    }

    /// Y 分量
    #[inline]
    pub const fn y(self) -> f64 {
        self.y
    }

    /// 向量长度
    #[inline]
    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    /// 长度平方
    #[inline]
    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// 归一化
    #[inline]
    pub fn normalize(self) -> Self {
        self / self.length()
    }

    /// 点积
    #[inline]
    pub fn dot(self, other: Vec2) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// 叉积 (2D 叉乘结果为标量)
    #[inline]
    pub fn cross(self, other: Vec2) -> f64 {
        self.x * other.y - self.y * other.x
    }

    /// 旋转向量（顺时针，Y轴向下坐标系）
    ///
    /// 使用标准旋转矩阵，在 Y 轴向下的坐标系中表现为顺时针旋转。
    /// 角度为正时，向量向顺时针方向旋转。
    #[inline]
    pub fn rotate_radians(self, radians: f64) -> Self {
        let (s, c) = radians.sin_cos();
        Vec2::new(self.x * c + self.y * s, -self.x * s + self.y * c)
    }

    /// 线性插值
    #[inline]
    pub fn lerp(self, other: Vec2, t: f64) -> Vec2 {
        self + (other - self) * t
    }

    /// 到另一点的距离
    #[inline]
    pub fn distance(self, other: Vec2) -> f64 {
        (self - other).length()
    }
}

impl Default for Vec2 {
    fn default() -> Self {
        Vec2::ZERO
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;

    #[inline]
    fn add(self, other: Vec2) -> Self::Output {
        Vec2::new(self.x + other.x, self.y + other.y)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Vec2;

    #[inline]
    fn sub(self, other: Vec2) -> Self::Output {
        Vec2::new(self.x - other.x, self.y - other.y)
    }
}

impl std::ops::Mul<f64> for Vec2 {
    type Output = Vec2;

    #[inline]
    fn mul(self, scalar: f64) -> Self::Output {
        Vec2::new(self.x * scalar, self.y * scalar)
    }
}

impl std::ops::Mul<Vec2> for Vec2 {
    type Output = f64;

    #[inline]
    fn mul(self, other: Vec2) -> Self::Output {
        self.dot(other)
    }
}

impl std::ops::Div<f64> for Vec2 {
    type Output = Vec2;

    #[inline]
    fn div(self, scalar: f64) -> Self::Output {
        Vec2::new(self.x / scalar, self.y / scalar)
    }
}

impl std::ops::Neg for Vec2 {
    type Output = Vec2;

    #[inline]
    fn neg(self) -> Self::Output {
        Vec2::new(-self.x, -self.y)
    }
}

impl std::ops::AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, other: Vec2) {
        self.x += other.x;
        self.y += other.y;
    }
}

impl std::ops::SubAssign for Vec2 {
    #[inline]
    fn sub_assign(&mut self, other: Vec2) {
        self.x -= other.x;
        self.y -= other.y;
    }
}

impl std::ops::MulAssign<f64> for Vec2 {
    #[inline]
    fn mul_assign(&mut self, scalar: f64) {
        self.x *= scalar;
        self.y *= scalar;
    }
}

impl std::ops::DivAssign<f64> for Vec2 {
    #[inline]
    fn div_assign(&mut self, scalar: f64) {
        self.x /= scalar;
        self.y /= scalar;
    }
}

impl From<(f64, f64)> for Vec2 {
    fn from((x, y): (f64, f64)) -> Self {
        Vec2::new(x, y)
    }
}

impl From<[f64; 2]> for Vec2 {
    fn from([x, y]: [f64; 2]) -> Self {
        Vec2::new(x, y)
    }
}

impl std::fmt::Display for Vec2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Vec2({:.4}, {:.4})", self.x(), self.y())
    }
}

#[derive(Serialize, Deserialize)]
struct Vec2Serde(f64, f64);

impl From<Vec2Serde> for Vec2 {
    fn from(val: Vec2Serde) -> Self {
        Vec2::new(val.0, val.1)
    }
}

impl From<Vec2> for Vec2Serde {
    fn from(val: Vec2) -> Self {
        Vec2Serde(val.x(), val.y())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(
            Vec2::new(1.0, 2.0) + Vec2::new(3.0, 4.0),
            Vec2::new(4.0, 6.0)
        );
    }

    #[test]
    fn test_sub() {
        assert_eq!(
            Vec2::new(5.0, 6.0) - Vec2::new(2.0, 3.0),
            Vec2::new(3.0, 3.0)
        );
    }

    #[test]
    fn test_mul_scalar() {
        assert_eq!(Vec2::new(2.0, 3.0) * 2.0, Vec2::new(4.0, 6.0));
    }

    #[test]
    fn test_mul_vec() {
        assert_eq!(Vec2::new(2.0, 3.0) * Vec2::new(4.0, 5.0), 23.0); // 2*4 + 3*5
    }

    #[test]
    fn test_div() {
        assert_eq!(Vec2::new(6.0, 8.0) / 2.0, Vec2::new(3.0, 4.0));
    }

    #[test]
    fn test_neg() {
        assert_eq!(-Vec2::new(3.0, -4.0), Vec2::new(-3.0, 4.0));
    }
}
