//! 2D 仿射变换
//!
//! 基于 kurbo::Affine，提供统一的 2D 仿射变换接口。
//! 便于未来替换为其他实现（如 miniquad、nalgebra 等）。
//!
//! # 矩阵布局
//!
//! | a c e |
//! | b d f |
//! | 0 0 1 |
//!
//! # 变换顺序
//!
//! `A * B` 表示先应用 B，再应用 A。

use kurbo::Affine;
use serde::{Deserialize, Serialize};

use super::Vec2;

/// 2D 仿射变换
///
/// 基于 `kurbo::Affine`，提供高性能的 2D 仿射变换实现。
/// 使用行优先矩阵布局，与 CSS 和 HTML5 Canvas 一致。
///
/// # 运算符
///
/// `A * B` 表示矩阵乘法 `A × B`
/// 语义：先应用 B，再应用 A
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "TransformSerde", into = "TransformSerde")]
pub struct Transform {
    inner: Affine,
}

impl Transform {
    /// 单位变换
    pub const IDENTITY: Transform = Transform {
        inner: Affine::IDENTITY,
    };

    /// 从系数创建
    ///
    /// 系数顺序: [a, b, c, d, e, f]
    /// 矩阵布局:
    /// | a c e |
    /// | b d f |
    /// | 0 0 1 |
    #[inline]
    pub fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self {
            inner: Affine::new([a, b, c, d, e, f]),
        }
    }

    /// 从平移创建
    #[inline]
    pub fn from_translation(x: f64, y: f64) -> Self {
        Self {
            inner: Affine::translate(kurbo::Vec2::new(x, y)),
        }
    }

    /// 从平移创建 (Vec2)
    #[inline]
    pub fn from_translation_vec(translation: Vec2) -> Self {
        Self {
            inner: Affine::translate(kurbo::Vec2::new(translation.x(), translation.y())),
        }
    }

    /// 从缩放创建
    #[inline]
    pub fn from_scale(x: f64, y: f64) -> Self {
        Self {
            inner: Affine::scale_non_uniform(x, y),
        }
    }

    /// 从统一缩放创建
    #[inline]
    pub fn from_uniform_scale(s: f64) -> Self {
        Self {
            inner: Affine::scale(s),
        }
    }

    /// 从旋转创建 (弧度，绕原点，逆时针)
    #[inline]
    pub fn from_rotation(radians: f64) -> Self {
        Self {
            inner: Affine::rotate(radians),
        }
    }

    /// 变换组合：`self * other = self × other`
    ///
    /// 语义：先应用 other，再应用 self
    #[inline]
    pub fn multiply(self, other: Transform) -> Transform {
        Transform {
            inner: self.inner * other.inner,
        }
    }

    /// 在当前变换之后拼接 local 变换。
    ///
    /// 返回 `self * local`。对列向量语义，这表示先应用 `local`，
    /// 再应用已有的 parent/world 变换 `self`。
    #[inline]
    pub fn post_concat(self, local: Transform) -> Transform {
        self.multiply(local)
    }

    /// 在当前变换之前拼接 parent 变换。
    ///
    /// 返回 `parent * self`。
    #[inline]
    pub fn pre_concat(self, parent: Transform) -> Transform {
        parent.multiply(self)
    }

    /// 变换点
    #[inline]
    pub fn transform_point(self, x: f64, y: f64) -> (f64, f64) {
        let p = self.inner * kurbo::Point::new(x, y);
        (p.x, p.y)
    }

    /// 变换点 (返回 Vec2)
    #[inline]
    pub fn transform_point_vec2(self, point: Vec2) -> Vec2 {
        let p = self.inner * kurbo::Point::new(point.x(), point.y());
        Vec2::new(p.x, p.y)
    }

    /// 变换向量 (不含平移)
    ///
    /// 向量只受线性变换影响，不包含平移分量
    #[inline]
    pub fn transform_vector(self, x: f64, y: f64) -> (f64, f64) {
        let coeffs = self.inner.as_coeffs();
        // coeffs = [a, b, c, d, e, f]
        // 线性部分: x' = a*x + c*y, y' = b*x + d*y
        let a = coeffs[0];
        let b = coeffs[1];
        let c = coeffs[2];
        let d = coeffs[3];
        (a * x + c * y, b * x + d * y)
    }

    /// 逆变换
    #[inline]
    pub fn inverse(self) -> Option<Transform> {
        let inv = self.inner.inverse();
        if inv.as_coeffs().iter().all(|v| v.is_finite()) {
            Some(Transform { inner: inv })
        } else {
            None
        }
    }

    /// 行列式
    #[inline]
    pub fn determinant(self) -> f64 {
        self.inner.determinant()
    }

    /// 提取系数
    ///
    /// 返回 [a, b, c, d, e, f]
    #[inline]
    pub fn coeffs(self) -> [f64; 6] {
        self.inner.as_coeffs()
    }

    /// 平移分量
    #[inline]
    pub fn translation(self) -> (f64, f64) {
        let t = self.inner.translation();
        (t.x, t.y)
    }

    /// 平移分量 (Vec2)
    #[inline]
    pub fn translation_vec2(self) -> Vec2 {
        let t = self.inner.translation();
        Vec2::new(t.x, t.y)
    }

    /// 内部 Affine 引用（用于低级操作）
    #[inline]
    pub fn inner(&self) -> &Affine {
        &self.inner
    }

    /// 消耗内部 Affine
    #[inline]
    pub fn into_inner(self) -> Affine {
        self.inner
    }

    /// 追加平移
    ///
    /// 语义：`self.then_translate(...)` 等价于 `Transform::from_translation(...) * self`
    #[inline]
    pub fn then_translate(self, x: f64, y: f64) -> Self {
        Self {
            inner: self.inner.then_translate(kurbo::Vec2::new(x, y)),
        }
    }

    /// 追加统一缩放
    #[inline]
    pub fn then_scale(self, scale: f64) -> Self {
        Self {
            inner: self.inner.then_scale(scale),
        }
    }

    /// 追加非均匀缩放
    #[inline]
    pub fn then_scale_non_uniform(self, scale_x: f64, scale_y: f64) -> Self {
        Self {
            inner: self.inner.then_scale_non_uniform(scale_x, scale_y),
        }
    }

    /// 追加旋转（弧度，绕原点，逆时针）
    #[inline]
    pub fn then_rotate(self, radians: f64) -> Self {
        Self {
            inner: self.inner.then_rotate(radians),
        }
    }

    /// 追加旋转（绕指定点）
    #[inline]
    pub fn then_rotate_about(self, radians: f64, cx: f64, cy: f64) -> Self {
        Self {
            inner: self
                .inner
                .then_rotate_about(radians, kurbo::Point::new(cx, cy)),
        }
    }

    /// 追加缩放（绕指定点）
    ///
    /// 语义：`self.then_scale_about(...)` 等价于 `Transform::from_scale_about(...) * self`
    #[inline]
    pub fn then_scale_about(self, scale: f64, cx: f64, cy: f64) -> Self {
        Self {
            inner: self
                .inner
                .then_scale_about(scale, kurbo::Point::new(cx, cy)),
        }
    }

    /// 追加任意变换
    ///
    /// 语义：`self.then_transform(other)` 等价于 `other * self`
    #[inline]
    pub fn then_transform(self, other: Transform) -> Self {
        Self {
            inner: other.inner * self.inner,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Transform::IDENTITY
    }
}

/// 变换乘法：`A * B = A × B`
///
/// 语义：先应用 B，再应用 A
impl std::ops::Mul for Transform {
    type Output = Transform;

    #[inline]
    fn mul(self, other: Transform) -> Self::Output {
        self.multiply(other)
    }
}

impl std::ops::MulAssign for Transform {
    #[inline]
    fn mul_assign(&mut self, other: Transform) {
        *self = *self * other;
    }
}

impl std::fmt::Display for Transform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let coeffs = self.inner.as_coeffs();
        write!(
            f,
            "Transform({:.4}, {:.4}, {:.4}\n          {:.4}, {:.4}, {:.4})",
            coeffs[0], coeffs[2], coeffs[4], coeffs[1], coeffs[3], coeffs[5]
        )
    }
}

#[derive(Serialize, Deserialize)]
struct TransformSerde([f64; 6]);

impl From<TransformSerde> for Transform {
    fn from(val: TransformSerde) -> Self {
        Transform::new(val.0[0], val.0[1], val.0[2], val.0[3], val.0[4], val.0[5])
    }
}

impl From<Transform> for TransformSerde {
    fn from(val: Transform) -> Self {
        TransformSerde(val.coeffs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let identity = Transform::IDENTITY;
        let p = identity.transform_point(5.0, 10.0);
        assert_eq!(p, (5.0, 10.0));
    }

    #[test]
    fn test_translation() {
        let t = Transform::from_translation(10.0, 20.0);
        let p = t.transform_point(5.0, 5.0);
        assert_eq!(p, (15.0, 25.0));
    }

    #[test]
    fn test_scale() {
        let t = Transform::from_scale(2.0, 3.0);
        let p = t.transform_point(5.0, 10.0);
        assert_eq!(p, (10.0, 30.0));
    }

    #[test]
    fn test_multiplication_order() {
        let parent = Transform::from_translation(10.0, 0.0);
        let child = Transform::from_scale(2.0, 2.0);

        // parent * child = parent × child
        // 语义：先 child(缩放)，后 parent(平移)
        let combined = parent * child;
        let p = combined.transform_point(5.0, 5.0);

        // 缩放后：(10, 10)，平移后：(20, 10)
        assert_eq!(p, (20.0, 10.0));
    }

    #[test]
    fn test_reverse_order() {
        let parent = Transform::from_translation(10.0, 0.0);
        let child = Transform::from_scale(2.0, 2.0);

        // child * parent = child × parent
        // 语义：先 parent(平移)，后 child(缩放)
        let combined = child * parent;
        let p = combined.transform_point(5.0, 5.0);

        // 平移后：(15, 5)，缩放后：(30, 10)
        assert_eq!(p, (30.0, 10.0));
    }

    #[test]
    fn test_rotation() {
        let t = Transform::from_rotation(std::f64::consts::FRAC_PI_2);
        let p = t.transform_point(0.0, 1.0);
        // 逆时针旋转90度: (0, 1) -> (-1, 0)
        assert!((p.0 + 1.0).abs() < 1e-10);
        assert!((p.1 - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_inverse() {
        let t = Transform::from_translation(10.0, 20.0);
        let inv = t.inverse().unwrap();
        let p = inv.transform_point(15.0, 25.0);
        assert_eq!(p, (5.0, 5.0));
    }

    #[test]
    fn test_coeffs() {
        let t = Transform::from_translation(10.0, 20.0);
        let coeffs = t.coeffs();
        // identity: [1, 0, 0, 1, 10, 20]
        // translation: [1, 0, 0, 1, 10, 20]
        assert_eq!(coeffs[0], 1.0); // a
        assert_eq!(coeffs[1], 0.0); // b
        assert_eq!(coeffs[2], 0.0); // c
        assert_eq!(coeffs[3], 1.0); // d
        assert_eq!(coeffs[4], 10.0); // e
        assert_eq!(coeffs[5], 20.0); // f
    }

    #[test]
    fn test_transform_vector() {
        let t = Transform::from_translation(10.0, 20.0);
        let v = t.transform_vector(5.0, 5.0);
        // 向量不应该包含平移
        assert_eq!(v, (5.0, 5.0));
    }

    #[test]
    fn test_then_translate() {
        // 先平移(10,0)，再追加平移(0,5)
        let t = Transform::IDENTITY
            .then_translate(10.0, 0.0)
            .then_translate(0.0, 5.0);
        let p = t.transform_point(0.0, 0.0);
        assert_eq!(p, (10.0, 5.0));
    }

    #[test]
    fn test_then_scale() {
        let t = Transform::IDENTITY.then_scale(2.0);
        let p = t.transform_point(5.0, 10.0);
        assert_eq!(p, (10.0, 20.0));
    }

    #[test]
    fn test_then_scale_non_uniform() {
        let t = Transform::IDENTITY.then_scale_non_uniform(2.0, 3.0);
        let p = t.transform_point(5.0, 10.0);
        assert_eq!(p, (10.0, 30.0));
    }

    #[test]
    fn test_then_rotate() {
        let t = Transform::IDENTITY.then_rotate(std::f64::consts::FRAC_PI_2);
        let p = t.transform_point(1.0, 0.0);
        // 逆时针旋转90度: (1, 0) -> (0, 1)
        assert!((p.0 - 0.0).abs() < 1e-10);
        assert!((p.1 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_then_rotate_about() {
        // 绕点(1,0)旋转90度
        let t = Transform::IDENTITY.then_rotate_about(std::f64::consts::FRAC_PI_2, 1.0, 0.0);
        let p = t.transform_point(1.0, 0.0);
        // 绕(1,0)旋转，自身不变
        assert!((p.0 - 1.0).abs() < 1e-10);
        assert!((p.1 - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_then_scale_about() {
        // 绕点(1,1)缩放2倍
        let t = Transform::IDENTITY.then_scale_about(2.0, 1.0, 1.0);
        let p = t.transform_point(1.0, 1.0);
        // 绕(1,1)缩放，自身不变
        assert!((p.0 - 1.0).abs() < 1e-10);
        assert!((p.1 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_then_chain() {
        let t = Transform::IDENTITY
            .then_translate(10.0, 0.0)
            .then_rotate(std::f64::consts::FRAC_PI_2)
            .then_scale(2.0);

        let p = t.transform_point(5.0, 0.0);
        // 先平移(10,0): (5,0) -> (15,0)
        // 再旋转90度: (15,0) -> (0,15)
        // 再缩放2倍: (0,15) -> (0,30)
        assert!((p.0 - 0.0).abs() < 1e-10);
        assert!((p.1 - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_then_equivalent_to_multiply() {
        let t1 = Transform::IDENTITY
            .then_translate(10.0, 0.0)
            .then_scale(2.0);

        let t2 = Transform::from_scale(2.0, 2.0) * Transform::from_translation(10.0, 0.0);

        let p1 = t1.transform_point(0.0, 5.0);
        let p2 = t2.transform_point(0.0, 5.0);

        assert!((p1.0 - p2.0).abs() < 1e-10);
        assert!((p1.1 - p2.1).abs() < 1e-10);
    }

    #[test]
    fn test_then_transform() {
        let t1 = Transform::from_translation(10.0, 0.0);
        let t2 = Transform::from_scale(2.0, 2.0);

        let combined = Transform::IDENTITY.then_transform(t1).then_transform(t2);
        let p = combined.transform_point(5.0, 5.0);

        // 等价于 t2 * t1
        // 先平移(10,0): (5,5) -> (15,5)
        // 再缩放2倍: (15,5) -> (30,10)
        assert_eq!(p, (30.0, 10.0));
    }

    #[test]
    fn test_then_transform_equivalent() {
        // then_transform(other) 等价于 other * self
        let t1 = Transform::from_translation(10.0, 0.0);
        let t2 = Transform::from_scale(2.0, 2.0);

        // 使用 then_transform
        let result1 = t1.then_transform(t2);

        // 使用 multiply
        let result2 = t2 * t1;

        let p1 = result1.transform_point(5.0, 5.0);
        let p2 = result2.transform_point(5.0, 5.0);

        assert!((p1.0 - p2.0).abs() < 1e-10);
        assert!((p1.1 - p2.1).abs() < 1e-10);
    }

    #[test]
    fn post_concat_keeps_parent_scale_outside_child_translation() {
        let parent = Transform::from_scale(2.0, 3.0);
        let child = Transform::from_translation(10.0, 20.0);

        let world = parent.post_concat(child);

        assert_eq!(world.transform_point(0.0, 0.0), (20.0, 60.0));
        assert_eq!(world.transform_vector(5.0, 4.0), (10.0, 12.0));
    }

    #[test]
    fn pre_concat_places_parent_before_existing_local_transform() {
        let local = Transform::from_translation(10.0, 20.0);
        let parent = Transform::from_scale(2.0, 3.0);

        assert_eq!(local.pre_concat(parent), parent.post_concat(local));
    }
}
