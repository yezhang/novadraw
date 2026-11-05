use novadraw::{
    event::{EventContext, FocusEvent, MouseButton, MouseEvent, WheelEvent},
    figure::{
        CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, FigureContainer, FigureEventHandler, HitParticipation,
        INPUT, InputCapability,
    },
    geometry::Rectangle,
    graphics::{GraphicsError, PaintContext, StrokeStyle},
    runtime::{FigureComponentContext, FigureComponentUpdate, PreparedFigureUpdate},
    tree::{FigureId, FigureTree},
};
use std::convert::Infallible;

const SELECTION_WIDTH: f64 = 2.0;

/// Layout groups do not intercept clicks on empty canvas space.
pub struct Group(pub Rectangle);
impl Figure for Group {
    fn name(&self) -> &'static str {
        "SampleGroup"
    }
    fn initial_bounds(&self) -> Rectangle {
        self.0
    }
    fn hit_participation(&self) -> HitParticipation {
        HitParticipation::DescendantsOnly
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}
impl FigureContainer for Group {}

pub struct Canvas(pub Rectangle);
impl Figure for Canvas {
    fn name(&self) -> &'static str {
        "SampleCanvas"
    }
    fn initial_bounds(&self) -> Rectangle {
        self.0
    }
    fn paint(&self, ctx: &mut PaintContext<'_>, bounds: Rectangle) -> Result<(), GraphicsError> {
        ctx.set_fill_paint(super::color("#f0f4f7"));
        ctx.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(INPUT, InputCapability::of::<Self>())
    }
}
impl FigureContainer for Canvas {}
impl FigureEventHandler for Canvas {
    fn on_mouse_pressed(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        event.button == MouseButton::Left
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Idle,
    Selected,
    Editing,
}

/// The Core focus owner determines exclusive selection. Editing opens this
/// surface to the component's existing input subtree, while retaining focus.
pub struct SelectionSurface {
    bounds: Rectangle,
    editable: bool,
    mode: Mode,
}
impl Figure for SelectionSurface {
    fn name(&self) -> &'static str {
        "SampleSelectionSurface"
    }
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        self.mode != Mode::Editing
            && x >= 0.0
            && y >= 0.0
            && x <= bounds.width
            && y <= bounds.height
    }
    fn paint(&self, ctx: &mut PaintContext<'_>, bounds: Rectangle) -> Result<(), GraphicsError> {
        if self.mode == Mode::Idle {
            return Ok(());
        }
        let hex = if self.mode == Mode::Editing {
            "#176c68"
        } else {
            "#2563eb"
        };
        ctx.set_stroke_paint(super::color(hex));
        ctx.set_stroke_style(
            StrokeStyle::default()
                .with_width(SELECTION_WIDTH)
                .expect("selection width"),
        );
        let inset = SELECTION_WIDTH / 2.0;
        ctx.stroke_rect(Rectangle::new(
            inset,
            inset,
            (bounds.width - SELECTION_WIDTH).max(0.0),
            (bounds.height - SELECTION_WIDTH).max(0.0),
        ))
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())
    }
}
impl FigureEventHandler for SelectionSurface {
    fn on_mouse_pressed(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        event.button == MouseButton::Left
    }
    fn on_mouse_double_clicked(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        if !self.editable || event.button != MouseButton::Left {
            return false;
        }
        ctx.effects().update_component_later(SetMode(Mode::Editing));
        true
    }
    fn on_mouse_wheel(&self, _: &WheelEvent, _: &mut EventContext<'_>) -> bool {
        true
    }
    fn on_focus_gained(&self, _: &FocusEvent, ctx: &mut EventContext<'_>) -> bool {
        if self.mode != Mode::Editing {
            ctx.effects()
                .update_component_later(SetMode(Mode::Selected));
        }
        true
    }
    fn on_focus_lost(&self, _: &FocusEvent, ctx: &mut EventContext<'_>) -> bool {
        ctx.effects().update_component_later(SetMode(Mode::Idle));
        true
    }
}
struct SetMode(Mode);
impl FigureComponentUpdate for SetMode {
    type Figure = SelectionSurface;
    type Prepared = Mode;
    type Error = Infallible;
    fn prepare(
        self,
        current: &Self::Figure,
        _: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Mode>, Infallible> {
        Ok(if current.mode == self.0 {
            PreparedFigureUpdate::unchanged(self.0)
        } else {
            PreparedFigureUpdate::geometry_and_paint(self.0)
        })
    }
    fn commit(mode: Mode, target: &mut Self::Figure) {
        target.mode = mode;
    }
}

pub fn selection(
    tree: &mut FigureTree,
    parent: FigureId,
    bounds: Rectangle,
    editable: bool,
) -> FigureId {
    let id = tree
        .builder()
        .add_child(
            parent,
            Box::new(SelectionSurface {
                bounds,
                editable,
                mode: Mode::Idle,
            }),
        )
        .unwrap();
    tree.builder().set_focusable(id, true).unwrap();
    id
}
