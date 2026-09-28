//! 视口管理
//!
//! 提供 viewport 坐标域与 content 坐标域之间的变换。
//!
//! 这里的 `content` 不是 Figure 树外的统一全局空间，而是某个 viewport
//! 管理的内容坐标域。未来如果 Viewport 作为 Figure 节点接入树结构，应通过
//! `local_to_parent_transform` / `parent_to_local_transform` 协议加入父链，而不是在事件或渲染入口
//! 额外添加全局空间特判。

use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use novadraw_geometry::{Dimension, Point, Rectangle};
use novadraw_render::NdCanvas;

use super::range_model::normalize_range;
use crate::figure::{
    BorderedFigure, Bounded, ChildClippingStrategy, ChildPolicy, ChildTransform, Figure,
    FigureContainer, FigureMeasurement, MeasureConstraints, border::Border,
};
use crate::layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot};
use crate::{
    DefaultRangeModel, FigureId, FigureTree, FigureTreeBuilder, GraphMutationError, PropertyValue,
    RangeModel, RangeModelError, RangeModelSnapshot, UpdateManager,
};

const DEFAULT_RANGE_MAXIMUM: f64 = i32::MAX as f64;

struct ViewportRuntime {
    horizontal: Arc<dyn RangeModel>,
    vertical: Arc<dyn RangeModel>,
    tracks_width: bool,
    tracks_height: bool,
    content_scale: f64,
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
            content_scale: 1.0,
        }
    }

    fn view_location(&self) -> Point {
        Point::new(self.horizontal.value(), self.vertical.value())
    }
}

#[derive(Clone)]
pub(crate) struct ViewportLayoutEffect {
    viewport: FigureId,
    contents: FigureId,
    runtime: Arc<Mutex<ViewportRuntime>>,
    content_scale: f64,
    horizontal: RangeModelSnapshot,
    vertical: RangeModelSnapshot,
}

impl fmt::Debug for ViewportLayoutEffect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ViewportLayoutEffect")
            .field("viewport", &self.viewport)
            .field("contents", &self.contents)
            .field("content_scale", &self.content_scale)
            .field("horizontal", &self.horizontal)
            .field("vertical", &self.vertical)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ViewportLayoutEffect {
    fn eq(&self, other: &Self) -> bool {
        self.viewport == other.viewport
            && self.contents == other.contents
            && Arc::ptr_eq(&self.runtime, &other.runtime)
            && self.content_scale == other.content_scale
            && self.horizontal == other.horizontal
            && self.vertical == other.vertical
    }
}

impl ViewportLayoutEffect {
    pub(crate) fn viewport(&self) -> FigureId {
        self.viewport
    }

    pub(crate) fn commit(&self) -> Result<(), LayoutError> {
        let mut runtime = lock_unpoisoned(&self.runtime);
        runtime
            .horizontal
            .set_all(
                self.horizontal.minimum,
                self.horizontal.extent,
                self.horizontal.maximum,
            )
            .map_err(|_| LayoutError::NonFiniteGeometry {
                figure: self.contents,
            })?;
        runtime
            .vertical
            .set_all(
                self.vertical.minimum,
                self.vertical.extent,
                self.vertical.maximum,
            )
            .map_err(|_| LayoutError::NonFiniteGeometry {
                figure: self.contents,
            })?;
        runtime.content_scale = self.content_scale;
        Ok(())
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn record_range_changes(
    out: &mut LayoutOutput,
    viewport: FigureId,
    old_horizontal: RangeModelSnapshot,
    old_vertical: RangeModelSnapshot,
    new_horizontal: RangeModelSnapshot,
    new_vertical: RangeModelSnapshot,
) {
    if old_horizontal == new_horizontal && old_vertical == new_vertical {
        return;
    }
    if old_horizontal.value != new_horizontal.value || old_vertical.value != new_vertical.value {
        out.record_property_change(
            viewport,
            "viewLocation",
            PropertyValue::Point(Point::new(old_horizontal.value, old_vertical.value)),
            PropertyValue::Point(Point::new(new_horizontal.value, new_vertical.value)),
        );
        out.coordinate_system_changed(viewport);
    }
    out.repaint(viewport);
    out.repaint_parent(viewport);
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
    figure_id: FigureId,
    runtime: Arc<Mutex<ViewportRuntime>>,
}

impl ViewportHandle {
    pub fn figure_id(&self) -> FigureId {
        self.figure_id
    }

    pub fn contents(&self, graph: &FigureTree) -> Option<FigureId> {
        graph
            .child_order(self.figure_id)
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

    pub(crate) fn set_view_location(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        x: f64,
        y: f64,
    ) -> Result<bool, ViewportError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(ViewportError::InvalidViewLocation);
        }
        if graph.node(self.figure_id).is_none() {
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
            self.figure_id,
            "viewLocation",
            PropertyValue::Point(old),
            PropertyValue::Point(new),
        );
        graph.record_coordinate_system_changed(self.figure_id);
        graph.repaint(update_manager, self.figure_id, None);
        Ok(true)
    }

    pub fn contents_tracks_width(&self) -> bool {
        lock_unpoisoned(&self.runtime).tracks_width
    }

    pub fn contents_tracks_height(&self) -> bool {
        lock_unpoisoned(&self.runtime).tracks_height
    }

    pub(crate) fn set_contents(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        figure: Box<dyn Figure>,
    ) -> Result<FigureId, ViewportError> {
        if graph.node(self.figure_id).is_none() {
            return Err(ViewportError::MissingViewport);
        }
        if let Some(previous) = self.contents(graph) {
            graph.remove_child(update_manager, self.figure_id, previous);
        }
        let child = graph.try_add_child_to(self.figure_id, figure)?;
        graph.mark_invalid(update_manager, self.figure_id);
        graph.mark_invalid(update_manager, child);
        graph.repaint(update_manager, self.figure_id, None);
        Ok(child)
    }

    pub(crate) fn set_tracks_width(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        tracks: bool,
    ) -> Result<bool, ViewportError> {
        self.set_track_policy(graph, update_manager, Some(tracks), None)
    }

    pub(crate) fn set_tracks_height(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        tracks: bool,
    ) -> Result<bool, ViewportError> {
        self.set_track_policy(graph, update_manager, None, Some(tracks))
    }

    fn set_track_policy(
        &self,
        graph: &mut FigureTree,
        update_manager: &mut UpdateManager,
        tracks_width: Option<bool>,
        tracks_height: Option<bool>,
    ) -> Result<bool, ViewportError> {
        if graph.node(self.figure_id).is_none() {
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
            graph.mark_invalid(update_manager, self.figure_id);
            graph.repaint(update_manager, self.figure_id, None);
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
    fn preferred_measurement(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement {
        let Some((contents, _)) = snapshot.children(container).first().copied() else {
            return FigureMeasurement::default();
        };
        let runtime = lock_unpoisoned(&self.runtime);
        let child_constraints = MeasureConstraints::new(
            runtime
                .tracks_width
                .then(|| constraints.max_width())
                .flatten(),
            runtime
                .tracks_height
                .then(|| constraints.max_height())
                .flatten(),
        )
        .expect("filtered measurement constraints remain valid");
        snapshot.preferred_measurement(contents, child_constraints)
    }

    fn minimum_size(
        &self,
        _container: FigureId,
        _constraints: MeasureConstraints,
        _snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension {
        Dimension::ZERO
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let Some((contents, _)) = snapshot.children(container).first().copied() else {
            return Ok(());
        };
        let area = snapshot.container_bounds(container);
        if let Some(freeform_extent) = snapshot.freeform_extent(contents) {
            let scale = snapshot.content_scale(contents).unwrap_or(1.0);
            if !scale.is_finite() || scale <= 0.0 {
                return Err(LayoutError::NonFiniteGeometry { figure: contents });
            }
            let baseline = Rectangle::new(0.0, 0.0, area.width / scale, area.height / scale);
            let envelope = freeform_extent.union(baseline);
            let horizontal_maximum = envelope.x + envelope.width;
            let vertical_maximum = envelope.y + envelope.height;
            if !baseline.width.is_finite()
                || !baseline.height.is_finite()
                || !envelope.x.is_finite()
                || !envelope.y.is_finite()
                || !horizontal_maximum.is_finite()
                || !vertical_maximum.is_finite()
            {
                return Err(LayoutError::NonFiniteGeometry { figure: contents });
            }
            out.set_child_bounds(
                contents,
                Rectangle::new(0.0, 0.0, baseline.width, baseline.height),
            );
            let runtime = lock_unpoisoned(&self.runtime);
            let old_horizontal = runtime.horizontal.snapshot();
            let old_vertical = runtime.vertical.snapshot();
            let new_horizontal = normalize_range(
                envelope.x,
                baseline.width,
                horizontal_maximum,
                old_horizontal.value,
            )
            .map_err(|_| LayoutError::NonFiniteGeometry { figure: contents })?;
            let new_vertical = normalize_range(
                envelope.y,
                baseline.height,
                vertical_maximum,
                old_vertical.value,
            )
            .map_err(|_| LayoutError::NonFiniteGeometry { figure: contents })?;
            drop(runtime);
            out.set_viewport_effect(ViewportLayoutEffect {
                viewport: container,
                contents,
                runtime: Arc::clone(&self.runtime),
                content_scale: scale,
                horizontal: new_horizontal,
                vertical: new_vertical,
            });
            record_range_changes(
                out,
                container,
                old_horizontal,
                old_vertical,
                new_horizontal,
                new_vertical,
            );
            return Ok(());
        }

        let (tracks_width, tracks_height) = {
            let runtime = lock_unpoisoned(&self.runtime);
            (runtime.tracks_width, runtime.tracks_height)
        };
        let constraints = MeasureConstraints::bounded(area.width, area.height)
            .expect("viewport client area is valid layout geometry");
        let preferred = snapshot.preferred_measurement(contents, constraints).size();
        let minimum = snapshot.minimum_size(contents, constraints);
        let width = if tracks_width {
            area.width.max(minimum.width)
        } else {
            area.width.max(preferred.width)
        };
        let height = if tracks_height {
            area.height.max(minimum.height)
        } else {
            area.height.max(preferred.height)
        };
        out.set_child_bounds(contents, Rectangle::new(0.0, 0.0, width, height));

        let runtime = lock_unpoisoned(&self.runtime);
        let old_horizontal = runtime.horizontal.snapshot();
        let old_vertical = runtime.vertical.snapshot();
        let new_horizontal = normalize_range(0.0, area.width, width, old_horizontal.value)
            .map_err(|_| LayoutError::NonFiniteGeometry { figure: contents })?;
        let new_vertical = normalize_range(0.0, area.height, height, old_vertical.value)
            .map_err(|_| LayoutError::NonFiniteGeometry { figure: contents })?;
        drop(runtime);
        out.set_viewport_effect(ViewportLayoutEffect {
            viewport: container,
            contents,
            runtime: Arc::clone(&self.runtime),
            content_scale: 1.0,
            horizontal: new_horizontal,
            vertical: new_vertical,
        });
        record_range_changes(
            out,
            container,
            old_horizontal,
            old_vertical,
            new_horizontal,
            new_vertical,
        );
        Ok(())
    }

    fn requires_valid_children_before_layout(&self) -> bool {
        true
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

    fn child_transform(&self) -> ChildTransform {
        let runtime = lock_unpoisoned(&self.runtime);
        let view_location = runtime.view_location();
        ChildTransform::translation(
            -view_location.x() * runtime.content_scale,
            -view_location.y() * runtime.content_scale,
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
        let (top, left, bottom, right) = self.insets();
        Rectangle::new(
            left,
            top,
            (self.bounds.width - left - right).max(0.0),
            (self.bounds.height - top - bottom).max(0.0),
        )
    }
}

impl Figure for ViewportFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ViewportFigure"
    }

    fn initial_insets(&self) -> (f64, f64, f64, f64) {
        Bounded::insets(self)
    }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        gc.fill_rectangle(0.0, 0.0, self.bounds.width, self.bounds.height);
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        gc.fill_rectangle(0.0, 0.0, bounds.width, bounds.height);
    }

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn bordered_mut(&mut self) -> Option<&mut dyn BorderedFigure> {
        Some(self)
    }
}

impl FigureContainer for ViewportFigure {
    fn child_transform(&self) -> ChildTransform {
        Bounded::child_transform(self)
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Single
    }
}

impl BorderedFigure for ViewportFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }

    fn replace_border(&mut self, border: Option<Arc<dyn Border>>) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border)
    }
}

impl FigureTree {
    pub fn viewport_handle(&self, figure_id: FigureId) -> Option<ViewportHandle> {
        let viewport = self
            .node(figure_id)?
            .figure
            .as_any()
            .downcast_ref::<ViewportFigure>()?;
        Some(ViewportHandle {
            figure_id,
            runtime: Arc::clone(&viewport.runtime),
        })
    }

    /// Adds a Viewport Figure and returns its typed transactional handle.
    pub(crate) fn add_viewport_to(
        &mut self,
        parent: FigureId,
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
        parent: FigureId,
        bounds: Rectangle,
        horizontal: Arc<dyn RangeModel>,
        vertical: Arc<dyn RangeModel>,
    ) -> Result<ViewportHandle, GraphMutationError> {
        let runtime = Arc::new(Mutex::new(ViewportRuntime::with_models(
            horizontal, vertical,
        )));
        let figure = ViewportFigure::with_runtime(bounds, Arc::clone(&runtime));
        let figure_id = self.try_add_child_to(parent, Box::new(figure))?;
        self.replace_layout_manager(
            figure_id,
            Some(Box::new(ViewportLayout::new(Arc::clone(&runtime)))),
        );
        Ok(ViewportHandle { figure_id, runtime })
    }
}

impl FigureTreeBuilder<'_> {
    pub fn add_viewport_to(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
    ) -> Result<ViewportHandle, GraphMutationError> {
        self.tree_mut().add_viewport_to(parent, bounds)
    }

    pub fn set_view_location(
        &mut self,
        viewport: FigureId,
        x: f64,
        y: f64,
    ) -> Result<bool, ViewportError> {
        let handle = self
            .tree_mut()
            .viewport_handle(viewport)
            .ok_or(ViewportError::MissingViewport)?;
        let mut updates = UpdateManager::with_namespace(self.tree_mut().namespace());
        handle.set_view_location(self.tree_mut(), &mut updates, x, y)
    }

    pub fn set_viewport_tracks_width(
        &mut self,
        viewport: FigureId,
        tracks: bool,
    ) -> Result<bool, ViewportError> {
        let handle = self
            .tree_mut()
            .viewport_handle(viewport)
            .ok_or(ViewportError::MissingViewport)?;
        let mut updates = UpdateManager::with_namespace(self.tree_mut().namespace());
        handle.set_tracks_width(self.tree_mut(), &mut updates, tracks)
    }

    pub fn set_viewport_tracks_height(
        &mut self,
        viewport: FigureId,
        tracks: bool,
    ) -> Result<bool, ViewportError> {
        let handle = self
            .tree_mut()
            .viewport_handle(viewport)
            .ok_or(ViewportError::MissingViewport)?;
        let mut updates = UpdateManager::with_namespace(self.tree_mut().namespace());
        handle.set_tracks_height(self.tree_mut(), &mut updates, tracks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingRangeListener(Arc<AtomicUsize>);

    impl crate::RangeListener for CountingRangeListener {
        fn range_changed(&self, _change: crate::RangeChange) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct InvalidViewportLayout {
        inner: ViewportLayout,
        invalid_child: FigureId,
    }

    impl LayoutManager for InvalidViewportLayout {
        fn preferred_measurement(
            &self,
            container: FigureId,
            constraints: MeasureConstraints,
            snapshot: &LayoutSnapshot<'_>,
        ) -> FigureMeasurement {
            self.inner
                .preferred_measurement(container, constraints, snapshot)
        }

        fn minimum_size(
            &self,
            container: FigureId,
            constraints: MeasureConstraints,
            snapshot: &LayoutSnapshot<'_>,
        ) -> Dimension {
            self.inner.minimum_size(container, constraints, snapshot)
        }

        fn layout(
            &mut self,
            container: FigureId,
            snapshot: &LayoutSnapshot<'_>,
            out: &mut LayoutOutput,
        ) -> Result<(), LayoutError> {
            self.inner.layout(container, snapshot, out)?;
            out.set_child_bounds(self.invalid_child, Rectangle::ZERO);
            Ok(())
        }

        fn requires_valid_children_before_layout(&self) -> bool {
            true
        }
    }

    #[test]
    fn invalid_layout_output_does_not_commit_viewport_range_effect() {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(crate::RectangleFigure::new(
                0.0, 0.0, 800.0, 600.0,
            )));
        let viewport = tree
            .builder()
            .add_viewport_to(root, Rectangle::new(0.0, 0.0, 300.0, 200.0))
            .unwrap();
        let mut updates = UpdateManager::new();
        viewport
            .set_contents(
                &mut tree,
                &mut updates,
                Box::new(crate::RectangleFigure::new(0.0, 0.0, 600.0, 450.0)),
            )
            .unwrap();
        tree.revalidate(viewport.figure_id());
        let before_horizontal = viewport.horizontal_range();
        let before_vertical = viewport.vertical_range();
        let notifications = Arc::new(AtomicUsize::new(0));
        let runtime = lock_unpoisoned(&viewport.runtime);
        runtime
            .horizontal
            .add_listener(Arc::new(CountingRangeListener(Arc::clone(&notifications))));
        runtime
            .vertical
            .add_listener(Arc::new(CountingRangeListener(Arc::clone(&notifications))));
        drop(runtime);

        tree.set_bounds(viewport.figure_id(), 0.0, 0.0, 120.0, 90.0);
        tree.replace_layout_manager(
            viewport.figure_id(),
            Some(Box::new(InvalidViewportLayout {
                inner: ViewportLayout::new(Arc::clone(&viewport.runtime)),
                invalid_child: root,
            })),
        );
        tree.mark_invalid(&mut updates, viewport.figure_id());
        updates.perform_validation(&mut tree);

        assert!(matches!(
            updates.last_validation_error(),
            Some(crate::ValidationError::Layout(
                LayoutError::InvalidChild { .. }
            ))
        ));
        assert_eq!(viewport.horizontal_range(), before_horizontal);
        assert_eq!(viewport.vertical_range(), before_vertical);
        assert_eq!(notifications.load(Ordering::Relaxed), 0);
    }
}
