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

use novadraw_geometry::{Point, Rectangle};
use novadraw_render::NdCanvas;

use crate::figure::{
    Bounded, ChildClippingStrategy, ChildPolicy, ChildTransform, Figure, Updatable, border::Border,
};
use crate::layout::{LayoutContext, LayoutManager};
use crate::{
    BlockId, DefaultRangeModel, FigureGraph, GraphMutationError, PropertyValue, RangeModel,
    RangeModelError, RangeModelSnapshot, UpdateManager,
};

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

    fn view_location(&self) -> Point {
        Point::new(self.horizontal.value(), self.vertical.value())
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
        let view_location = lock_unpoisoned(&self.runtime).view_location();
        let (top, left, _, _) = self.insets();
        ChildTransform::translation(
            self.bounds.x + left - view_location.x(),
            self.bounds.y + top - view_location.y(),
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
        let view_location = lock_unpoisoned(&self.runtime).view_location();
        let (top, left, bottom, right) = self.insets();
        Rectangle::new(
            view_location.x(),
            view_location.y(),
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
    pub fn viewport_handle(&self, block_id: BlockId) -> Option<ViewportHandle> {
        let viewport = self
            .block(block_id)?
            .figure
            .as_any()
            .downcast_ref::<ViewportFigure>()?;
        Some(ViewportHandle {
            block_id,
            runtime: Arc::clone(&viewport.runtime),
        })
    }

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
            Box::new(ViewportLayout::new(Arc::clone(&runtime))),
        );
        Ok(ViewportHandle { block_id, runtime })
    }
}
