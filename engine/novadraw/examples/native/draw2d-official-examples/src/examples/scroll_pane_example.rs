//! Source port of ScrollPaneExample, InternalFrame and ExampleUtil (EPL-2.0).
use std::sync::{Arc, Mutex};

use novadraw::{
    container::ScrollPaneHandle,
    event::{EventContext, FigureEventHandler, MouseEvent},
    figure::{
        BORDER, Border, BorderCapability, BorderedFigure, CONTAINER, ContainerCapability,
        CursorIcon, Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError,
        FigureContainer, FigureStyle, INPUT, InputCapability, LabelFigure, RectangleFigure,
        border::{BevelBorder, BevelStyle, CompoundBorder, EtchedBorder, LineBorder},
    },
    geometry::{Dimension, Insets, Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    layout::{FlowLayout, StackLayout, ToolbarLayout},
    render::BuiltinFont,
    runtime::{ImageId, Runtime, mutation::ReplaceFigureBorder},
    text::{FontDescriptor, TextConstraints, TextLayout},
    tree::{FigureId, FigureTree},
};

const FRAME_BOUNDS: Rectangle = Rectangle {
    x: 20.0,
    y: 20.0,
    width: 200.0,
    height: 140.0,
};
const MIN_WIDTH: f64 = 100.0;
const MIN_HEIGHT: f64 = 22.0;
const GRIP: f64 = 4.0;
const RING_WIDTH: f64 = 3.0;
const BUTTON: Color = Color::rgba(238.0 / 255.0, 238.0 / 255.0, 238.0 / 255.0, 1.0);
const SHADOW: Color = Color::rgba(0.5, 0.5, 0.5, 1.0);
const NORTH: u8 = 1;
const SOUTH: u8 = 2;
const WEST: u8 = 4;
const EAST: u8 = 8;

struct TitleBorder(TextLayout);

impl Border for TitleBorder {
    fn get_insets(&self) -> Insets {
        // TitleBarBorder's default asymmetric padding is (1, 3, 2, 2).
        Insets::new(f64::from(self.0.height()) + 3.0, 0.0, 0.0, 0.0)
    }
    fn paint(&self, bounds: Rectangle, g: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        let height = self.get_insets().top.min(bounds.height);
        g.set_fill_paint(SHADOW);
        g.fill_rect(Rectangle::new(bounds.x, bounds.y, bounds.width, height))?;
        g.set_fill_paint(Color::WHITE);
        g.fill_text(&self.0, Point::new(bounds.x + 3.0, bounds.y + 1.0))
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct GroupBorder(TextLayout);

impl Border for GroupBorder {
    fn get_insets(&self) -> Insets {
        Insets::uniform(f64::from(self.0.height()))
    }
    fn preferred_size(&self) -> Dimension {
        Dimension::new(
            f64::from(self.0.width() + 2.0 * self.0.height()),
            f64::from(self.0.height()),
        )
    }
    fn paint(&self, bounds: Rectangle, g: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        let height = f64::from(self.0.height());
        EtchedBorder::new(Color::WHITE, SHADOW)
            .paint(bounds.inflate(-height / 2.0, -height / 2.0), g)?;
        g.set_fill_paint(BUTTON);
        g.fill_rect(Rectangle::new(
            bounds.x + height,
            bounds.y,
            f64::from(self.0.width()),
            height,
        ))?;
        g.set_fill_paint(Color::BLACK);
        g.fill_text(&self.0, Point::new(bounds.x + height, bounds.y))
    }
}

#[derive(Clone, Copy, Default)]
struct Tracker {
    position: u8,
    moving: bool,
    start: Option<(Point, Rectangle)>,
}

struct InternalFrame {
    border: Option<Arc<dyn Border>>,
    tracker: Mutex<Tracker>,
}

impl InternalFrame {
    fn border_for(title: TextLayout) -> CompoundBorder {
        let raised = BevelBorder::new(BevelStyle::Raised, Color::WHITE, Color::BLACK, 1)
            .expect("valid bevel");
        let lowered = BevelBorder::new(BevelStyle::Lowered, Color::WHITE, Color::BLACK, 1)
            .expect("valid bevel");
        let mixed = Color::rgba(
            (238.0 / 255.0 + 0.5) / 2.0,
            (238.0 / 255.0 + 0.5) / 2.0,
            (238.0 / 255.0 + 0.5) / 2.0,
            1.0,
        );
        CompoundBorder::new(
            CompoundBorder::new(raised, TitleBorder(title)).expect("valid title border"),
            CompoundBorder::new(
                LineBorder::new(mixed, RING_WIDTH).expect("valid ring"),
                lowered,
            )
            .expect("valid border"),
        )
        .expect("valid frame border")
    }

    fn new() -> Self {
        Self {
            border: None,
            tracker: Mutex::new(Tracker::default()),
        }
    }
}

impl Figure for InternalFrame {
    fn initial_bounds(&self) -> Rectangle {
        FRAME_BOUNDS
    }
    fn name(&self) -> &'static str {
        "InternalFrame"
    }
    fn initial_insets(&self) -> Insets {
        self.border
            .as_deref()
            .map(Border::get_insets)
            .unwrap_or_default()
    }
    fn initial_focusable(&self) -> bool {
        true
    }
    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            background: Some(BUTTON),
            ..FigureStyle::default()
        }
    }
    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }
    fn paint(&self, g: &mut PaintContext<'_>, bounds: Rectangle) -> Result<(), GraphicsError> {
        g.set_fill_paint(BUTTON);
        g.fill_rect(Rectangle::new(0.0, 0.0, bounds.width, bounds.height))
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(BORDER, BorderCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())
    }
}

impl FigureContainer for InternalFrame {}

impl BorderedFigure for InternalFrame {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }
    fn replace_border(
        &mut self,
        border: novadraw::figure::border::BorderCandidate,
    ) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border.into_border())
    }
}

impl FigureEventHandler for InternalFrame {
    fn on_mouse_moved(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        let bounds = ctx.target_bounds();
        let insets = self.initial_insets();
        let client = Rectangle::new(
            insets.left + GRIP,
            insets.top + GRIP,
            (bounds.width - insets.width() - 2.0 * GRIP).max(0.0),
            (bounds.height - insets.height() - 2.0 * GRIP).max(0.0),
        );
        let mut position = 0;
        if event.x < client.x {
            position |= WEST;
        } else if event.x >= client.x + client.width {
            position |= EAST;
        }
        if event.y < client.y {
            position |= NORTH;
        } else if event.y >= client.y + client.height {
            position |= SOUTH;
        }
        let mut tracker = self.tracker.lock().expect("frame tracker");
        tracker.position = position;
        tracker.moving = position == NORTH && event.y > insets.left;
        let cursor = if tracker.moving {
            None
        } else {
            match position {
                EAST | WEST => Some(CursorIcon::EastWestResize),
                NORTH | SOUTH => Some(CursorIcon::NorthSouthResize),
                value if value == (NORTH | EAST) || value == (SOUTH | WEST) => {
                    Some(CursorIcon::NorthEastSouthWestResize)
                }
                value if value == (NORTH | WEST) || value == (SOUTH | EAST) => {
                    Some(CursorIcon::NorthWestSouthEastResize)
                }
                _ => None,
            }
        };
        let target = ctx.target_id();
        let mut style = ctx.scene().figure_style(target).expect("frame style");
        style.cursor = cursor;
        ctx.effects().set_style_later(target, style);
        false
    }
    fn on_mouse_exited(&self, _: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        let target = ctx.target_id();
        let mut style = ctx.scene().figure_style(target).expect("frame style");
        style.cursor = None;
        ctx.effects().set_style_later(target, style);
        false
    }
    fn on_mouse_pressed(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        let bounds = ctx
            .scene()
            .figure_bounds(ctx.target_id())
            .expect("frame bounds");
        self.tracker.lock().expect("frame tracker").start = Some((event.entry_point(), bounds));
        true
    }
    fn on_mouse_dragged(&self, event: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        let tracker = *self.tracker.lock().expect("frame tracker");
        let Some((start, bounds)) = tracker.start else {
            return false;
        };
        if tracker.position == 0 {
            return false;
        }
        let diff = event.entry_point() - start;
        let result = drag_bounds(bounds, tracker.position, tracker.moving, diff.x(), diff.y());
        let target = ctx.target_id();
        ctx.effects().set_bounds_later(target, result);
        ctx.effects().invalidate();
        true
    }
    fn on_mouse_released(&self, _: &MouseEvent, ctx: &mut EventContext<'_>) -> bool {
        *self.tracker.lock().expect("frame tracker") = Tracker::default();
        ctx.effects().invalidate();
        true
    }
}

fn drag_bounds(mut rect: Rectangle, position: u8, moving: bool, x: f64, y: f64) -> Rectangle {
    let (mut dx, mut dy, mut dw, mut dh) = (0.0, 0.0, 0.0, 0.0);
    if position & NORTH != 0 {
        dy = y;
        if moving {
            dx = x;
        } else {
            dh = -y;
        }
    }
    if position & WEST != 0 {
        dx = x.min(rect.width - MIN_WIDTH);
        dw = -x;
    }
    if position & EAST != 0 {
        dw = x;
    }
    if position & SOUTH != 0 {
        dh = y;
    }
    rect.x += dx;
    rect.y += dy;
    rect.width = (rect.width + dw).max(MIN_WIDTH);
    rect.height = (rect.height + dh).max(MIN_HEIGHT);
    rect
}

#[cfg_attr(not(test), allow(dead_code))]
struct Fixture {
    runtime: Runtime,
    frames: [FigureId; 2],
    panes: [ScrollPaneHandle; 2],
    toolbar: FigureId,
}

pub fn scroll_pane_example() -> Runtime {
    fixture().runtime
}

fn text(runtime: &mut Runtime, value: &str) -> TextLayout {
    runtime
        .text()
        .layout(
            value,
            &FontDescriptor::parse("12px Inter Variable").expect("font"),
            TextConstraints::UNBOUNDED,
        )
        .expect("border title")
}

fn fixture() -> Fixture {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, 800.0, 600.0, BUTTON)
                .expect("valid figure bounds"),
        ))
        .expect("main panel");
    let mut frames = Vec::new();
    let mut panes = Vec::new();
    for _ in 0..2 {
        let frame = tree
            .builder()
            .add_child(root, Box::new(InternalFrame::new()))
            .expect("frame");
        tree.builder()
            .set_layout_manager(frame, Box::new(StackLayout::new()))
            .expect("StackLayout");
        panes.push(
            tree.builder()
                .add_scroll_pane_to(frame, Rectangle::ZERO)
                .expect("ScrollPane"),
        );
        frames.push(frame);
    }
    let mut runtime = Runtime::new(tree);
    for font in BuiltinFont::ALL {
        runtime
            .resources()
            .register_builtin_font(font)
            .expect("font");
    }
    for (frame, title) in frames.iter().zip(["Flow layouts", "fixed 100x100"]) {
        let border = InternalFrame::border_for(text(&mut runtime, title));
        runtime
            .figure(*frame)
            .expect("frame")
            .update_capability(ReplaceFigureBorder::set(border))
            .expect("title border");
    }
    let image = register_george(&mut runtime);
    for pane in &panes {
        let border = GroupBorder(text(&mut runtime, "scrollpane"));
        runtime
            .figure(pane.pane_id())
            .expect("pane")
            .update_capability(ReplaceFigureBorder::set(border))
            .expect("GroupBoxBorder");
        runtime
            .viewports()
            .set_tracks_width(pane.viewport().figure_id(), true)
            .expect("tracks width");
    }
    let toolbar_border = GroupBorder(text(&mut runtime, "Toolbar"));
    let toolbar = runtime
        .viewports()
        .set_scroll_contents(
            &panes[0],
            Box::new(
                RectangleFigure::new_with_color(0.0, 0.0, 0.0, 0.0, BUTTON)
                    .expect("valid figure bounds")
                    .with_border(toolbar_border)
                    .expect("valid detached figure input"),
            ),
        )
        .expect("toolbar");
    runtime
        .figure(toolbar)
        .expect("toolbar")
        .set_layout_manager(Box::new(ToolbarLayout::new()))
        .expect("ToolbarLayout");
    for _ in 0..2 {
        let border = GroupBorder(text(&mut runtime, "FlowLayout"));
        let flow = runtime
            .figure(toolbar)
            .expect("toolbar")
            .add(Box::new(
                RectangleFigure::new_with_color(0.0, 0.0, 0.0, 0.0, BUTTON)
                    .expect("valid figure bounds")
                    .with_border(border)
                    .expect("valid detached figure input"),
            ))
            .expect("flow panel");
        runtime
            .figure(flow)
            .expect("flow")
            .set_layout_manager(Box::new(FlowLayout::new()))
            .expect("FlowLayout");
        for index in (0..5).chain(6..10) {
            runtime
                .figure(flow)
                .expect("flow")
                .add(Box::new(
                    LabelFigure::new(format!("Label {index}")).with_icon(image),
                ))
                .expect("George Label");
        }
    }
    let fixed = runtime
        .viewports()
        .set_scroll_contents(
            &panes[1],
            Box::new(
                RectangleFigure::new_with_color(0.0, 0.0, 0.0, 0.0, Color::TRANSPARENT)
                    .expect("valid figure bounds")
                    .with_border(LineBorder::default_border())
                    .expect("valid detached figure input"),
            ),
        )
        .expect("fixed contents");
    runtime
        .figure(fixed)
        .expect("fixed")
        .set_preferred_size(Dimension::new(100.0, 100.0))
        .expect("fixed preferred");
    Fixture {
        runtime,
        frames: frames.try_into().expect("two frames"),
        panes: panes.try_into().ok().expect("two panes"),
        toolbar,
    }
}

fn register_george(runtime: &mut Runtime) -> ImageId {
    let image = runtime.resources().register_image().expect("George");
    let request = runtime
        .resources()
        .begin_request(image.resource_id())
        .expect("request");
    runtime
        .resources()
        .complete_image_request_bytes(
            // Core decodes PNG; this is a lossless conversion of the single-frame upstream GIF.
            request,
            include_bytes!("../../assets/scrollpane/george.png"),
            1.0,
        )
        .expect("upstream George pixels");
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        event::MouseButton,
        render::{BackendCapabilities, SurfaceInfo},
    };

    #[test]
    fn official_overlap_and_label_composition_are_preserved() {
        let fixture = fixture();
        for frame in fixture.frames {
            assert_eq!(
                fixture.runtime.tree().figure_bounds(frame),
                Ok(FRAME_BOUNDS)
            );
            assert_eq!(fixture.runtime.tree().child_order(frame).unwrap().len(), 1);
        }
        for pane in fixture.panes {
            assert_eq!(
                fixture
                    .runtime
                    .tree()
                    .child_order(pane.pane_id())
                    .unwrap()
                    .len(),
                3
            );
        }
        let panels = fixture.runtime.tree().child_order(fixture.toolbar).unwrap();
        assert_eq!(panels.len(), 2);
        for panel in panels {
            let labels = fixture.runtime.tree().child_order(panel).unwrap();
            assert_eq!(labels.len(), 9);
        }
    }

    #[test]
    fn drag_formulas_preserve_all_eight_directions_and_official_minima() {
        for position in [
            NORTH,
            SOUTH,
            EAST,
            WEST,
            NORTH | EAST,
            NORTH | WEST,
            SOUTH | EAST,
            SOUTH | WEST,
        ] {
            let bounds = drag_bounds(FRAME_BOUNDS, position, false, 10.0, 15.0);
            assert_eq!(
                bounds.width,
                if position & WEST != 0 {
                    190.0
                } else if position & EAST != 0 {
                    210.0
                } else {
                    200.0
                }
            );
            assert_eq!(
                bounds.height,
                if position & NORTH != 0 {
                    125.0
                } else if position & SOUTH != 0 {
                    155.0
                } else {
                    140.0
                }
            );
        }
        assert_eq!(
            drag_bounds(FRAME_BOUNDS, NORTH, true, 30.0, 40.0),
            Rectangle::new(50.0, 60.0, 200.0, 140.0)
        );
        assert_eq!(
            drag_bounds(FRAME_BOUNDS, WEST | SOUTH, false, 500.0, -500.0),
            Rectangle::new(120.0, 20.0, 100.0, 22.0)
        );
    }

    fn prepare(runtime: &mut Runtime) {
        let preparation = runtime.frames().prepare_submission(
            SurfaceInfo {
                logical_width: 800.0,
                logical_height: 600.0,
                pixel_width: 800,
                pixel_height: 600,
                scale_factor: 1.0,
            },
            BackendCapabilities::RETAINED_PARTIAL,
        );
        assert!(
            !matches!(preparation, novadraw::runtime::FramePreparation::Error(_)),
            "{preparation:?}"
        );
        if let novadraw::runtime::FramePreparation::Ready(submission) = preparation {
            let _ = runtime
                .frames()
                .complete_submission(
                    submission.session_id,
                    submission.frame_id,
                    Ok(novadraw::render::RenderDisposition::Presented),
                )
                .unwrap();
        }
    }

    #[test]
    fn frame_cursor_follows_edges_corners_and_pointer_exit() {
        let mut fixture = fixture();
        prepare(&mut fixture.runtime);
        for (x, y, expected) in [
            (219.0, 90.0, CursorIcon::EastWestResize),
            (100.0, 159.0, CursorIcon::NorthSouthResize),
            (219.0, 159.0, CursorIcon::NorthWestSouthEastResize),
            (21.0, 159.0, CursorIcon::NorthEastSouthWestResize),
        ] {
            fixture.runtime.input().dispatch_mouse_moved(x, y);
            assert_eq!(fixture.runtime.focus().cursor_icon(), expected);
        }
        fixture.runtime.input().pointer_exited();
        assert_eq!(fixture.runtime.focus().cursor_icon(), CursorIcon::Default);
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn title_drag_moves_only_the_top_frame_and_keeps_paint_order() {
        let mut fixture = fixture();
        prepare(&mut fixture.runtime);
        fixture.runtime.input().dispatch_mouse_moved(80.0, 30.0);
        fixture
            .runtime
            .input()
            .dispatch_mouse_pressed(80.0, 30.0, MouseButton::Left);
        fixture.runtime.input().dispatch_mouse_moved(120.0, 60.0);
        fixture
            .runtime
            .input()
            .dispatch_mouse_released(120.0, 60.0, MouseButton::Left);
        assert_eq!(
            fixture.runtime.tree().figure_bounds(fixture.frames[0]),
            Ok(FRAME_BOUNDS)
        );
        assert_eq!(
            fixture.runtime.tree().figure_bounds(fixture.frames[1]),
            Ok(Rectangle::new(60.0, 50.0, 200.0, 140.0))
        );
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn resizing_revalidates_fixed_contents_and_automatic_scrollbars() {
        let mut fixture = fixture();
        prepare(&mut fixture.runtime);
        let pane = &fixture.panes[1];
        assert!(pane.viewport().contents_tracks_width());
        assert!(
            fixture
                .runtime
                .tree()
                .is_visible(pane.vertical_scroll_bar())
                .unwrap()
        );
        assert!(
            !fixture
                .runtime
                .tree()
                .is_visible(pane.horizontal_scroll_bar())
                .unwrap()
        );
        // East border is outside the nested ScrollPane hit area.
        fixture.runtime.input().dispatch_mouse_moved(218.0, 90.0);
        fixture
            .runtime
            .input()
            .dispatch_mouse_pressed(218.0, 90.0, MouseButton::Left);
        fixture.runtime.input().dispatch_mouse_moved(318.0, 90.0);
        fixture
            .runtime
            .input()
            .dispatch_mouse_released(318.0, 90.0, MouseButton::Left);
        assert_eq!(
            fixture.runtime.tree().figure_bounds(fixture.frames[1]),
            Ok(Rectangle::new(20.0, 20.0, 300.0, 140.0))
        );
        // Shrinking below content height needs scrolling; growing removes it.
        fixture
            .runtime
            .figure(fixture.frames[1])
            .unwrap()
            .set_bounds(Rectangle::new(20.0, 20.0, 300.0, 100.0))
            .unwrap();
        fixture
            .runtime
            .figure(fixture.frames[1])
            .unwrap()
            .revalidate()
            .unwrap();
        prepare(&mut fixture.runtime);
        assert!(
            fixture
                .runtime
                .tree()
                .is_visible(fixture.panes[1].vertical_scroll_bar())
                .unwrap()
        );
        fixture
            .runtime
            .figure(fixture.frames[1])
            .unwrap()
            .set_bounds(Rectangle::new(20.0, 20.0, 300.0, 240.0))
            .unwrap();
        fixture
            .runtime
            .figure(fixture.frames[1])
            .unwrap()
            .revalidate()
            .unwrap();
        prepare(&mut fixture.runtime);
        assert!(
            !fixture
                .runtime
                .tree()
                .is_visible(fixture.panes[1].vertical_scroll_bar())
                .unwrap()
        );
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }
}
