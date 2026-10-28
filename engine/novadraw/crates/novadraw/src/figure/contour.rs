//! 可复用的圆角轮廓；公开签名只使用引擎几何与 Path。

use crate::geometry::{Dimension, Point, Rectangle};
use crate::graphics::{Color, Path};
use kurbo::{ParamCurveNearest, Shape as _};

/// 几何构造失败；失败不会改变调用方的请求配置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeGeometryError {
    NonFinite,
    NegativeMetric,
    RadiusCount { vertices: usize, radii: usize },
    DegeneratePolygon,
    SelfIntersection,
}

impl std::fmt::Display for ShapeGeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shape geometry: {self:?}")
    }
}

impl std::error::Error for ShapeGeometryError {}

impl ShapeGeometryError {
    pub(crate) fn drawing_error(self) -> crate::graphics::GraphicsError {
        crate::render::GraphicsInputError::InvalidPath { index: 0 }.into()
    }
}

/// 矩形四角的真实半径，顺序为左上、右上、右下、左下。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectangleCorners([Dimension; 4]);

impl RectangleCorners {
    pub const ZERO: Self = Self(
        [Dimension {
            width: 0.0,
            height: 0.0,
        }; 4],
    );

    pub fn new(radii: [Dimension; 4]) -> Result<Self, ShapeGeometryError> {
        for radius in radii {
            metric(radius.width)?;
            metric(radius.height)?;
        }
        Ok(Self(radii))
    }

    pub fn circular(radii: [f64; 4]) -> Result<Self, ShapeGeometryError> {
        Self::new(radii.map(|r| Dimension::new(r, r)))
    }

    pub fn uniform(radius: f64) -> Result<Self, ShapeGeometryError> {
        Self::circular([radius; 4])
    }

    pub const fn radii(self) -> [Dimension; 4] {
        self.0
    }
}

/// 一条矩形边的颜色策略。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum EdgeStroke {
    #[default]
    Inherit,
    Color(Color),
    None,
}

impl EdgeStroke {
    pub(crate) fn resolve(self, inherited: Option<Color>) -> Option<Color> {
        match self {
            Self::Inherit => inherited,
            Self::Color(color) => Some(color),
            Self::None => None,
        }
    }
}

/// 上、右、下、左四条边，使用同一线宽。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectangleEdges(pub [EdgeStroke; 4]);

impl RectangleEdges {
    pub const INHERIT: Self = Self([EdgeStroke::Inherit; 4]);
}

/// 不可变轮廓，填充、描边和命中共用同一路径。
#[derive(Clone, Debug)]
pub struct ShapeContour {
    path: Path,
    hit_path: kurbo::BezPath,
    effective_radii: Vec<f64>,
}

impl ShapeContour {
    fn from_path(path: Path, effective_radii: Vec<f64>) -> Self {
        use crate::render::backend_support::{NormalizedPathOp, for_each_normalized};
        let mut hit_path = kurbo::BezPath::new();
        let p = |p: Point| kurbo::Point::new(p.x(), p.y());
        for_each_normalized(path.operations(), 1.0, |op| match op {
            NormalizedPathOp::MoveTo(v) => hit_path.move_to(p(v)),
            NormalizedPathOp::LineTo(v) => hit_path.line_to(p(v)),
            NormalizedPathOp::CubicTo(v) => {
                hit_path.curve_to(p(v.control1), p(v.control2), p(v.end));
            }
            NormalizedPathOp::QuadTo { control, end } => {
                hit_path.quad_to(p(control), p(end));
            }
            NormalizedPathOp::Close => hit_path.close_path(),
        });
        Self {
            path,
            hit_path,
            effective_radii,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 有效的多边形半径；请求值仍由消费者保存。
    pub fn effective_radii(&self) -> &[f64] {
        &self.effective_radii
    }

    /// 命中使用通用 Path 填充默认的 NonZero 规则，并包含轮廓边界。
    pub fn contains(&self, point: Point) -> bool {
        const HIT_BOUNDARY_TOLERANCE: f64 = 1e-7;
        let p = kurbo::Point::new(point.x(), point.y());
        self.hit_path.winding(p) != 0
            || self.hit_path.segments().any(|segment| {
                segment.nearest(p, HIT_BOUNDARY_TOLERANCE).distance_sq
                    <= HIT_BOUNDARY_TOLERANCE * HIT_BOUNDARY_TOLERANCE
            })
    }

    pub fn rectangle(
        bounds: Rectangle,
        corners: RectangleCorners,
    ) -> Result<Self, ShapeGeometryError> {
        let corners = rectangle_geometry(bounds, corners)?;
        let mut path = Path::new();
        move_to(&mut path, corners[0].exit);
        for index in 1..=4 {
            let c = corners[index % 4];
            line_to(&mut path, c.entry);
            c.arc(&mut path, c.mid, true);
            c.arc(&mut path, c.exit, true);
        }
        path.close();
        Ok(Self::from_path(path, Vec::new()))
    }

    /// 为 simple polygon 构造与两邻边相切的圆形圆角。
    pub fn polygon(points: &[Point], radii: &[f64]) -> Result<Self, ShapeGeometryError> {
        if points.len() != radii.len() {
            return Err(ShapeGeometryError::RadiusCount {
                vertices: points.len(),
                radii: radii.len(),
            });
        }
        for p in points {
            finite(p.x())?;
            finite(p.y())?;
        }
        for &r in radii {
            metric(r)?;
        }
        let mut path = Path::new();
        if points.len() < 3 {
            if radii.iter().any(|r| *r > 0.0) {
                return Err(ShapeGeometryError::DegeneratePolygon);
            }
            return Ok(Self::from_path(path, radii.to_vec()));
        }
        let n = points.len();
        let mut distances = vec![0.0; n];
        let mut sweeps = vec![true; n];
        if radii.iter().any(|r| *r > 0.0) {
            validate_polygon(points)?;
            for i in 0..n {
                let a = points[(i + n - 1) % n];
                let v = points[i];
                let b = points[(i + 1) % n];
                let (ax, ay) = unit(a.x() - v.x(), a.y() - v.y())?;
                let (bx, by) = unit(b.x() - v.x(), b.y() - v.y())?;
                let dot = (ax * bx + ay * by).clamp(-1.0, 1.0);
                if dot > -1.0 + f64::EPSILON && radii[i] > 0.0 {
                    distances[i] = radii[i] * ((1.0 + dot) / (1.0 - dot)).sqrt();
                    finite(distances[i])?;
                }
                sweeps[i] = cross(a, v, b) > 0.0;
            }
        }
        let mut scale = 1.0_f64;
        for i in 0..n {
            let next = (i + 1) % n;
            let sum = distances[i] + distances[next];
            finite(sum)?;
            if sum > 0.0 {
                let length = distance(points[i], points[next]);
                finite(length)?;
                scale = scale.min(length / sum);
            }
        }
        let mut entry = points.to_vec();
        let mut exit = points.to_vec();
        let mut effective = vec![0.0; n];
        for i in 0..n {
            let d = distances[i] * scale;
            if d > 0.0 {
                let v = points[i];
                let a = points[(i + n - 1) % n];
                let b = points[(i + 1) % n];
                let (ax, ay) = unit(a.x() - v.x(), a.y() - v.y())?;
                let (bx, by) = unit(b.x() - v.x(), b.y() - v.y())?;
                entry[i] = Point::new(v.x() + ax * d, v.y() + ay * d);
                exit[i] = Point::new(v.x() + bx * d, v.y() + by * d);
                effective[i] = radii[i] * scale;
                for value in [
                    entry[i].x(),
                    entry[i].y(),
                    exit[i].x(),
                    exit[i].y(),
                    effective[i],
                ] {
                    finite(value)?;
                }
            }
        }
        move_to(&mut path, exit[0]);
        for step in 1..=n {
            let i = step % n;
            line_to(&mut path, entry[i]);
            if effective[i] > 0.0 {
                path.arc_to_degrees(
                    effective[i],
                    effective[i],
                    0.0,
                    false,
                    sweeps[i],
                    exit[i].x(),
                    exit[i].y(),
                );
            } else {
                line_to(&mut path, exit[i]);
            }
        }
        path.close();
        Ok(Self::from_path(path, effective))
    }
}

/// 外轮廓和内轮廓形成的四个互不重叠边框区域。
#[derive(Clone, Debug)]
pub struct RectangleBorderGeometry {
    regions: [Path; 4],
    sectors: [Path; 4],
    ring: Path,
    clip: crate::graphics::ClipPath,
    free_ends: std::sync::Arc<FreeEndGeometry>,
}

const RECTANGLE_EDGE_COUNT: usize = 4;
const ALL_RECTANGLE_EDGES: u8 = (1 << RECTANGLE_EDGE_COUNT) - 1;

#[derive(Debug)]
struct BorderPaths {
    regions: [Path; RECTANGLE_EDGE_COUNT],
    sectors: [Path; RECTANGLE_EDGE_COUNT],
}

#[derive(Debug)]
struct FreeEndGeometry {
    outer: [Corner; RECTANGLE_EDGE_COUNT],
    inner: [Corner; RECTANGLE_EDGE_COUNT],
    vertices: [Point; RECTANGLE_EDGE_COUNT],
    center: Point,
    extent: f64,
    // One object-owned immutable result, not a cache for every possible mask.
    cached: std::sync::Mutex<Option<(u8, std::sync::Arc<BorderPaths>)>>,
}

impl FreeEndGeometry {
    fn paths(&self, mask: u8) -> Result<std::sync::Arc<BorderPaths>, ShapeGeometryError> {
        let mut cached = self
            .cached
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((old_mask, paths)) = &*cached
            && *old_mask == mask
        {
            return Ok(std::sync::Arc::clone(paths));
        }
        let paths = std::sync::Arc::new(self.build_paths(mask)?);
        *cached = Some((mask, std::sync::Arc::clone(&paths)));
        Ok(paths)
    }

    fn build_paths(&self, mask: u8) -> Result<BorderPaths, ShapeGeometryError> {
        let mut regions = std::array::from_fn(|_| Path::new());
        let mut sectors = std::array::from_fn(|_| Path::new());
        for i in 0..RECTANGLE_EDGE_COUNT {
            let j = (i + 1) % RECTANGLE_EDGE_COUNT;
            let previous = (i + RECTANGLE_EDGE_COUNT - 1) % RECTANGLE_EDGE_COUNT;
            let active = mask & (1 << i) != 0;
            let free_start = active && mask & (1 << previous) == 0;
            let free_end = active && mask & (1 << j) == 0;
            let outer_start = if free_start {
                self.outer[i].exit
            } else {
                self.outer[i].mid
            };
            let outer_end = if free_end {
                self.outer[j].entry
            } else {
                self.outer[j].mid
            };
            let inner_start = if free_start {
                self.inner[i].exit
            } else {
                self.inner[i].mid
            };
            let inner_end = if free_end {
                self.inner[j].entry
            } else {
                self.inner[j].mid
            };
            // Butt sections stay perpendicular to the straight edge. Rounded
            // free corners terminate at the tangent instead of forming a join.
            let project = |inner: Point, outer: Point| {
                if i % 2 == 0 {
                    Point::new(outer.x(), inner.y())
                } else {
                    Point::new(inner.x(), outer.y())
                }
            };
            let cap_start = if free_start {
                project(inner_start, outer_start)
            } else {
                inner_start
            };
            let cap_end = if free_end {
                project(inner_end, outer_end)
            } else {
                inner_end
            };
            let region = &mut regions[i];
            move_to(region, outer_start);
            if !free_start {
                self.outer[i].arc(region, self.outer[i].exit, true);
            }
            line_to(region, self.outer[j].entry);
            if !free_end {
                self.outer[j].arc(region, outer_end, true);
            }
            line_to(region, cap_end);
            if free_end {
                line_to(region, self.inner[j].entry);
            } else {
                self.inner[j].arc(region, self.inner[j].entry, false);
            }
            line_to(region, self.inner[i].exit);
            if free_start {
                line_to(region, cap_start);
            } else {
                self.inner[i].arc(region, cap_start, false);
            }
            region.close();

            let far_start = self.cut_far(outer_start, cap_start, i, free_start.then_some(i))?;
            let far_end = self.cut_far(outer_end, cap_end, j, free_end.then_some(i))?;
            let sector = &mut sectors[i];
            move_to(sector, far_start);
            for p in [self.vertices[i], self.vertices[j], far_end, cap_end] {
                line_to(sector, p);
            }
            if cap_end != inner_end {
                line_to(sector, inner_end);
            }
            line_to(sector, self.center);
            line_to(sector, inner_start);
            if cap_start != inner_start {
                line_to(sector, cap_start);
            }
            sector.close();
        }
        Ok(BorderPaths { regions, sectors })
    }

    fn cut_far(
        &self,
        outer: Point,
        inner: Point,
        corner: usize,
        free_edge: Option<usize>,
    ) -> Result<Point, ShapeGeometryError> {
        let dx = outer.x() - inner.x();
        let dy = outer.y() - inner.y();
        let length = dx.abs().max(dy.abs());
        let (dx, dy) = if length > 0.0 {
            (dx / length, dy / length)
        } else if let Some(edge) = free_edge {
            [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)][edge]
        } else {
            [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)][corner]
        };
        let far = Point::new(outer.x() + dx * self.extent, outer.y() + dy * self.extent);
        finite(far.x())?;
        finite(far.y())?;
        Ok(far)
    }
}

impl RectangleBorderGeometry {
    pub fn new(
        bounds: Rectangle,
        corners: RectangleCorners,
        width: f64,
    ) -> Result<Self, ShapeGeometryError> {
        metric(width)?;
        let outer = rectangle_geometry(bounds, corners)?;
        let inset = width.min(bounds.width / 2.0).min(bounds.height / 2.0);
        let inner_bounds = Rectangle::new(
            bounds.x + inset,
            bounds.y + inset,
            bounds.width - inset * 2.0,
            bounds.height - inset * 2.0,
        );
        let inner_radii = RectangleCorners::new(outer.map(|c| {
            Dimension::new(
                (c.radius.width - inset).max(0.0),
                (c.radius.height - inset).max(0.0),
            )
        }))?;
        let inner = rectangle_geometry(inner_bounds, inner_radii)?;
        let mut ring = Path::new();
        move_to(&mut ring, outer[0].exit);
        for step in 1..=4 {
            let c = outer[step % 4];
            line_to(&mut ring, c.entry);
            c.arc(&mut ring, c.mid, true);
            c.arc(&mut ring, c.exit, true);
        }
        ring.close();
        // Opposite winding gives a hole using the ordinary nonzero fill rule.
        move_to(&mut ring, inner[0].exit);
        for step in 0..4 {
            let c = inner[(4 - step) % 4];
            c.arc(&mut ring, c.mid, false);
            c.arc(&mut ring, c.entry, false);
            line_to(&mut ring, inner[(7 - step) % 4].exit);
        }
        ring.close();
        let clip = crate::graphics::ClipPath::try_new(&ring, crate::graphics::FillRule::NonZero)
            .map_err(|_| ShapeGeometryError::NonFinite)?;
        let vertices = [
            Point::new(bounds.x, bounds.y),
            Point::new(bounds.x + bounds.width, bounds.y),
            Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
            Point::new(bounds.x, bounds.y + bounds.height),
        ];
        let extent = bounds.width + bounds.height;
        finite(extent)?;
        let free_ends = std::sync::Arc::new(FreeEndGeometry {
            outer,
            inner,
            vertices,
            extent,
            center: Point::new(
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0,
            ),
            cached: std::sync::Mutex::new(None),
        });
        let BorderPaths { regions, sectors } = free_ends.build_paths(ALL_RECTANGLE_EDGES)?;
        Ok(Self {
            regions,
            sectors,
            ring,
            clip,
            free_ends,
        })
    }

    /// 四边都连接时的区域，顺序为上、右、下、左。
    /// [`Self::paint`] 根据实际启用的边应用自由端平齐截面。
    pub fn regions(&self) -> &[Path; 4] {
        &self.regions
    }

    /// 一次轮廓覆盖配合像素中心的分区归属，避免相邻半透明颜色的 AA 接缝。
    /// 混合颜色使用已有 path-clip 与 per-operation-antialias capability。
    pub fn paint(
        &self,
        context: &mut crate::graphics::PaintContext<'_>,
        edges: RectangleEdges,
        inherited: Option<crate::graphics::Paint>,
    ) -> Result<(), crate::graphics::GraphicsError> {
        context.push_state();
        let result = (|| {
            let paints = edges.0.map(|edge| match edge {
                EdgeStroke::Inherit => inherited.clone(),
                EdgeStroke::Color(color) => Some(color.into()),
                EdgeStroke::None => None,
            });
            if paints.iter().all(|paint| paint == &paints[0]) {
                if let Some(paint) = &paints[0] {
                    context.set_fill_paint(paint.clone());
                    context.fill_path(&self.ring)?;
                }
                return Ok(());
            }
            let mask = paints.iter().enumerate().fold(0, |mask, (i, paint)| {
                mask | (u8::from(paint.is_some()) << i)
            });
            let free_paths = if mask != ALL_RECTANGLE_EDGES {
                Some(
                    self.free_ends
                        .paths(mask)
                        .map_err(ShapeGeometryError::drawing_error)?,
                )
            } else {
                None
            };
            let paths = if context.antialias() == crate::graphics::AntialiasMode::Off {
                free_paths
                    .as_ref()
                    .map_or(&self.regions, |paths| &paths.regions)
            } else {
                context.clip_path(&self.clip)?;
                context.set_antialias(crate::graphics::AntialiasMode::Off);
                free_paths
                    .as_ref()
                    .map_or(&self.sectors, |paths| &paths.sectors)
            };
            for (paint, region) in paints.into_iter().zip(paths) {
                if let Some(paint) = paint {
                    context.set_fill_paint(paint);
                    context.fill_path(region)?;
                }
            }
            Ok(())
        })();
        let restore = context.pop_state();
        result.and(restore)
    }
}

#[derive(Clone, Copy, Debug)]
struct Corner {
    entry: Point,
    mid: Point,
    exit: Point,
    radius: Dimension,
}

impl Corner {
    fn arc(self, path: &mut Path, end: Point, sweep: bool) {
        if self.radius.width > 0.0 && self.radius.height > 0.0 {
            path.arc_to_degrees(
                self.radius.width,
                self.radius.height,
                0.0,
                false,
                sweep,
                end.x(),
                end.y(),
            );
        } else {
            line_to(path, end);
        }
    }
}

fn rectangle_geometry(
    bounds: Rectangle,
    corners: RectangleCorners,
) -> Result<[Corner; 4], ShapeGeometryError> {
    finite(bounds.x)?;
    finite(bounds.y)?;
    metric(bounds.width)?;
    metric(bounds.height)?;
    finite(bounds.x + bounds.width)?;
    finite(bounds.y + bounds.height)?;
    let mut radii = corners.0;
    for r in &mut radii {
        if r.width == 0.0 || r.height == 0.0 {
            *r = Dimension::new(0.0, 0.0);
        }
    }
    let mut scale = 1.0_f64;
    for (length, sum) in [
        (bounds.width, radii[0].width + radii[1].width),
        (bounds.width, radii[2].width + radii[3].width),
        (bounds.height, radii[0].height + radii[3].height),
        (bounds.height, radii[1].height + radii[2].height),
    ] {
        if sum > 0.0 {
            finite(sum)?;
            scale = scale.min(length / sum);
        }
    }
    let vertices = [
        Point::new(bounds.x, bounds.y),
        Point::new(bounds.x + bounds.width, bounds.y),
        Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
        Point::new(bounds.x, bounds.y + bounds.height),
    ];
    const DIAGONAL_COMPONENT: f64 = std::f64::consts::FRAC_1_SQRT_2;
    Ok(std::array::from_fn(|i| {
        let v = vertices[i];
        let r = Dimension::new(radii[i].width * scale, radii[i].height * scale);
        let (sx, sy) = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)][i];
        let horizontal = Point::new(v.x() + sx * r.width, v.y());
        let vertical = Point::new(v.x(), v.y() + sy * r.height);
        let (entry, exit) = if i % 2 == 0 {
            (vertical, horizontal)
        } else {
            (horizontal, vertical)
        };
        Corner {
            entry,
            exit,
            radius: r,
            mid: Point::new(
                v.x() + sx * r.width * (1.0 - DIAGONAL_COMPONENT),
                v.y() + sy * r.height * (1.0 - DIAGONAL_COMPONENT),
            ),
        }
    }))
}

fn metric(v: f64) -> Result<(), ShapeGeometryError> {
    finite(v)?;
    if v < 0.0 {
        Err(ShapeGeometryError::NegativeMetric)
    } else {
        Ok(())
    }
}

fn finite(v: f64) -> Result<(), ShapeGeometryError> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(ShapeGeometryError::NonFinite)
    }
}

fn unit(x: f64, y: f64) -> Result<(f64, f64), ShapeGeometryError> {
    let length = x.hypot(y);
    finite(length)?;
    if length == 0.0 {
        return Err(ShapeGeometryError::DegeneratePolygon);
    }
    Ok((x / length, y / length))
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x() - b.x()).hypot(a.y() - b.y())
}

fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b.x() - a.x()) * (c.y() - a.y()) - (b.y() - a.y()) * (c.x() - a.x())
}

fn validate_polygon(points: &[Point]) -> Result<(), ShapeGeometryError> {
    let n = points.len();
    let mut area = 0.0;
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        if distance(a, b) == 0.0 {
            return Err(ShapeGeometryError::DegeneratePolygon);
        }
        area += cross(points[0], a, b);
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let c = points[j];
            let d = points[(j + 1) % n];
            let ab_c = cross(a, b, c);
            let ab_d = cross(a, b, d);
            let cd_a = cross(c, d, a);
            let cd_b = cross(c, d, b);
            for value in [ab_c, ab_d, cd_a, cd_b] {
                finite(value)?;
            }
            let overlap = a.x().min(b.x()) <= c.x().max(d.x())
                && c.x().min(d.x()) <= a.x().max(b.x())
                && a.y().min(b.y()) <= c.y().max(d.y())
                && c.y().min(d.y()) <= a.y().max(b.y());
            let straddles = |a: f64, b: f64| (a <= 0.0 && b >= 0.0) || (b <= 0.0 && a >= 0.0);
            if overlap && straddles(ab_c, ab_d) && straddles(cd_a, cd_b) {
                return Err(ShapeGeometryError::SelfIntersection);
            }
        }
    }
    finite(area)?;
    if area == 0.0 {
        Err(ShapeGeometryError::DegeneratePolygon)
    } else {
        Ok(())
    }
}

fn move_to(path: &mut Path, p: Point) {
    path.move_to(p.x(), p.y());
}
fn line_to(path: &mut Path, p: Point) {
    path.line_to(p.x(), p.y());
}

#[cfg(test)]
mod free_end_tests {
    use super::*;

    #[test]
    fn terminal_paths_are_shared_and_cache_retains_only_the_latest_mask() {
        let geometry = RectangleBorderGeometry::new(
            Rectangle::new(0.0, 0.0, 100.0, 80.0),
            RectangleCorners::ZERO,
            6.0,
        )
        .unwrap();
        let first = geometry.free_ends.paths(0b1011).unwrap();
        let same = geometry.free_ends.paths(0b1011).unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &same));
        let other = geometry.free_ends.paths(0b0001).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&first, &other));
        let cached = geometry.free_ends.cached.lock().unwrap();
        assert_eq!(cached.as_ref().unwrap().0, 0b0001);
        assert!(std::sync::Arc::ptr_eq(&other, &cached.as_ref().unwrap().1));
    }

    #[test]
    #[ignore = "release measurement of free-end construction and cached access"]
    fn free_end_path_cost() {
        use std::{hint::black_box, time::Instant};
        let mut cold = Vec::new();
        for _ in 0..100 {
            let geometry = RectangleBorderGeometry::new(
                Rectangle::new(0.0, 0.0, 800.0, 800.0),
                RectangleCorners::circular([30.0, 12.0, 0.0, 0.0]).unwrap(),
                10.0,
            )
            .unwrap();
            let start = Instant::now();
            black_box(geometry.free_ends.paths(0b1011).unwrap());
            cold.push(start.elapsed().as_secs_f64() * 1e6);
        }
        cold.sort_by(f64::total_cmp);
        let geometry = RectangleBorderGeometry::new(
            Rectangle::new(0.0, 0.0, 800.0, 800.0),
            RectangleCorners::ZERO,
            10.0,
        )
        .unwrap();
        geometry.free_ends.paths(0b1011).unwrap();
        let start = Instant::now();
        for _ in 0..100_000 {
            black_box(geometry.free_ends.paths(black_box(0b1011)).unwrap());
        }
        println!(
            "free-end cold p95={} us; warm={} ns/op; model={} bytes; path container={} bytes (owned Path heaps excluded)",
            cold[95],
            start.elapsed().as_secs_f64() * 1e9 / 100_000.0,
            std::mem::size_of::<FreeEndGeometry>(),
            std::mem::size_of::<BorderPaths>()
        );
    }
}
