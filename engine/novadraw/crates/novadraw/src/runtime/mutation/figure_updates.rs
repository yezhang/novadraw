use std::sync::Arc;

use crate::event::property;
use crate::figure::{
    Alignment, BORDER, Border, BorderCapability, CLICKABLE, ClickableCapability, Direction,
    FlowPage, FlowWrapping, ImageFigure, LabelFigure, PolygonScaleMode, PolylineFigure,
    RoundedRectangleFigure, ScalablePolygonFigure, ShapeMutationError, TextFlowFigure,
    TextPlacement, TriangleFigure, WidgetError,
};
use crate::geometry::{Dimension, Point, PointList, Rectangle};
use crate::render::{DashPattern, LineCap, LineJoin, StrokeStyle};
use crate::runtime::{
    FigureCapabilityContext, FigureCapabilityUpdate, FigureComponentContext, FigureComponentUpdate,
    FigureUpdateEffect, FigureUpdateEffectContext, PreparedCapabilityUpdate, PreparedFigureUpdate,
};
use crate::{Figure, ImageId, RuntimeMutationError};

pub struct SetLabelText(String);

impl SetLabelText {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }
}

impl FigureComponentUpdate for SetLabelText {
    type Figure = LabelFigure;
    type Prepared = String;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.text().to_owned();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(
            PreparedFigureUpdate::layout_geometry_and_paint(self.0.clone()).with_property_change(
                property::TEXT,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.set_text(prepared);
    }
}

pub struct SetLabelIcon(Option<ImageId>);

impl SetLabelIcon {
    pub const fn new(icon: Option<ImageId>) -> Self {
        Self(icon)
    }
}

impl FigureComponentUpdate for SetLabelIcon {
    type Figure = LabelFigure;
    type Prepared = Option<ImageId>;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.icon();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(PreparedFigureUpdate::layout_geometry_and_paint(self.0)
            .with_property_change(property::ICON, old, self.0)
            .with_resource_dependency_change(
                old.map(ImageId::resource_id),
                self.0.map(ImageId::resource_id),
            ))
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.set_icon(prepared);
    }
}

pub struct SetLabelTextPlacement(TextPlacement);

impl SetLabelTextPlacement {
    pub const fn new(placement: TextPlacement) -> Self {
        Self(placement)
    }
}

impl FigureComponentUpdate for SetLabelTextPlacement {
    type Figure = LabelFigure;
    type Prepared = TextPlacement;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.text_placement();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(
            PreparedFigureUpdate::layout_geometry_and_paint(self.0).with_property_change(
                property::TEXT_PLACEMENT,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.set_text_placement(prepared);
    }
}

macro_rules! label_alignment_update {
    ($name:ident, $getter:ident, $setter:ident, $property:expr) => {
        pub struct $name(Alignment);

        impl $name {
            pub const fn new(alignment: Alignment) -> Self {
                Self(alignment)
            }
        }

        impl FigureComponentUpdate for $name {
            type Figure = LabelFigure;
            type Prepared = Alignment;
            type Error = ShapeMutationError;

            fn prepare(
                self,
                current: &Self::Figure,
                _context: FigureComponentContext,
            ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
                let old = current.$getter();
                if old == self.0 {
                    return Ok(PreparedFigureUpdate::unchanged(self.0));
                }
                Ok(
                    PreparedFigureUpdate::paint(self.0)
                        .with_property_change($property, old, self.0),
                )
            }

            fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
                target.$setter(prepared);
            }
        }
    };
}

label_alignment_update!(
    SetLabelAlignment,
    label_alignment,
    set_label_alignment,
    property::LABEL_ALIGNMENT
);
label_alignment_update!(
    SetLabelTextAlignment,
    text_alignment,
    set_text_alignment,
    property::TEXT_ALIGNMENT
);
label_alignment_update!(
    SetLabelIconAlignment,
    icon_alignment,
    set_icon_alignment,
    property::ICON_ALIGNMENT
);

pub struct SetLabelIconTextGap(f64);

impl SetLabelIconTextGap {
    pub const fn new(gap: f64) -> Self {
        Self(gap)
    }
}

impl FigureComponentUpdate for SetLabelIconTextGap {
    type Figure = LabelFigure;
    type Prepared = f64;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if !self.0.is_finite() {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        if self.0 < 0.0 {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let old = current.icon_text_gap();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(
            PreparedFigureUpdate::layout_geometry_and_paint(self.0).with_property_change(
                property::ICON_TEXT_GAP,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.set_icon_text_gap(prepared);
    }
}

pub struct SetImageSource(ImageId);

impl SetImageSource {
    pub const fn new(image: ImageId) -> Self {
        Self(image)
    }
}

impl FigureComponentUpdate for SetImageSource {
    type Figure = ImageFigure;
    type Prepared = ImageId;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.image();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(PreparedFigureUpdate::layout_geometry_and_paint(self.0)
            .with_property_change(property::IMAGE, old, self.0)
            .with_resource_dependency_change(Some(old.resource_id()), Some(self.0.resource_id())))
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.set_image(prepared);
    }
}

pub struct SetImageAlignment(Alignment);

impl SetImageAlignment {
    pub const fn new(alignment: Alignment) -> Self {
        Self(alignment)
    }
}

impl FigureComponentUpdate for SetImageAlignment {
    type Figure = ImageFigure;
    type Prepared = Alignment;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.alignment();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(PreparedFigureUpdate::paint(self.0).with_property_change(
            property::IMAGE_ALIGNMENT,
            old,
            self.0,
        ))
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.set_alignment(prepared);
    }
}

pub struct ReplaceScalablePolygonTemplate(PointList);

impl ReplaceScalablePolygonTemplate {
    pub fn new(template: PointList) -> Self {
        Self(template)
    }
}

impl FigureComponentUpdate for ReplaceScalablePolygonTemplate {
    type Figure = ScalablePolygonFigure;
    type Prepared = PointList;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if self
            .0
            .iter()
            .any(|point| !point.x().is_finite() || !point.y().is_finite())
        {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        let old = current.template().clone();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(
            PreparedFigureUpdate::geometry_and_paint(self.0.clone()).with_property_change(
                property::POLYGON_TEMPLATE,
                old.into_vec(),
                self.0.clone().into_vec(),
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.replace_template(prepared);
    }
}

pub struct SetScalablePolygonScaleMode(PolygonScaleMode);

impl SetScalablePolygonScaleMode {
    pub const fn new(mode: PolygonScaleMode) -> Self {
        Self(mode)
    }
}

impl FigureComponentUpdate for SetScalablePolygonScaleMode {
    type Figure = ScalablePolygonFigure;
    type Prepared = PolygonScaleMode;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.scale_mode();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(
            PreparedFigureUpdate::geometry_and_paint(self.0).with_property_change(
                property::POLYGON_SCALE_MODE,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.replace_scale_mode(prepared);
    }
}

pub struct SetScalablePolygonAlignment {
    horizontal: Alignment,
    vertical: Alignment,
}

impl SetScalablePolygonAlignment {
    pub const fn new(horizontal: Alignment, vertical: Alignment) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }
}

impl FigureComponentUpdate for SetScalablePolygonAlignment {
    type Figure = ScalablePolygonFigure;
    type Prepared = (Alignment, Alignment);
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.alignment();
        let new = (self.horizontal, self.vertical);
        if old == new {
            return Ok(PreparedFigureUpdate::unchanged(new));
        }
        Ok(
            PreparedFigureUpdate::geometry_and_paint(new).with_property_change(
                property::POLYGON_ALIGNMENT,
                old,
                new,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.replace_alignment(prepared.0, prepared.1);
    }
}

pub struct ReplaceTextFlowPage(FlowPage);

impl ReplaceTextFlowPage {
    pub fn new(page: FlowPage) -> Self {
        Self(page)
    }
}

impl FigureComponentUpdate for ReplaceTextFlowPage {
    type Figure = TextFlowFigure;
    type Prepared = FlowPage;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.page().clone();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(
            PreparedFigureUpdate::layout_geometry_and_paint(self.0.clone()).with_property_change(
                property::FLOW_PAGE,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.replace_page(prepared);
    }
}

pub struct SetTextFlowWrapping(FlowWrapping);

impl SetTextFlowWrapping {
    pub const fn new(wrapping: FlowWrapping) -> Self {
        Self(wrapping)
    }
}

impl FigureComponentUpdate for SetTextFlowWrapping {
    type Figure = TextFlowFigure;
    type Prepared = FlowWrapping;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.wrapping();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(
            PreparedFigureUpdate::layout_geometry_and_paint(self.0).with_property_change(
                property::FLOW_WRAPPING,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.replace_wrapping(prepared);
    }
}

pub struct SetRoundedRectangleCorners(Dimension);

impl SetRoundedRectangleCorners {
    pub const fn new(dimensions: Dimension) -> Self {
        Self(dimensions)
    }
}

impl FigureComponentUpdate for SetRoundedRectangleCorners {
    type Figure = RoundedRectangleFigure;
    type Prepared = Dimension;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if !self.0.width.is_finite() || !self.0.height.is_finite() {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        if self.0.width < 0.0 || self.0.height < 0.0 {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let old = current.corner_dimensions();
        if old == self.0 && current.corners().is_none() {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(PreparedFigureUpdate::layout_geometry_and_paint(self.0)
            .with_property_change(property::CORNER_DIMENSIONS, old, self.0)
            .with_property_change(property::RECTANGLE_CORNERS, current.corners(), None))
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.set_corner_dimensions(prepared);
    }
}

pub struct SetTriangleDirection(Direction);

impl SetTriangleDirection {
    pub const fn new(direction: Direction) -> Self {
        Self(direction)
    }
}

impl FigureComponentUpdate for SetTriangleDirection {
    type Figure = TriangleFigure;
    type Prepared = Direction;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.direction;
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(self.0));
        }
        Ok(
            PreparedFigureUpdate::layout_geometry_and_paint(self.0).with_property_change(
                property::DIRECTION,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.set_direction(prepared);
    }
}

struct RefreshBorderState;

impl FigureUpdateEffect for RefreshBorderState {
    fn prepare(
        self: Box<Self>,
        context: &mut FigureUpdateEffectContext<'_>,
    ) -> Result<(), RuntimeMutationError> {
        context.refresh_border_state();
        Ok(())
    }
}

pub struct ReplaceFigureBorder(Option<Arc<dyn Border>>);

impl ReplaceFigureBorder {
    pub fn new(border: Option<Arc<dyn Border>>) -> Self {
        Self(border)
    }

    pub fn set(border: impl Border + 'static) -> Self {
        Self(Some(Arc::new(border)))
    }

    pub const fn clear() -> Self {
        Self(None)
    }
}

impl FigureCapabilityUpdate for ReplaceFigureBorder {
    type Capability = BorderCapability;
    type Prepared = Option<Arc<dyn Border>>;
    type Error = ShapeMutationError;

    const KEY: crate::figure::CapabilityKey<Self::Capability> = BORDER;

    fn prepare(
        self,
        capability: &Self::Capability,
        target: &dyn Figure,
        _context: FigureCapabilityContext,
    ) -> Result<PreparedCapabilityUpdate<Self::Prepared>, Self::Error> {
        if self.0.as_deref().is_some_and(|border| {
            let insets = border.get_insets();
            let preferred = border.preferred_size();
            [
                insets.top,
                insets.left,
                insets.bottom,
                insets.right,
                preferred.width,
                preferred.height,
            ]
            .into_iter()
            .any(|value| !value.is_finite() || value < 0.0)
        }) {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let current = capability
            .resolve(target)
            .expect("registered Border capability must resolve");
        let unchanged = match (current.border(), self.0.as_ref()) {
            (Some(old), Some(new)) => Arc::ptr_eq(old, new),
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return Ok(PreparedCapabilityUpdate::unchanged(self.0));
        }
        let had_border = current.border().is_some();
        let has_border = self.0.is_some();
        Ok(PreparedCapabilityUpdate::layout_geometry_and_paint(self.0)
            .with_property_change(property::BORDER, had_border, has_border)
            .with_effect(RefreshBorderState))
    }

    fn commit(capability: &Self::Capability, prepared: Self::Prepared, target: &mut dyn Figure) {
        capability
            .resolve_mut(target)
            .expect("registered Border capability must resolve")
            .replace_border(prepared);
    }
}

pub struct SetClickableSelected(bool);

impl SetClickableSelected {
    pub const fn new(selected: bool) -> Self {
        Self(selected)
    }
}

impl FigureCapabilityUpdate for SetClickableSelected {
    type Capability = ClickableCapability;
    type Prepared = bool;
    type Error = WidgetError;

    const KEY: crate::figure::CapabilityKey<Self::Capability> = CLICKABLE;

    fn prepare(
        self,
        capability: &Self::Capability,
        target: &dyn Figure,
        _context: FigureCapabilityContext,
    ) -> Result<PreparedCapabilityUpdate<Self::Prepared>, Self::Error> {
        let old = capability
            .resolve(target)
            .expect("registered Clickable capability must resolve")
            .clickable_model()
            .is_selected();
        if old == self.0 {
            return Ok(PreparedCapabilityUpdate::unchanged(self.0));
        }
        Ok(
            PreparedCapabilityUpdate::paint(self.0).with_property_change(
                property::SELECTED,
                old,
                self.0,
            ),
        )
    }

    fn commit(capability: &Self::Capability, prepared: Self::Prepared, target: &mut dyn Figure) {
        let model = capability
            .resolve_mut(target)
            .expect("registered Clickable capability must resolve")
            .clickable_model_mut();
        crate::figure::widget::set_selected(model, prepared);
    }
}

pub struct SetClickableRolloverEnabled(bool);

impl SetClickableRolloverEnabled {
    pub const fn new(enabled: bool) -> Self {
        Self(enabled)
    }
}

impl FigureCapabilityUpdate for SetClickableRolloverEnabled {
    type Capability = ClickableCapability;
    type Prepared = bool;
    type Error = WidgetError;

    const KEY: crate::figure::CapabilityKey<Self::Capability> = CLICKABLE;

    fn prepare(
        self,
        capability: &Self::Capability,
        target: &dyn Figure,
        _context: FigureCapabilityContext,
    ) -> Result<PreparedCapabilityUpdate<Self::Prepared>, Self::Error> {
        let old = capability
            .resolve(target)
            .expect("registered Clickable capability must resolve")
            .clickable_model()
            .rollover_enabled();
        if old == self.0 {
            return Ok(PreparedCapabilityUpdate::unchanged(self.0));
        }
        Ok(
            PreparedCapabilityUpdate::paint(self.0).with_property_change(
                property::ROLLOVER_ENABLED,
                old,
                self.0,
            ),
        )
    }

    fn commit(capability: &Self::Capability, prepared: Self::Prepared, target: &mut dyn Figure) {
        let model = capability
            .resolve_mut(target)
            .expect("registered Clickable capability must resolve")
            .clickable_model_mut();
        crate::figure::widget::set_rollover_enabled(model, prepared);
    }
}

struct CommitNodeBounds(Rectangle);

impl FigureUpdateEffect for CommitNodeBounds {
    fn prepare(
        self: Box<Self>,
        context: &mut FigureUpdateEffectContext<'_>,
    ) -> Result<(), RuntimeMutationError> {
        context.set_bounds(self.0)
    }
}

#[doc(hidden)]
pub struct PreparedPolylineGeometry {
    bounds: Rectangle,
    local_points: Vec<Point>,
    stroke: Option<StrokeStyle>,
}

fn prepare_polyline_points(
    current: &PolylineFigure,
    points: Vec<Point>,
) -> Result<PreparedFigureUpdate<Option<PreparedPolylineGeometry>>, ShapeMutationError> {
    if points
        .iter()
        .any(|point| !point.x().is_finite() || !point.y().is_finite())
    {
        return Err(ShapeMutationError::NonFiniteGeometry);
    }
    let old = current.parent_points();
    if old == points {
        return Ok(PreparedFigureUpdate::unchanged(None));
    }
    let (bounds, local_points) =
        crate::figure::normalize_points(points.clone(), current.stroke_style(), 2);
    Ok(
        PreparedFigureUpdate::layout_geometry_and_paint(Some(PreparedPolylineGeometry {
            bounds,
            local_points,
            stroke: None,
        }))
        .with_property_change(property::POINTS, old, points)
        .with_effect(CommitNodeBounds(bounds)),
    )
}

fn prepare_polyline_stroke(
    current: &PolylineFigure,
    stroke: StrokeStyle,
) -> Result<PreparedFigureUpdate<Option<PreparedPolylineGeometry>>, ShapeMutationError> {
    let old = current.stroke_style().clone();
    if old == stroke {
        return Ok(PreparedFigureUpdate::unchanged(None));
    }
    let points = current.parent_points();
    let (bounds, local_points) = crate::figure::normalize_points(points, &stroke, 2);
    let mut prepared =
        PreparedFigureUpdate::layout_geometry_and_paint(Some(PreparedPolylineGeometry {
            bounds,
            local_points,
            stroke: Some(stroke.clone()),
        }))
        .with_property_change(property::STROKE_STYLE, old.clone(), stroke.clone())
        .with_effect(CommitNodeBounds(bounds));
    if old.width() != stroke.width() {
        prepared =
            prepared.with_property_change(property::STROKE_WIDTH, old.width(), stroke.width());
    }
    if old.join() != stroke.join() {
        prepared = prepared.with_property_change(property::LINE_JOIN, old.join(), stroke.join());
    }
    Ok(prepared)
}

fn commit_polyline(prepared: Option<PreparedPolylineGeometry>, target: &mut PolylineFigure) {
    let Some(prepared) = prepared else {
        return;
    };
    if let Some(stroke) = prepared.stroke {
        target.commit_stroke_style(stroke);
    }
    target.commit_geometry(prepared.bounds, prepared.local_points);
}

pub struct ReplacePolylinePoints(Vec<Point>);

impl ReplacePolylinePoints {
    pub fn new(points: Vec<Point>) -> Self {
        Self(points)
    }
}

impl FigureComponentUpdate for ReplacePolylinePoints {
    type Figure = PolylineFigure;
    type Prepared = Option<PreparedPolylineGeometry>;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        prepare_polyline_points(current, self.0)
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        commit_polyline(prepared, target);
    }
}

pub struct InsertPolylinePoint {
    index: usize,
    point: Point,
}

impl InsertPolylinePoint {
    pub const fn new(index: usize, point: Point) -> Self {
        Self { index, point }
    }
}

impl FigureComponentUpdate for InsertPolylinePoint {
    type Figure = PolylineFigure;
    type Prepared = Option<PreparedPolylineGeometry>;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let mut points = current.parent_points();
        if self.index > points.len() {
            return Err(ShapeMutationError::PointIndexOutOfRange {
                index: self.index,
                len: points.len(),
            });
        }
        points.insert(self.index, self.point);
        prepare_polyline_points(current, points)
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        commit_polyline(prepared, target);
    }
}

pub struct SetPolylinePoint {
    index: usize,
    point: Point,
}

impl SetPolylinePoint {
    pub const fn new(index: usize, point: Point) -> Self {
        Self { index, point }
    }
}

impl FigureComponentUpdate for SetPolylinePoint {
    type Figure = PolylineFigure;
    type Prepared = Option<PreparedPolylineGeometry>;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let mut points = current.parent_points();
        let len = points.len();
        let Some(point) = points.get_mut(self.index) else {
            return Err(ShapeMutationError::PointIndexOutOfRange {
                index: self.index,
                len,
            });
        };
        *point = self.point;
        prepare_polyline_points(current, points)
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        commit_polyline(prepared, target);
    }
}

pub struct RemovePolylinePoint(usize);

impl RemovePolylinePoint {
    pub const fn new(index: usize) -> Self {
        Self(index)
    }
}

impl FigureComponentUpdate for RemovePolylinePoint {
    type Figure = PolylineFigure;
    type Prepared = Option<PreparedPolylineGeometry>;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let mut points = current.parent_points();
        if self.0 >= points.len() {
            return Err(ShapeMutationError::PointIndexOutOfRange {
                index: self.0,
                len: points.len(),
            });
        }
        points.remove(self.0);
        prepare_polyline_points(current, points)
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        commit_polyline(prepared, target);
    }
}

pub struct ClearPolylinePoints;

impl FigureComponentUpdate for ClearPolylinePoints {
    type Figure = PolylineFigure;
    type Prepared = Option<PreparedPolylineGeometry>;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        prepare_polyline_points(current, Vec::new())
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        commit_polyline(prepared, target);
    }
}

pub struct SetPolylineStrokeStyle(StrokeStyle);

impl SetPolylineStrokeStyle {
    pub fn new(stroke: StrokeStyle) -> Self {
        Self(stroke)
    }
}

impl FigureComponentUpdate for SetPolylineStrokeStyle {
    type Figure = PolylineFigure;
    type Prepared = Option<PreparedPolylineGeometry>;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        prepare_polyline_stroke(current, self.0)
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        commit_polyline(prepared, target);
    }
}

macro_rules! polyline_stroke_update {
    ($name:ident, $value:ty, $apply:expr) => {
        pub struct $name($value);

        impl $name {
            pub fn new(value: $value) -> Self {
                Self(value)
            }
        }

        impl FigureComponentUpdate for $name {
            type Figure = PolylineFigure;
            type Prepared = Option<PreparedPolylineGeometry>;
            type Error = ShapeMutationError;

            fn prepare(
                self,
                current: &Self::Figure,
                _context: FigureComponentContext,
            ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
                let stroke = ($apply)(current.stroke_style().clone(), self.0)?;
                prepare_polyline_stroke(current, stroke)
            }

            fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
                commit_polyline(prepared, target);
            }
        }
    };
}

pub struct SetPolylineStrokeWidth(f64);

impl SetPolylineStrokeWidth {
    pub const fn new(width: f64) -> Self {
        Self(width)
    }
}

impl FigureComponentUpdate for SetPolylineStrokeWidth {
    type Figure = PolylineFigure;
    type Prepared = Option<PreparedPolylineGeometry>;
    type Error = ShapeMutationError;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if !self.0.is_finite() {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        if self.0 < 0.0 {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let stroke = current
            .stroke_style()
            .clone()
            .with_width(self.0)
            .map_err(ShapeMutationError::InvalidStroke)?;
        prepare_polyline_stroke(current, stroke)
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        commit_polyline(prepared, target);
    }
}

polyline_stroke_update!(SetPolylineMiterLimit, f64, |stroke: StrokeStyle, value| {
    stroke
        .with_miter_limit(value)
        .map_err(ShapeMutationError::InvalidStroke)
});
polyline_stroke_update!(SetPolylineLineCap, LineCap, |stroke: StrokeStyle, value| {
    Ok(stroke.with_cap(value))
});
polyline_stroke_update!(
    SetPolylineDashPattern,
    DashPattern,
    |stroke: StrokeStyle, value| { Ok(stroke.with_dash_pattern(value)) }
);
polyline_stroke_update!(SetPolylineDashOffset, f64, |stroke: StrokeStyle, value| {
    stroke
        .with_dash_offset(value)
        .map_err(ShapeMutationError::InvalidStroke)
});
polyline_stroke_update!(
    SetPolylineLineJoin,
    LineJoin,
    |stroke: StrokeStyle, value| { Ok(stroke.with_join(value)) }
);
