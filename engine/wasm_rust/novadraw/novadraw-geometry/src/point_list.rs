//! 点序列类型。

use serde::{Deserialize, Serialize};

use super::{Point, Rectangle, Transform, Translatable};

/// 点序列。
///
/// 对应 draw2d: org.eclipse.draw2d.geometry.PointList。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<Point>", into = "Vec<Point>")]
pub struct PointList {
    points: Vec<Point>,
}

impl PointList {
    /// 创建空点序列。
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// 从点集合创建点序列。
    #[inline]
    pub fn from_points(points: impl Into<Vec<Point>>) -> Self {
        Self {
            points: points.into(),
        }
    }

    /// 点数量。
    #[inline]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// 是否为空。
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// 追加点。
    #[inline]
    pub fn push(&mut self, point: Point) {
        self.points.push(point);
    }

    /// 获取指定索引的点。
    #[inline]
    pub fn get(&self, index: usize) -> Option<Point> {
        self.points.get(index).copied()
    }

    /// 返回点切片。
    #[inline]
    pub fn as_slice(&self) -> &[Point] {
        &self.points
    }

    /// 遍历点。
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Point> {
        self.points.iter()
    }

    /// 返回包含所有点的最小矩形。
    #[inline]
    pub fn bounds(&self) -> Option<Rectangle> {
        let first = self.points.first().copied()?;
        let mut min_x = first.x();
        let mut min_y = first.y();
        let mut max_x = first.x();
        let mut max_y = first.y();

        for point in &self.points[1..] {
            min_x = min_x.min(point.x());
            min_y = min_y.min(point.y());
            max_x = max_x.max(point.x());
            max_y = max_y.max(point.y());
        }

        Some(Rectangle::new(min_x, min_y, max_x - min_x, max_y - min_y))
    }

    /// 返回应用变换后的新点序列。
    #[inline]
    pub fn transformed(&self, transform: Transform) -> Self {
        Self::from_points(
            self.points
                .iter()
                .map(|point| transform.transform_point_vec2(*point))
                .collect::<Vec<_>>(),
        )
    }

    /// 消耗并返回内部点集合。
    #[inline]
    pub fn into_vec(self) -> Vec<Point> {
        self.points
    }
}

impl Translatable for PointList {
    #[inline]
    fn translate(&mut self, dx: f64, dy: f64) {
        for point in &mut self.points {
            point.translate(dx, dy);
        }
    }

    #[inline]
    fn scale(&mut self, factor: f64) {
        for point in &mut self.points {
            point.scale(factor);
        }
    }

    #[inline]
    fn transform(&mut self, transform: Transform) {
        for point in &mut self.points {
            point.transform(transform);
        }
    }
}

impl From<Vec<Point>> for PointList {
    #[inline]
    fn from(points: Vec<Point>) -> Self {
        Self::from_points(points)
    }
}

impl From<PointList> for Vec<Point> {
    #[inline]
    fn from(point_list: PointList) -> Self {
        point_list.points
    }
}

impl FromIterator<Point> for PointList {
    #[inline]
    fn from_iter<T: IntoIterator<Item = Point>>(iter: T) -> Self {
        Self::from_points(iter.into_iter().collect::<Vec<_>>())
    }
}

impl IntoIterator for PointList {
    type Item = Point;
    type IntoIter = std::vec::IntoIter<Point>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.points.into_iter()
    }
}

impl<'a> IntoIterator for &'a PointList {
    type Item = &'a Point;
    type IntoIter = std::slice::Iter<'a, Point>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.points.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_returns_minimum_rectangle_containing_all_points() {
        let points = PointList::from_points(vec![
            Point::new(10.0, -2.0),
            Point::new(-4.0, 8.0),
            Point::new(6.0, 3.0),
        ]);

        assert_eq!(
            points.bounds(),
            Some(Rectangle::new(-4.0, -2.0, 14.0, 10.0))
        );
    }

    #[test]
    fn empty_bounds_returns_none() {
        assert_eq!(PointList::new().bounds(), None);
    }

    #[test]
    fn translate_and_scale_apply_to_all_points() {
        let mut points = PointList::from_points(vec![Point::new(1.0, 2.0), Point::new(-3.0, 4.0)]);

        points.translate(2.0, -1.0);
        points.scale(2.0);

        assert_eq!(
            points.as_slice(),
            &[Point::new(6.0, 2.0), Point::new(-2.0, 6.0)]
        );
    }

    #[test]
    fn transformed_returns_new_point_list() {
        let points = PointList::from_points(vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)]);
        let transformed = points.transformed(Transform::from_translation(10.0, 20.0));

        assert_eq!(
            transformed.as_slice(),
            &[Point::new(11.0, 22.0), Point::new(13.0, 24.0)]
        );
        assert_eq!(
            points.as_slice(),
            &[Point::new(1.0, 2.0), Point::new(3.0, 4.0)]
        );
    }
}
