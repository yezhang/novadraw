//! 视口管理
//!
//! 提供 viewport 坐标域与 content 坐标域之间的变换。
//!
//! 这里的 `content` 不是 Figure 树外的统一全局空间，而是某个 viewport
//! 管理的内容坐标域。未来如果 Viewport 作为 Figure 节点接入树结构，应通过
//! `translate_to_parent` / `translate_from_parent` 协议加入父链，而不是在事件或渲染入口
//! 额外添加全局空间特判。

use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use glam::DVec2;
use novadraw_geometry::{Point, Rectangle, Transform};
use novadraw_render::NdCanvas;

use crate::figure::{
    Bounded, ChildClippingStrategy, ChildPolicy, ChildTransform, Figure, Updatable, border::Border,
};
use crate::layout::{LayoutContext, LayoutManager};
use crate::{
    BlockId, DefaultRangeModel, FigureGraph, GraphMutationError, PropertyValue, RangeModel,
    RangeModelError, RangeModelSnapshot, UpdateManager,
};

fn is_valid_zoom(zoom: f64) -> bool {
    zoom.is_finite() && zoom > 0.0
}

const DEFAULT_RANGE_MAXIMUM: f64 = i32::MAX as f64;

struct ViewportRuntime {
    horizontal: Arc<dyn RangeModel>,
    vertical: Arc<dyn RangeModel>,
    tracks_width: bool,
    tracks_height: bool,
}

impl ViewportRuntime {
    fn new() -> Self {
        Self::with_models(
            Arc::new(
                DefaultRangeModel::new(0.0, 0.0, DEFAULT_RANGE_MAXIMUM)
                    .expect("default horizontal range is valid"),
            ),
            Arc::new(
                DefaultRangeModel::new(0.0, 0.0, DEFAULT_RANGE_MAXIMUM)
                    .expect("default vertical range is valid"),
            ),
        )
    }

    fn with_models(horizontal: Arc<dyn RangeModel>, vertical: Arc<dyn RangeModel>) -> Self {
        Self {
            horizontal,
            vertical,
            tracks_width: false,
            tracks_height: false,
        }
    }

    fn viewport(&self) -> Viewport {
        Viewport {
            origin: DVec2::new(self.horizontal.value(), self.vertical.value()),
            zoom: 1.0,
        }
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewportError {
    Graph(GraphMutationError),
    Range(RangeModelError),
    MissingViewport,
    InvalidViewLocation,
}

impl fmt::Display for ViewportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => error.fmt(f),
            Self::Range(error) => error.fmt(f),
            Self::MissingViewport => write!(f, "viewport block does not exist"),
            Self::InvalidViewLocation => write!(f, "view location must be finite"),
        }
    }
}

impl Error for ViewportError {}

impl From<GraphMutationError> for ViewportError {
    fn from(value: GraphMutationError) -> Self {
        Self::Graph(value)
    }
}

impl From<RangeModelError> for ViewportError {
    fn from(value: RangeModelError) -> Self {
        Self::Range(value)
    }
}

#[derive(Clone)]
pub struct ViewportHandle {
    block_id: BlockId,
    runtime: Arc<Mutex<ViewportRuntime>>,
}

impl ViewportHandle {
    pub fn block_id(&self) -> BlockId {
        self.block_id
    }

    pub fn contents(&self, graph: &FigureGraph) -> Option<BlockId> {
        graph
            .child_order(self.block_id)
            .and_then(|children| children.first().copied())
    }

    pub fn horizontal_range(&self) -> RangeModelSnapshot {
        lock_unpoisoned(&self.runtime).horizontal.snapshot()
    }

    pub fn vertical_range(&self) -> RangeModelSnapshot {
        lock_unpoisoned(&self.runtime).vertical.snapshot()
    }

    pub fn view_location(&self) -> Point {
        let runtime = lock_unpoisoned(&self.runtime);
        Point::new(runtime.horizontal.value(), runtime.vertical.value())
    }

    pub fn set_view_location(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        x: f64,
        y: f64,
    ) -> Result<bool, ViewportError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(ViewportError::InvalidViewLocation);
        }
        if graph.get_block(self.block_id).is_none() {
            return Err(ViewportError::MissingViewport);
        }

        let (old, new) = {
            let runtime = lock_unpoisoned(&self.runtime);
            let old = Point::new(runtime.horizontal.value(), runtime.vertical.value());
            runtime.horizontal.set_value(x)?;
            runtime.vertical.set_value(y)?;
            let new = Point::new(runtime.horizontal.value(), runtime.vertical.value());
            (old, new)
        };
        if old == new {
            return Ok(false);
        }

        graph.record_property_change(
            self.block_id,
            "viewLocation",
            PropertyValue::Point(old),
            PropertyValue::Point(new),
        );
        graph.record_coordinate_system_changed(self.block_id);
        graph.repaint(update_manager, self.block_id, None);
        Ok(true)
    }

    pub fn set_horizontal_location(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        x: f64,
    ) -> Result<bool, ViewportError> {
        let current = self.view_location();
        self.set_view_location(graph, update_manager, x, current.y())
    }

    pub fn set_vertical_location(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        y: f64,
    ) -> Result<bool, ViewportError> {
        let current = self.view_location();
        self.set_view_location(graph, update_manager, current.x(), y)
    }

    pub fn scroll_by(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        dx: f64,
        dy: f64,
    ) -> Result<bool, ViewportError> {
        let current = self.view_location();
        self.set_view_location(graph, update_manager, current.x() + dx, current.y() + dy)
    }

    pub fn contents_tracks_width(&self) -> bool {
        lock_unpoisoned(&self.runtime).tracks_width
    }

    pub fn contents_tracks_height(&self) -> bool {
        lock_unpoisoned(&self.runtime).tracks_height
    }

    pub fn set_contents(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        figure: Box<dyn Figure>,
    ) -> Result<BlockId, ViewportError> {
        if graph.get_block(self.block_id).is_none() {
            return Err(ViewportError::MissingViewport);
        }
        if let Some(previous) = self.contents(graph) {
            graph.remove_child(update_manager, self.block_id, previous);
        }
        let child = graph.try_add_child_to(self.block_id, figure)?;
        graph.mark_invalid(update_manager, self.block_id);
        graph.mark_invalid(update_manager, child);
        graph.repaint(update_manager, self.block_id, None);
        Ok(child)
    }

    pub fn set_tracks_width(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        tracks: bool,
    ) -> Result<bool, ViewportError> {
        self.set_track_policy(graph, update_manager, Some(tracks), None)
    }

    pub fn set_tracks_height(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        tracks: bool,
    ) -> Result<bool, ViewportError> {
        self.set_track_policy(graph, update_manager, None, Some(tracks))
    }

    fn set_track_policy(
        &self,
        graph: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        tracks_width: Option<bool>,
        tracks_height: Option<bool>,
    ) -> Result<bool, ViewportError> {
        if graph.get_block(self.block_id).is_none() {
            return Err(ViewportError::MissingViewport);
        }
        let changed = {
            let mut runtime = lock_unpoisoned(&self.runtime);
            let mut changed = false;
            if let Some(value) = tracks_width
                && runtime.tracks_width != value
            {
                runtime.tracks_width = value;
                changed = true;
            }
            if let Some(value) = tracks_height
                && runtime.tracks_height != value
            {
                runtime.tracks_height = value;
                changed = true;
            }
            changed
        };
        if changed {
            graph.mark_invalid(update_manager, self.block_id);
            graph.repaint(update_manager, self.block_id, None);
        }
        Ok(changed)
    }
}

#[derive(Clone)]
pub struct ViewportLayout {
    runtime: Arc<Mutex<ViewportRuntime>>,
}

impl ViewportLayout {
    fn new(runtime: Arc<Mutex<ViewportRuntime>>) -> Self {
        Self { runtime }
    }
}

impl LayoutManager for ViewportLayout {
    fn get_preferred_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        let Some((contents, _)) = ctx.get_children(container).first().copied() else {
            return (0.0, 0.0);
        };
        let runtime = lock_unpoisoned(&self.runtime);
        let width_hint = if runtime.tracks_width { w_hint } else { -1.0 };
        let height_hint = if runtime.tracks_height { h_hint } else { -1.0 };
        ctx.get_preferred_size(contents, width_hint, height_hint)
    }

    fn get_minimum_size(
        &self,
        _container: BlockId,
        _w_hint: f64,
        _h_hint: f64,
        _ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn layout(&self, container: BlockId, ctx: &mut dyn LayoutContext) {
        let Some((contents, _)) = ctx.get_children(container).first().copied() else {
            return;
        };
        let area = ctx.get_container_bounds(container);
        let (tracks_width, tracks_height) = {
            let runtime = lock_unpoisoned(&self.runtime);
            (runtime.tracks_width, runtime.tracks_height)
        };
        let preferred = ctx.get_preferred_size(contents, area.width, area.height);
        let minimum = ctx.get_minimum_size(contents, area.width, area.height);
        let width = if tracks_width {
            area.width.max(minimum.0)
        } else {
            area.width.max(preferred.0)
        };
        let height = if tracks_height {
            area.height.max(minimum.1)
        } else {
            area.height.max(preferred.1)
        };
        ctx.set_child_bounds(contents, Rectangle::new(0.0, 0.0, width, height));

        let runtime = lock_unpoisoned(&self.runtime);
        let _ = runtime.horizontal.set_all(0.0, area.width, width);
        let _ = runtime.vertical.set_all(0.0, area.height, height);
    }
}

/// 视口
///
/// 管理 content 坐标域的可见区域，支持平移和缩放。
///
/// `origin` 表示 viewport 左上角对应的 content 坐标。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub origin: DVec2,
    pub zoom: f64,
}

impl Viewport {
    /// 创建新视口
    pub fn new() -> Self {
        Self {
            origin: DVec2::ZERO,
            zoom: 1.0,
        }
    }

    /// 设置原点
    pub fn with_origin(mut self, x: f64, y: f64) -> Self {
        self.origin = DVec2::new(x, y);
        self
    }

    /// 设置缩放
    pub fn with_zoom(mut self, zoom: f64) -> Self {
        if is_valid_zoom(zoom) {
            self.zoom = zoom;
        }
        self
    }

    /// viewport 坐标转 content 坐标。
    ///
    /// 对齐 draw2d `Viewport.translateFromParent()` 的方向：从父/viewport 坐标进入内容坐标。
    pub fn viewport_to_content(&self, point: DVec2) -> DVec2 {
        (point / self.zoom) + self.origin
    }

    /// content 坐标转 viewport 坐标。
    ///
    /// 对齐 draw2d `Viewport.translateToParent()` 的方向：从内容坐标回到父/viewport 坐标。
    pub fn content_to_viewport(&self, point: DVec2) -> DVec2 {
        (point - self.origin) * self.zoom
    }

    /// 将点从内容坐标转换到父/viewport 坐标。
    pub fn translate_to_parent(&self, point: &mut DVec2) {
        *point = self.content_to_viewport(*point);
    }

    /// 将点从父/viewport 坐标转换到内容坐标。
    pub fn translate_from_parent(&self, point: &mut DVec2) {
        *point = self.viewport_to_content(*point);
    }

    /// 平移
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.origin -= DVec2::new(dx, dy) / self.zoom;
    }

    /// 以指定中心点缩放
    pub fn zoom_at(&mut self, factor: f64, center: DVec2) {
        if !is_valid_zoom(factor) || !center.is_finite() {
            return;
        }
        let content_center_before = self.viewport_to_content(center);
        let next_zoom = self.zoom * factor;
        if !is_valid_zoom(next_zoom) {
            return;
        }
        self.zoom = next_zoom;
        let content_center_after = self.viewport_to_content(center);
        let offset = content_center_before - content_center_after;
        self.origin += offset;
    }

    /// 缩放以适应矩形
    pub fn zoom_to_fit(
        &mut self,
        rect: &crate::Rectangle,
        viewport_width: f64,
        viewport_height: f64,
        padding: f64,
    ) {
        let available_width = viewport_width - padding * 2.0;
        let available_height = viewport_height - padding * 2.0;
        if !rect.x.is_finite()
            || !rect.y.is_finite()
            || !rect.width.is_finite()
            || !rect.height.is_finite()
            || !available_width.is_finite()
            || !available_height.is_finite()
            || !padding.is_finite()
            || padding < 0.0
            || rect.width <= 0.0
            || rect.height <= 0.0
            || available_width <= 0.0
            || available_height <= 0.0
        {
            return;
        }
        let zoom = (available_width / rect.width).min(available_height / rect.height);
        if !is_valid_zoom(zoom) {
            return;
        }
        let origin = DVec2::new(rect.x - padding / zoom, rect.y - padding / zoom);
        if !origin.is_finite() {
            return;
        }
        self.zoom = zoom;
        self.origin = origin;
    }

    /// 放大
    pub fn zoom_in(&mut self, factor: f64) {
        if is_valid_zoom(factor) {
            self.set_zoom(self.zoom * factor);
        }
    }

    /// 缩小
    pub fn zoom_out(&mut self, factor: f64) {
        if is_valid_zoom(factor) {
            self.set_zoom(self.zoom / factor);
        }
    }

    /// 设置原点
    pub fn set_origin(&mut self, x: f64, y: f64) {
        self.origin = DVec2::new(x, y);
    }

    /// 设置缩放
    pub fn set_zoom(&mut self, zoom: f64) {
        if is_valid_zoom(zoom) {
            self.zoom = zoom;
        }
    }

    /// 转换为变换矩阵
    ///
    /// 变换公式: viewport = (content - origin) * zoom
    /// 即: 先平移 `-origin`，再缩放
    /// 使用 `*` 运算符：T(translate) * S(scale) = 先 S，后 T
    pub fn to_transform(&self) -> Transform {
        let scale = Transform::from_scale(self.zoom, self.zoom);
        let translate = Transform::from_translation(-self.origin.x, -self.origin.y);
        scale * translate // S * T = 先平移 origin，后缩放
    }

    /// 转换为逆变换
    ///
    /// 逆变换公式: content = viewport / zoom + origin
    pub fn to_inverse_transform(&self) -> Transform {
        let inv_zoom = 1.0 / self.zoom;
        let scale = Transform::from_scale(inv_zoom, inv_zoom);
        let translate = Transform::from_translation(self.origin.x, self.origin.y);
        translate * scale // T * S = 先缩放回 content 增量，后加 origin
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}

/// Draw2D 风格的 Viewport Figure。
///
/// `ViewportFigure` 是 Figure 树中的坐标根和裁剪容器：自身 bounds 位于父坐标域，
/// 子节点位于 content 坐标域。
#[derive(Clone)]
pub struct ViewportFigure {
    bounds: Rectangle,
    runtime: Arc<Mutex<ViewportRuntime>>,
    child_clipping_strategy: ChildClippingStrategy,
    border: Option<Arc<dyn Border>>,
}

impl ViewportFigure {
    /// 创建 Viewport Figure。
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::with_runtime(
            Rectangle::new(x, y, width, height),
            Arc::new(Mutex::new(ViewportRuntime::new())),
        )
    }

    fn with_runtime(bounds: Rectangle, runtime: Arc<Mutex<ViewportRuntime>>) -> Self {
        Self {
            bounds,
            runtime,
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 设置 content origin。
    pub fn with_origin(self, x: f64, y: f64) -> Self {
        {
            let runtime = lock_unpoisoned(&self.runtime);
            let _ = runtime.horizontal.set_value(x);
            let _ = runtime.vertical.set_value(y);
        }
        self
    }

    /// 设置子节点绘制裁剪策略。
    pub fn with_child_clipping_strategy(mut self, strategy: ChildClippingStrategy) -> Self {
        self.child_clipping_strategy = strategy;
        self
    }

    /// 添加边框装饰器。
    pub fn with_border(mut self, border: impl Border + 'static) -> Self {
        self.border = Some(Arc::new(border));
        self
    }

    /// 返回当前 viewport helper。
    pub fn viewport(&self) -> Viewport {
        lock_unpoisoned(&self.runtime).viewport()
    }
}

impl Bounded for ViewportFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "ViewportFigure"
    }

    fn use_local_coordinates(&self) -> bool {
        true
    }

    fn child_transform(&self) -> ChildTransform {
        let viewport = lock_unpoisoned(&self.runtime).viewport();
        let (top, left, _, _) = self.insets();
        ChildTransform::translation(
            self.bounds.x + left - viewport.origin.x,
            self.bounds.y + top - viewport.origin.y,
        )
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Single
    }

    fn insets(&self) -> (f64, f64, f64, f64) {
        self.border
            .as_ref()
            .map(|border| border.get_insets())
            .unwrap_or((0.0, 0.0, 0.0, 0.0))
    }

    fn client_area(&self) -> Rectangle {
        let viewport = lock_unpoisoned(&self.runtime).viewport();
        let (top, left, bottom, right) = self.insets();
        Rectangle::new(
            viewport.origin.x,
            viewport.origin.y,
            (self.bounds.width - left - right).max(0.0),
            (self.bounds.height - top - bottom).max(0.0),
        )
    }
}

impl Updatable for ViewportFigure {
    fn validate(&mut self) {}
}

impl Figure for ViewportFigure {
    fn paint_figure(&self, _gc: &mut NdCanvas) {}

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }
}

impl FigureGraph {
    /// Adds a Viewport Figure and returns its typed transactional handle.
    pub fn add_viewport_to(
        &mut self,
        parent: BlockId,
        bounds: Rectangle,
    ) -> Result<ViewportHandle, GraphMutationError> {
        self.add_viewport_with_models_to(
            parent,
            bounds,
            Arc::new(
                DefaultRangeModel::new(0.0, 0.0, DEFAULT_RANGE_MAXIMUM)
                    .expect("default horizontal range is valid"),
            ),
            Arc::new(
                DefaultRangeModel::new(0.0, 0.0, DEFAULT_RANGE_MAXIMUM)
                    .expect("default vertical range is valid"),
            ),
        )
    }

    pub(crate) fn add_viewport_with_models_to(
        &mut self,
        parent: BlockId,
        bounds: Rectangle,
        horizontal: Arc<dyn RangeModel>,
        vertical: Arc<dyn RangeModel>,
    ) -> Result<ViewportHandle, GraphMutationError> {
        let runtime = Arc::new(Mutex::new(ViewportRuntime::with_models(
            horizontal, vertical,
        )));
        let figure = ViewportFigure::with_runtime(bounds, Arc::clone(&runtime));
        let block_id = self.try_add_child_to(parent, Box::new(figure))?;
        self.set_block_layout_manager(
            block_id,
            Arc::new(ViewportLayout::new(Arc::clone(&runtime))),
        );
        Ok(ViewportHandle { block_id, runtime })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_content_conversion() {
        let viewport = Viewport::new().with_origin(100.0, 200.0).with_zoom(2.0);
        let content = DVec2::new(150.0, 250.0);
        let viewport_point = viewport.content_to_viewport(content);
        // viewport = (content - origin) * zoom
        // zoom=2, origin=(100, 200), content=(150, 250)
        // viewport = (150-100, 250-200) * 2 = (100, 100)
        assert_eq!(viewport_point, DVec2::new(100.0, 100.0));
        let back = viewport.viewport_to_content(viewport_point);
        assert_eq!(back, content);
    }

    #[test]
    fn test_translate_parent_protocol() {
        let viewport = Viewport::new().with_origin(100.0, 200.0).with_zoom(2.0);

        let mut point = DVec2::new(150.0, 250.0);
        viewport.translate_to_parent(&mut point);
        assert_eq!(point, DVec2::new(100.0, 100.0));

        viewport.translate_from_parent(&mut point);
        assert_eq!(point, DVec2::new(150.0, 250.0));
    }

    #[test]
    fn test_pan() {
        let mut viewport = Viewport::new().with_origin(100.0, 100.0).with_zoom(2.0);
        viewport.pan(100.0, 100.0);
        assert_eq!(viewport.origin, DVec2::new(50.0, 50.0));
    }

    #[test]
    fn test_zoom_at() {
        let mut viewport = Viewport::new().with_origin(0.0, 0.0).with_zoom(1.0);
        viewport.zoom_at(2.0, DVec2::new(100.0, 100.0));
        assert_eq!(viewport.zoom, 2.0);
    }

    #[test]
    fn test_zoom_in_out() {
        let mut viewport = Viewport::new().with_zoom(1.0);
        viewport.zoom_in(2.0);
        assert_eq!(viewport.zoom, 2.0);
        viewport.zoom_out(2.0);
        assert_eq!(viewport.zoom, 1.0);
    }

    #[test]
    fn test_to_transform_identity() {
        let viewport = Viewport::new().with_origin(0.0, 0.0).with_zoom(1.0);
        let transform = viewport.to_transform();
        let point = glam::DVec2::new(100.0, 200.0);
        let transformed = transform.transform_point(point.x, point.y);
        assert!((transformed.0 - point.x).abs() < 1e-10);
        assert!((transformed.1 - point.y).abs() < 1e-10);
    }

    #[test]
    fn test_to_transform_scale() {
        let viewport = Viewport::new().with_origin(0.0, 0.0).with_zoom(2.0);
        let transform = viewport.to_transform();
        let point = glam::DVec2::new(100.0, 200.0);
        let transformed = transform.transform_point(point.x, point.y);
        // viewport = (content - origin) * zoom = (100-0, 200-0) * 2 = (200, 400)
        assert_eq!(transformed.0, 200.0);
        assert_eq!(transformed.1, 400.0);
    }

    #[test]
    fn test_to_transform_with_non_zero_origin() {
        let viewport = Viewport::new().with_origin(100.0, 200.0).with_zoom(2.0);
        let transform = viewport.to_transform();
        let inverse = viewport.to_inverse_transform();

        let content = DVec2::new(150.0, 250.0);
        let transformed = transform.transform_point(content.x, content.y);
        assert_eq!(transformed, (100.0, 100.0));

        let restored = inverse.transform_point(transformed.0, transformed.1);
        assert_eq!(restored, (150.0, 250.0));
    }
}
