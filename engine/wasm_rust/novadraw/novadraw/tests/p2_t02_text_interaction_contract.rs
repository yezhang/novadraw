use std::sync::Arc;

use novadraw::geometry::Translatable;
use novadraw::render::{BuiltinFont, command::RenderCommandKind};
use novadraw::{
    Alignment, CaretGeometry, FigureTree, FlowPage, FlowParagraph, FlowTextPosition, FlowTextRange,
    FlowWrapping, Point, Rectangle, RectangleFigure, Runtime, SelectionQuad, TextAffinity,
    TextFlowFigure, TextFlowQueryError, TextFlowViewport, TextInteractionError, TextInteractionMap,
    TextInteractionProvider, TextMovement, TextPosition, TextRange,
};

fn runtime_with_flow(bounds: Rectangle, page: FlowPage) -> (Runtime, novadraw::FigureId) {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    runtime
        .register_builtin_font(BuiltinFont::NotoSansSc)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 480.0, 320.0)))
        .unwrap();
    let flow = runtime
        .container(root)
        .unwrap()
        .add(Box::new(TextFlowFigure::new(bounds, page)))
        .unwrap();
    runtime.prepare_frame().expect("initial TextFlow frame");
    (runtime, flow)
}

#[test]
fn caret_round_trips_through_surface_coordinates() {
    let (runtime, flow) = runtime_with_flow(
        Rectangle::new(40.0, 30.0, 240.0, 120.0),
        FlowPage::from_text("alpha beta"),
    );
    let position = FlowTextPosition::new(0, "alpha".len(), TextAffinity::Downstream);

    let caret = runtime.text_flow_caret_geometry(flow, position).unwrap();
    let hit = runtime
        .text_flow_hit_test(flow, caret.bounds().center())
        .unwrap();

    assert_eq!(hit.paragraph(), 0);
    assert_eq!(hit.byte_offset(), "alpha".len());
}

#[test]
fn centered_no_wrap_flow_offsets_paint_and_interaction_geometry_together() {
    let bounds = Rectangle::new(40.0, 30.0, 240.0, 120.0);
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 480.0, 320.0)))
        .unwrap();
    let unaligned_flow = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            TextFlowFigure::new(bounds, FlowPage::from_text("centered"))
                .with_wrapping(FlowWrapping::NoWrap),
        ))
        .unwrap();
    let flow = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            TextFlowFigure::new(bounds, FlowPage::from_text("centered"))
                .with_wrapping(FlowWrapping::NoWrap)
                .with_alignment(Alignment::Center, Alignment::Center),
        ))
        .unwrap();
    runtime.prepare_frame().expect("centered TextFlow frame");

    let layout = runtime.text_flow_layout(flow).unwrap();
    let expected_x = bounds.x + (bounds.width - f64::from(layout.width())) / 2.0;
    let expected_y = bounds.y + (bounds.height - f64::from(layout.height())) / 2.0;
    let caret = runtime
        .text_flow_caret_geometry(flow, FlowTextPosition::new(0, 0, TextAffinity::Downstream))
        .unwrap();
    let unaligned_caret = runtime
        .text_flow_caret_geometry(
            unaligned_flow,
            FlowTextPosition::new(0, 0, TextAffinity::Downstream),
        )
        .unwrap();

    assert!(
        (caret.bounds().x - unaligned_caret.bounds().x - (expected_x - bounds.x)).abs() < 0.001
    );
    assert!(
        (caret.bounds().y - unaligned_caret.bounds().y - (expected_y - bounds.y)).abs() < 0.001
    );
    let hit = runtime
        .text_flow_hit_test(flow, caret.bounds().center())
        .unwrap();
    assert_eq!(hit.byte_offset(), 0);
}

#[test]
fn text_flow_viewport_offsets_interaction_and_clips_paint_without_truncating() {
    let bounds = Rectangle::new(40.0, 30.0, 120.0, 48.0);
    let text = "this complete line remains shaped while scrolled";
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 480.0, 320.0)))
        .unwrap();
    let flow = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            TextFlowFigure::new(bounds, FlowPage::from_text(text))
                .with_wrapping(FlowWrapping::NoWrap)
                .with_viewport(TextFlowViewport::clipped(Point::new(-80.0, 0.0))),
        ))
        .unwrap();
    let frame = runtime.prepare_frame().expect("clipped TextFlow frame");
    let layout = runtime.text_flow_layout(flow).unwrap();

    assert!(!layout.is_truncated());
    assert_eq!(layout.visible_range(), 0..text.len());
    assert!(frame.commands().iter().any(|command| matches!(
        command.kind,
        RenderCommandKind::Clip { rect }
            if rect == Rectangle::new(0.0, 0.0, bounds.width, bounds.height)
    )));
    let caret = runtime
        .text_flow_local_caret_geometry(
            flow,
            FlowTextPosition::new(0, text.len(), TextAffinity::Upstream),
        )
        .unwrap();
    let hit = runtime
        .text_flow_hit_test(
            flow,
            runtime
                .text_flow_caret_geometry(
                    flow,
                    FlowTextPosition::new(0, text.len(), TextAffinity::Upstream),
                )
                .unwrap()
                .bounds()
                .center(),
        )
        .unwrap();

    assert!(caret.bounds().x < f64::from(layout.full_width()));
    assert_eq!(hit.byte_offset(), text.len());
}

#[test]
fn visual_movement_never_splits_a_grapheme_cluster() {
    let (runtime, flow) = runtime_with_flow(
        Rectangle::new(10.0, 10.0, 260.0, 120.0),
        FlowPage::from_text("a👩‍💻e\u{301}z"),
    );
    let start = FlowTextPosition::new(0, 1, TextAffinity::Downstream);

    let after_emoji = runtime
        .text_flow_move_position(flow, start, TextMovement::NextVisual)
        .unwrap();
    let after_combining = runtime
        .text_flow_move_position(flow, after_emoji, TextMovement::NextVisual)
        .unwrap();

    assert_eq!(after_emoji.byte_offset(), "a👩‍💻".len());
    assert_eq!(after_combining.byte_offset(), "a👩‍💻e\u{301}".len());
    assert!(matches!(
        runtime.text_flow_caret_geometry(
            flow,
            FlowTextPosition::new(0, "a👩‍💻e".len(), TextAffinity::Downstream),
        ),
        Err(TextFlowQueryError::Interaction(
            TextInteractionError::InvalidTextPosition
        ))
    ));
}

#[test]
fn cross_paragraph_selection_produces_ordered_visual_quads() {
    let (runtime, flow) = runtime_with_flow(
        Rectangle::new(20.0, 25.0, 100.0, 180.0),
        FlowPage::new(vec![
            FlowParagraph::from_text("first paragraph wraps"),
            FlowParagraph::from_text("第二段"),
        ]),
    );
    let range = FlowTextRange::new(
        FlowTextPosition::new(0, 6, TextAffinity::Downstream),
        FlowTextPosition::new(1, "第二".len(), TextAffinity::Upstream),
    );

    let quads = runtime.text_flow_selection_geometry(flow, range).unwrap();

    assert!(quads.len() >= 2);
    assert!(
        quads
            .windows(2)
            .all(|pair| pair[0].line_index() <= pair[1].line_index())
    );
    assert!(quads.iter().all(|quad| quad.bounds().x >= 20.0));
}

#[test]
fn wrapped_boundary_preserves_distinct_caret_affinity() {
    let (runtime, flow) = runtime_with_flow(
        Rectangle::new(0.0, 0.0, 58.0, 180.0),
        FlowPage::from_text("alpha beta gamma delta"),
    );
    let text = runtime.text_flow_layout(flow).unwrap().key().text();
    let differing_boundary = text
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .find(|offset| {
            let upstream = FlowTextPosition::new(0, *offset, TextAffinity::Upstream);
            let downstream = FlowTextPosition::new(0, *offset, TextAffinity::Downstream);
            let Ok(upstream) = runtime.text_flow_caret_geometry(flow, upstream) else {
                return false;
            };
            let Ok(downstream) = runtime.text_flow_caret_geometry(flow, downstream) else {
                return false;
            };
            upstream.bounds() != downstream.bounds()
        });

    assert!(
        differing_boundary.is_some(),
        "soft wrapping should expose an affinity-sensitive caret boundary"
    );
}

#[test]
fn invalid_utf8_and_truncated_positions_are_rejected() {
    let (mut runtime, flow) = runtime_with_flow(
        Rectangle::new(0.0, 0.0, 60.0, 80.0),
        FlowPage::from_text("中文 mixed text that wraps"),
    );
    runtime
        .text_flow(flow)
        .unwrap()
        .set_wrapping(novadraw::FlowWrapping::Truncate { max_lines: 1 })
        .unwrap();
    runtime.record_full_frame();

    let invalid_utf8 = runtime
        .text_flow_caret_geometry(flow, FlowTextPosition::new(0, 1, TextAffinity::Downstream));
    assert!(matches!(
        invalid_utf8,
        Err(TextFlowQueryError::Interaction(
            TextInteractionError::InvalidTextPosition
        ))
    ));

    let visible_end = runtime.text_flow_layout(flow).unwrap().visible_range().end;
    let invisible = runtime.text_flow_caret_geometry(
        flow,
        FlowTextPosition::new(0, visible_end + 1, TextAffinity::Downstream),
    );
    assert!(matches!(
        invisible,
        Err(TextFlowQueryError::Interaction(
            TextInteractionError::InvisibleTextPosition
        ))
    ));
}

#[test]
fn positions_from_an_old_layout_revision_are_rejected() {
    let (mut runtime, flow) = runtime_with_flow(
        Rectangle::new(0.0, 0.0, 180.0, 100.0),
        FlowPage::from_text("stale position after width change"),
    );
    let position = runtime
        .text_flow_hit_test(flow, Point::new(12.0, 8.0))
        .unwrap();
    assert!(position.layout_revision().is_some());

    runtime
        .figure(flow)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 72.0, 100.0))
        .unwrap();
    runtime.prepare_frame().expect("width change relayout");

    assert!(matches!(
        runtime.text_flow_caret_geometry(flow, position),
        Err(TextFlowQueryError::Interaction(
            TextInteractionError::StaleTextLayoutRevision
        ))
    ));
}

#[test]
fn caret_geometry_follows_nested_viewport_and_scale_transforms() {
    let mut graph = FigureTree::new();
    let root = graph
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 480.0, 320.0)))
        .expect("valid FigureTree construction");
    let viewport = graph
        .builder()
        .add_viewport_to(root, Rectangle::new(100.0, 80.0, 260.0, 180.0))
        .unwrap();
    let scalable = graph
        .builder()
        .add_scalable_layered_pane_to(viewport.figure_id(), Rectangle::new(0.0, 0.0, 300.0, 200.0))
        .unwrap();
    let flow = graph
        .builder()
        .add_child(
            scalable.figure_id(),
            Box::new(TextFlowFigure::new(
                Rectangle::new(15.0, 20.0, 160.0, 80.0),
                FlowPage::from_text("scaled"),
            )),
        )
        .unwrap();
    let mut runtime = Runtime::new(graph);
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    runtime
        .scalable(scalable.figure_id())
        .unwrap()
        .set_scale(2.0)
        .unwrap();
    runtime.prepare_frame().expect("scaled TextFlow frame");

    let local = runtime
        .text_flow_layout(flow)
        .unwrap()
        .caret_geometry(TextPosition::new(0, TextAffinity::Downstream))
        .unwrap();
    let mut expected = local.bounds();
    expected.transform(runtime.tree().local_to_surface_transform(flow).unwrap());
    let caret = runtime
        .text_flow_caret_geometry(flow, FlowTextPosition::new(0, 0, TextAffinity::Downstream))
        .unwrap();

    assert_eq!(caret.bounds(), expected);
    assert!(caret.bounds().height > local.bounds().height);
}

#[test]
fn text_queries_are_read_only_for_runtime_updates() {
    let (mut runtime, flow) = runtime_with_flow(
        Rectangle::new(20.0, 20.0, 180.0, 100.0),
        FlowPage::from_text("read only interaction"),
    );
    assert!(runtime.prepare_frame().is_none());
    let position = runtime
        .text_flow_hit_test(flow, novadraw::Point::new(30.0, 30.0))
        .unwrap();
    runtime.text_flow_caret_geometry(flow, position).unwrap();
    runtime
        .text_flow_move_position(flow, position, TextMovement::NextWord)
        .unwrap();
    assert!(runtime.prepare_frame().is_none());
}

#[derive(Debug)]
struct ExternalInteractionProvider;

impl TextInteractionProvider for ExternalInteractionProvider {
    fn text_len(&self) -> usize {
        3
    }

    fn hit_test(&self, _point: Point) -> Result<TextPosition, TextInteractionError> {
        Ok(TextPosition::new(1, TextAffinity::Downstream))
    }

    fn caret_geometry(
        &self,
        position: TextPosition,
    ) -> Result<CaretGeometry, TextInteractionError> {
        Ok(CaretGeometry::new(
            Rectangle::new(position.byte_offset() as f64, 0.0, 0.0, 12.0),
            0,
        ))
    }

    fn selection_geometry(
        &self,
        _range: TextRange,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError> {
        Ok(vec![SelectionQuad::new(
            Rectangle::new(0.0, 0.0, 3.0, 12.0),
            0,
        )])
    }

    fn move_position(
        &self,
        _position: TextPosition,
        _movement: TextMovement,
    ) -> Result<TextPosition, TextInteractionError> {
        Ok(TextPosition::new(3, TextAffinity::Upstream))
    }
}

#[test]
fn external_text_engines_can_supply_backend_neutral_interaction_maps() {
    let map = TextInteractionMap::new("abc", Arc::new(ExternalInteractionProvider)).unwrap();

    let hit = map.hit_test(Point::new(1.0, 1.0)).unwrap();
    assert_eq!(hit.byte_offset(), 1);
    assert_eq!(hit.affinity(), TextAffinity::Downstream);
    assert_eq!(hit.layout_revision(), Some(map.revision()));

    let moved = map
        .move_position(
            TextPosition::new(1, TextAffinity::Downstream),
            TextMovement::NextVisual,
        )
        .unwrap();
    assert_eq!(moved.byte_offset(), 3);
    assert_eq!(moved.affinity(), TextAffinity::Upstream);
    assert_eq!(moved.layout_revision(), Some(map.revision()));
    assert_eq!(
        TextInteractionMap::new("mismatch", Arc::new(ExternalInteractionProvider)),
        Err(TextInteractionError::ProviderTextMismatch)
    );
}

#[test]
fn paragraph_movement_uses_flow_page_boundaries_not_embedded_newlines() {
    let first = "first\nstill first";
    let (runtime, flow) = runtime_with_flow(
        Rectangle::new(0.0, 0.0, 220.0, 160.0),
        FlowPage::new(vec![
            FlowParagraph::from_text(first),
            FlowParagraph::from_text("second"),
        ]),
    );
    let position = FlowTextPosition::new(0, "first\nstill".len(), TextAffinity::Downstream);

    let start = runtime
        .text_flow_move_position(flow, position, TextMovement::ParagraphStart)
        .unwrap();
    assert_eq!((start.paragraph(), start.byte_offset()), (0, 0));
    assert_eq!(start.affinity(), TextAffinity::Downstream);
    assert!(start.layout_revision().is_some());

    let end = runtime
        .text_flow_move_position(flow, position, TextMovement::ParagraphEnd)
        .unwrap();
    assert_eq!((end.paragraph(), end.byte_offset()), (0, first.len()));
    assert_eq!(end.affinity(), TextAffinity::Upstream);
    assert!(end.layout_revision().is_some());
}

#[test]
fn runtime_text_queries_distinguish_unknown_and_wrong_capability() {
    let (mut runtime, flow) = runtime_with_flow(
        Rectangle::new(0.0, 0.0, 220.0, 100.0),
        FlowPage::from_text("query"),
    );
    let contents = runtime.tree().parent_id(flow).unwrap();
    let rectangle = runtime
        .container(contents)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let foreign = novadraw::FigureTree::new()
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)))
        .expect("valid FigureTree construction");

    assert!(matches!(
        runtime.text_flow_hit_test(rectangle, Point::ORIGIN),
        Err(TextFlowQueryError::WrongCapability(id)) if id == rectangle
    ));
    assert!(matches!(
        runtime.text_flow_hit_test(foreign, Point::ORIGIN),
        Err(TextFlowQueryError::UnknownFigure(id)) if id == foreign
    ));
    assert!(runtime.text_flow_layout(flow).is_ok());
}
