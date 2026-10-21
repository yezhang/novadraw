use super::text_examples::{TextFixture, add, canvas, font, populate_large};
use novadraw::{
    event::{EventContext, FigureEventHandler, FocusEvent, Key, KeyEvent, MouseEvent},
    figure::{
        ACCESSIBILITY, AccessibilityCapability, AccessibleFigure, Border, CONTAINER,
        ContainerCapability, Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError,
        FigureContainer, FigureDrawing, FigurePreparation, FigurePresentation, FigureStyle, INPUT,
        InputCapability, PREPARATION, PreparationCapability, PreparationScene, PreparedScene,
        border::LineBorder,
        flow::{
            BLOCK_CONTRIBUTION, BlockContribution, FLOW, FlowBlockStyle, FlowCapability,
            FlowContributionContext, FlowContributionSink, FlowContributor, FlowError,
            FlowLayoutService, FlowSnapshot, InlineFlowFigure, SetStructuredFlowText,
            SourceTextPosition, StructuredTextFigure,
        },
    },
    geometry::{Insets, Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    runtime::Runtime,
    text::{MeasureContext, ParagraphAlignment, TextAffinity, TextError},
    tree::FigureId,
};
use std::sync::Arc;

const CARET_TEXT: &str = "A TextFlow can be queried for an offset for a given Point relative to it. The returned offset will be between 0 and N, where N is the length of the figure's text.\n<Chinese>最新消息</Chinese>.\n<Japanese>日本語のページを検索</Japanese>.\n<Hebrew>דפיםֱ מֱ- ישרֱֹאןץֱֹ</Hebrew>\n<Arabic>نصً ثُ سٍَبُس</Arabic>";
const LABEL_VERTICAL_STEP: f64 = 26.0;
const LABEL_HORIZONTAL_STEP: f64 = 58.5;

#[derive(Clone, Copy)]
enum Interaction {
    Caret,
    Typing,
    Label(FigureId),
}

struct InteractivePage {
    service: FlowLayoutService,
    interaction: Interaction,
    target: Option<FigureId>,
    pointer: Option<Point>,
    focused: bool,
    border: LineBorder,
}
impl InteractivePage {
    fn new(interaction: Interaction) -> Self {
        Self {
            service: FlowLayoutService::default(),
            interaction,
            target: None,
            pointer: None,
            focused: false,
            border: LineBorder::new(Color::rgba(0.25, 0.25, 0.25, 1.0), 10.0),
        }
    }
}
impl Figure for InteractivePage {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 600.0, 500.0)
    }
    fn initial_style(&self) -> FigureStyle {
        font(if matches!(self.interaction, Interaction::Caret) {
            "26.666666px Tahoma"
        } else {
            "13px Arial"
        })
    }
    fn initial_insets(&self) -> Insets {
        match self.interaction {
            Interaction::Caret => self.border.get_insets(),
            Interaction::Label(_) => Insets::uniform(2.0),
            Interaction::Typing => Insets::ZERO,
        }
    }
    fn get_border(&self) -> Option<&dyn Border> {
        matches!(self.interaction, Interaction::Caret).then_some(&self.border as &dyn Border)
    }
    fn name(&self) -> &'static str {
        "OfficialInteractiveFlowPage"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(FLOW, FlowCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(INPUT, InputCapability::of::<Self>())?;
        out.register(ACCESSIBILITY, AccessibilityCapability::of::<Self>())?;
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigureContainer for InteractivePage {}
impl AccessibleFigure for InteractivePage {
    fn accessible_role(&self) -> novadraw::event::AccessibilityRole {
        novadraw::event::AccessibilityRole::Text
    }
    fn accessible_name(&self) -> Option<&str> {
        matches!(self.interaction, Interaction::Label(_))
            .then_some(super::text_examples::MULTI_LINE)
    }
}
impl FlowContributor for InteractivePage {
    fn contribute(
        &self,
        _: &FlowContributionContext<'_>,
        out: &mut FlowContributionSink,
    ) -> Result<(), FlowError> {
        out.append(
            BLOCK_CONTRIBUTION,
            BlockContribution(FlowBlockStyle {
                alignment: matches!(self.interaction, Interaction::Caret)
                    .then_some(ParagraphAlignment::Center),
                ..FlowBlockStyle::default()
            }),
        );
        Ok(())
    }
}
struct InteractiveDrawing {
    snapshot: Arc<FlowSnapshot>,
    pointer: Option<Point>,
    caret: bool,
    focus: Option<Rectangle>,
}
impl FigureDrawing for InteractiveDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        if let Some(bounds) = self.focus {
            context.set_stroke_paint(Color::BLACK);
            context.set_stroke_width(1.0)?;
            context.stroke_rect(bounds)?;
        }
        if self.caret {
            context.set_fill_paint(Color::rgba(
                190.0 / 255.0,
                220.0 / 255.0,
                250.0 / 255.0,
                1.0,
            ));
            context.fill_rect(Rectangle::new(
                0.0,
                0.0,
                self.snapshot.size.width,
                self.snapshot.size.height,
            ))?;
        }
        if let Some(pointer) = self.pointer
            && let Ok(position) = self.snapshot.hit_text(pointer)
        {
            let next = self
                .snapshot
                .paragraphs
                .iter()
                .flat_map(|p| p.sources.iter().map(move |s| (p, s)))
                .find(|(_, s)| s.source == position.source)
                .and_then(|(p, s)| {
                    let offset = s.range.start + position.byte_offset - s.source_offset;
                    p.layout.request().text()[offset..]
                        .chars()
                        .next()
                        .map(|ch| SourceTextPosition {
                            byte_offset: position.byte_offset + ch.len_utf8(),
                            affinity: TextAffinity::Upstream,
                            ..position
                        })
                });
            if let Some(next) = next
                && let Ok(selection) = self.snapshot.selection(position, next)
            {
                context.set_fill_paint(Color::rgba(0.3, 0.5, 0.9, 0.4));
                for quad in selection {
                    context.fill_rect(quad.bounds())?;
                }
            }
            self.snapshot.paint(context)?;
            if let Ok(caret) = self.snapshot.caret(position) {
                context.set_fill_paint(Color::BLACK);
                let b = caret.bounds();
                context.fill_rect(Rectangle::new(b.x, b.y, 1.0, b.height))?;
            }
            return Ok(());
        }
        self.snapshot.paint(context)
    }
}
impl FigurePreparation for InteractivePage {
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Err(TextError::InvalidPreparedScene("subtree required".into()))
    }
    fn prepare_scene(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        let prepared = self
            .service
            .prepare(context, bounds, scene)
            .map_err(|e| TextError::InvalidPreparedScene(e.to_string()))?;
        let original = prepared.root_presentation();
        let snapshot = Arc::new(original.data::<FlowSnapshot>().expect("flow data").clone());
        let root = FigurePresentation::new(
            original.measurement(),
            original.minimum_size(),
            original.visual_bounds(),
            Arc::new(InteractiveDrawing {
                snapshot: snapshot.clone(),
                pointer: self.pointer,
                caret: matches!(self.interaction, Interaction::Caret),
                focus: self.focused.then_some(Rectangle::new(
                    0.5,
                    0.5,
                    snapshot.size.width - 1.0,
                    snapshot.size.height - 1.0,
                )),
            }),
        )?
        .with_data(snapshot);
        Ok(prepared.with_root_presentation(root))
    }
}
struct SetPointer(Point);
impl novadraw::runtime::FigureComponentUpdate for SetPointer {
    type Figure = InteractivePage;
    type Prepared = Point;
    type Error = std::convert::Infallible;
    fn prepare(
        self,
        _: &Self::Figure,
        _: novadraw::runtime::FigureComponentContext,
    ) -> Result<novadraw::runtime::PreparedFigureUpdate<Point>, Self::Error> {
        Ok(novadraw::runtime::PreparedFigureUpdate::paint(self.0))
    }
    fn commit(point: Point, figure: &mut Self::Figure) {
        figure.pointer = Some(point);
    }
}
impl FigureEventHandler for InteractivePage {
    fn on_focus_gained(&self, _: &FocusEvent, context: &mut EventContext<'_>) -> bool {
        context.effects().update_component_later(SetFocused(true));
        false
    }
    fn on_focus_lost(&self, _: &FocusEvent, context: &mut EventContext<'_>) -> bool {
        context.effects().update_component_later(SetFocused(false));
        false
    }
    fn on_mouse_moved(&self, event: &MouseEvent, context: &mut EventContext<'_>) -> bool {
        if matches!(self.interaction, Interaction::Caret) {
            if context
                .scene()
                .prepared_data::<FlowSnapshot>(context.target_id())
                .ok()
                .flatten()
                .is_none()
            {
                return false;
            }
            context
                .effects()
                .update_component_later(SetPointer(Point::new(event.x, event.y)));
            true
        } else {
            false
        }
    }
    fn on_key_pressed(&self, event: &KeyEvent, context: &mut EventContext<'_>) -> bool {
        if let Interaction::Label(viewport) = self.interaction {
            let (dx, dy) = match event.key {
                Key::ArrowDown => (0.0, LABEL_VERTICAL_STEP),
                Key::ArrowUp => (0.0, -LABEL_VERTICAL_STEP),
                Key::ArrowRight => (LABEL_HORIZONTAL_STEP, 0.0),
                Key::ArrowLeft => (-LABEL_HORIZONTAL_STEP, 0.0),
                _ => return false,
            };
            context.effects().scroll_viewport_by_later(viewport, dx, dy);
            return true;
        }
        let Key::Character(character) = event.key else {
            return false;
        };
        if !matches!(self.interaction, Interaction::Typing)
            || !(character.is_ascii_alphabetic() || character == ' ')
        {
            return false;
        }
        let Some(target) = self.target else {
            return false;
        };
        let Some(current) = context
            .scene()
            .component::<StructuredTextFigure>(target)
            .ok()
            .flatten()
        else {
            return false;
        };
        let value = format!("{}{character}", current.text());
        context
            .effects()
            .update_figure_component_later(target, SetStructuredFlowText::new(value));
        true
    }
}
struct SetFocused(bool);
impl novadraw::runtime::FigureComponentUpdate for SetFocused {
    type Figure = InteractivePage;
    type Prepared = bool;
    type Error = std::convert::Infallible;
    fn prepare(
        self,
        _: &Self::Figure,
        _: novadraw::runtime::FigureComponentContext,
    ) -> Result<novadraw::runtime::PreparedFigureUpdate<bool>, Self::Error> {
        Ok(novadraw::runtime::PreparedFigureUpdate::paint(self.0))
    }
    fn commit(focused: bool, figure: &mut Self::Figure) {
        figure.focused = focused;
    }
}

pub(super) fn label_page(
    runtime: &mut Runtime,
    pane: &novadraw::container::ScrollPaneHandle,
) -> FigureId {
    let page = runtime
        .viewports()
        .set_scroll_contents(
            pane,
            Box::new(InteractivePage::new(Interaction::Label(
                pane.viewport().figure_id(),
            ))),
        )
        .expect("label page");
    runtime
        .figure(page)
        .expect("label")
        .set_focusable(true)
        .expect("focusable label");
    runtime
        .figure(page)
        .expect("label")
        .set_focus_traversable(true)
        .expect("traversable label");
    page
}

pub fn caret_example() -> Runtime {
    let TextFixture {
        mut runtime, pane, ..
    } = canvas();
    let page = runtime
        .viewports()
        .set_scroll_contents(&pane, Box::new(InteractivePage::new(Interaction::Caret)))
        .expect("caret page");
    let inline = add(
        &mut runtime,
        page,
        InlineFlowFigure::new().with_fragment_border(super::text_test_border::TestBorder),
    );
    add(&mut runtime, inline, StructuredTextFigure::new(CARET_TEXT));
    runtime
}
pub fn text_flow_large_example() -> Runtime {
    let TextFixture {
        mut runtime, pane, ..
    } = canvas();
    let page = runtime
        .viewports()
        .set_scroll_contents(&pane, Box::new(InteractivePage::new(Interaction::Typing)))
        .expect("typing page");
    let target = populate_large(&mut runtime, page);
    runtime
        .figure(page)
        .expect("page")
        .update_component(SetTarget(target))
        .expect("target");
    runtime
        .figure(page)
        .expect("page")
        .set_focusable(true)
        .expect("focus");
    runtime.focus().request(page).expect("initial focus");
    runtime
        .viewports()
        .set_scroll_bar_visibility(
            &pane,
            novadraw::container::ScrollBarVisibility::Automatic,
            novadraw::container::ScrollBarVisibility::Always,
        )
        .expect("always vertical");
    runtime
}
struct SetTarget(FigureId);
impl novadraw::runtime::FigureComponentUpdate for SetTarget {
    type Figure = InteractivePage;
    type Prepared = FigureId;
    type Error = std::convert::Infallible;
    fn prepare(
        self,
        _: &Self::Figure,
        _: novadraw::runtime::FigureComponentContext,
    ) -> Result<novadraw::runtime::PreparedFigureUpdate<FigureId>, Self::Error> {
        Ok(novadraw::runtime::PreparedFigureUpdate::paint(self.0))
    }
    fn commit(target: FigureId, figure: &mut Self::Figure) {
        figure.target = Some(target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::event::KeyModifiers;

    fn find_page(runtime: &Runtime) -> FigureId {
        let mut pending = vec![runtime.tree().contents().unwrap()];
        while let Some(id) = pending.pop() {
            if runtime.tree().node(id).unwrap().figure_name() == "OfficialInteractiveFlowPage" {
                return id;
            }
            pending.extend(runtime.tree().child_order(id).unwrap());
        }
        panic!("page missing")
    }
    fn snapshot(runtime: &Runtime, page: FigureId) -> &FlowSnapshot {
        runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .data()
            .unwrap()
    }
    #[test]
    fn typing_changes_only_first_source_and_reflows_the_document() {
        let mut runtime = text_flow_large_example();
        runtime.frames().stabilize_for_query().unwrap();
        let page = find_page(&runtime);
        let before = snapshot(&runtime, page).clone();
        assert_eq!(before.paragraphs.len(), 40);
        assert!(before.size.height > 500.0);
        runtime
            .input()
            .dispatch_key_pressed(Key::Character('Z'), KeyModifiers::default());
        runtime.frames().stabilize_for_query().unwrap();
        let after = snapshot(&runtime, page);
        assert_ne!(after.revision, before.revision);
        assert!(
            after.paragraphs[0]
                .layout
                .request()
                .text()
                .starts_with("Normal text.Z")
        );
        assert_eq!(
            after.paragraphs[1].layout.request().text(),
            before.paragraphs[1].layout.request().text()
        );
        let revision = after.revision;
        runtime
            .input()
            .dispatch_key_pressed(Key::Character('7'), KeyModifiers::default());
        runtime.frames().stabilize_for_query().unwrap();
        assert_eq!(snapshot(&runtime, page).revision, revision);
        assert!(runtime.take_deferred_mutation_errors().is_empty());
    }
    #[test]
    fn mouse_move_uses_admitted_fragment_and_preserves_source_geometry() {
        let mut runtime = caret_example();
        runtime.frames().stabilize_for_query().unwrap();
        let page = find_page(&runtime);
        let before = snapshot(&runtime, page).clone();
        let fragment = before
            .fragments
            .iter()
            .find(|f| f.bounds.width > 10.0)
            .unwrap();
        let point = Point::new(
            fragment.bounds.x + fragment.bounds.width / 2.0,
            fragment.bounds.y + fragment.bounds.height / 2.0,
        );
        let position = before.hit_text(point).unwrap();
        assert!(before.caret(position).unwrap().bounds().height > 0.0);
        let surface = runtime
            .tree()
            .local_to_surface_transform(page)
            .unwrap()
            .transform_point(point);
        runtime
            .input()
            .dispatch_mouse_moved(surface.x(), surface.y());
        runtime.frames().stabilize_for_query().unwrap();
        let after = snapshot(&runtime, page);
        assert_eq!(after.size, before.size);
        assert_eq!(after.placements, before.placements);
        assert!(runtime.take_deferred_mutation_errors().is_empty());
    }
    #[test]
    fn multiline_label_focus_and_keyboard_scroll_use_the_core_viewport() {
        let mut runtime = super::super::text_examples::multi_line_label_example();
        let root = runtime.tree().contents().unwrap();
        runtime
            .figure(root)
            .unwrap()
            .set_bounds(Rectangle::new(0.0, 0.0, 180.0, 80.0))
            .unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        let page = find_page(&runtime);
        let viewport = runtime.tree().parent_id(page).unwrap().unwrap();
        runtime.focus().request(page).unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        let before = runtime
            .viewports()
            .handle(viewport)
            .unwrap()
            .view_location();
        assert_eq!(before.y(), 0.0);
        runtime
            .input()
            .dispatch_key_pressed(Key::ArrowDown, KeyModifiers::default());
        runtime.frames().stabilize_for_query().unwrap();
        assert_eq!(
            runtime
                .viewports()
                .handle(viewport)
                .unwrap()
                .view_location()
                .y(),
            LABEL_VERTICAL_STEP
        );
        runtime
            .input()
            .dispatch_key_pressed(Key::ArrowUp, KeyModifiers::default());
        runtime.frames().stabilize_for_query().unwrap();
        assert_eq!(
            runtime
                .viewports()
                .handle(viewport)
                .unwrap()
                .view_location()
                .y(),
            0.0
        );
    }
}
