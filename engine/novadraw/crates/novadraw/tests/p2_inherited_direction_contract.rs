use novadraw::{
    figure::{FigureStyle, LayoutDirection, RectangleFigure},
    geometry::{Point, Rectangle},
    graphics::Color,
    runtime::Runtime,
    text::{
        BuiltinFont, FontDescriptor, ParagraphAlignment, ParagraphDirection, StyledTextRequest,
        TextAffinity, TextConstraints, TextInlineBox, TextPosition, TextRange, TextStyle,
        TextStyleSpan, TextSystem,
    },
    tree::FigureTree,
};
use std::sync::{Arc, Mutex};

struct ObservingEngine {
    delegate: novadraw::text::shaping::ParleyTextEngine,
    directions: Arc<Mutex<Vec<LayoutDirection>>>,
}
impl novadraw::text::shaping::TextLayoutEngine for ObservingEngine {
    fn revision(&self) -> u64 {
        self.delegate.revision()
    }
    fn register_font(
        &mut self,
        id: novadraw::render::ResourceId,
        revision: u64,
        bytes: &[u8],
    ) -> Result<(), novadraw::text::TextError> {
        self.delegate.register_font(id, revision, bytes)
    }
    fn remove_font(&mut self, id: novadraw::render::ResourceId) {
        self.delegate.remove_font(id);
    }
    fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<novadraw::text::TextLayout, novadraw::text::TextError> {
        self.delegate.layout(text, font, constraints)
    }
    fn layout_styled(
        &mut self,
        request: &StyledTextRequest,
    ) -> Result<novadraw::text::StyledTextLayout, novadraw::text::TextError> {
        self.directions
            .lock()
            .unwrap()
            .push(request.layout_direction());
        self.delegate.layout_styled(request)
    }
}

#[test]
fn external_engine_receives_inherited_direction_and_mutation_replaces_snapshot() {
    use novadraw::figure::flow::{FlowPageFigure, FlowSnapshot, StructuredTextFigure};
    let mut tree = FigureTree::new();
    let page = tree
        .builder()
        .set_contents(Box::new(
            FlowPageFigure::new(Rectangle::new(0.0, 0.0, 240.0, 100.0))
                .expect("valid detached figure input"),
        ))
        .unwrap();
    tree.builder()
        .add_child(page, Box::new(StructuredTextFigure::new("abc عربي xyz")))
        .unwrap();
    let directions = Arc::new(Mutex::new(Vec::new()));
    let engine = ObservingEngine {
        delegate: Default::default(),
        directions: directions.clone(),
    };
    let mut runtime = Runtime::with_text_layout_engine(tree, Box::new(engine));
    for font in [BuiltinFont::Inter, BuiltinFont::NotoSansArabic] {
        runtime.resources().register_builtin_font(font).unwrap();
    }
    runtime.frames().stabilize_for_query().unwrap();
    let before = runtime
        .tree()
        .prepared_presentation(page)
        .unwrap()
        .unwrap()
        .data::<FlowSnapshot>()
        .unwrap()
        .paragraphs[0]
        .layout
        .clone();
    runtime
        .figure(page)
        .unwrap()
        .set_style(FigureStyle {
            layout_direction: Some(LayoutDirection::RightToLeft),
            ..Default::default()
        })
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let after = &runtime
        .tree()
        .prepared_presentation(page)
        .unwrap()
        .unwrap()
        .data::<FlowSnapshot>()
        .unwrap()
        .paragraphs[0]
        .layout;
    assert_eq!(
        after.request().layout_direction(),
        LayoutDirection::RightToLeft
    );
    let position = before.layout().hit_test_text(Point::new(1.0, 5.0)).unwrap();
    assert_eq!(
        after.layout().caret_geometry(position),
        Err(novadraw::text::TextInteractionError::StaleTextLayoutRevision)
    );
    let observed = directions.lock().unwrap();
    assert!(observed.contains(&LayoutDirection::LeftToRight));
    assert_eq!(observed.last(), Some(&LayoutDirection::RightToLeft));
}

fn system() -> TextSystem {
    let mut system = TextSystem::new();
    for font in [BuiltinFont::Inter, BuiltinFont::NotoSansArabic] {
        system.register_builtin_font(font).unwrap();
    }
    system
}

fn request(direction: ParagraphDirection, alignment: ParagraphAlignment) -> StyledTextRequest {
    let style = TextStyle::new(
        FontDescriptor::new(BuiltinFont::Inter.family(), 16.0).unwrap(),
        Color::BLACK,
    );
    StyledTextRequest::new(
        "abc شصجح xyz",
        style.clone(),
        vec![
            TextStyleSpan {
                range: 0..4,
                style: style.clone(),
            },
            TextStyleSpan {
                range: 4..12,
                style: style.clone(),
            },
            TextStyleSpan {
                range: 12..16,
                style,
            },
        ],
        vec![],
        TextConstraints::new(Some(240.0)).unwrap(),
    )
    .unwrap()
    .with_paragraph(direction, alignment, true)
}

#[test]
fn mirror_moves_fragments_without_reflecting_glyphs_in_twenty_contexts() {
    let mut system = system();
    for direction in [
        ParagraphDirection::LeftToRight,
        ParagraphDirection::RightToLeft,
    ] {
        for alignment in [
            ParagraphAlignment::Leading,
            ParagraphAlignment::Trailing,
            ParagraphAlignment::Left,
            ParagraphAlignment::Right,
            ParagraphAlignment::Center,
        ] {
            let input = request(direction, alignment);
            let normal = system
                .measure(input.default_style().font().clone())
                .layout_styled(&input)
                .unwrap();
            let mirrored_input = input
                .clone()
                .with_layout_direction(LayoutDirection::RightToLeft);
            let mirrored = system
                .measure(input.default_style().font().clone())
                .layout_styled(&mirrored_input)
                .unwrap();
            assert_eq!(normal.runs().len(), mirrored.runs().len());
            for (original, moved) in normal.runs().iter().zip(mirrored.runs()) {
                assert_eq!(original.source_range, moved.source_range);
                assert_eq!(original.style_index, moved.style_index);
                assert_eq!(original.line_index, moved.line_index);
                assert_eq!(original.right_to_left, moved.right_to_left);
                assert_eq!(original.baseline, moved.baseline);
                assert_eq!(original.bounds.y, moved.bounds.y);
                assert_eq!(original.bounds.width, moved.bounds.width);
                assert_eq!(original.bounds.height, moved.bounds.height);
            }
            assert_ne!(
                normal.layout().interaction_map().unwrap().revision(),
                mirrored.layout().interaction_map().unwrap().revision()
            );
            for (index, (original, moved)) in normal
                .layout()
                .glyph_runs()
                .iter()
                .zip(mirrored.layout().glyph_runs())
                .enumerate()
            {
                assert_eq!(original.font, moved.font);
                assert_eq!(original.normalized_coords, moved.normalized_coords);
                let dx = moved.glyphs[0].x - original.glyphs[0].x;
                assert!(
                    (mirrored.runs()[index].bounds.x
                        - normal.runs()[index].bounds.x
                        - f64::from(dx))
                    .abs()
                        < 0.0001
                );
                for (a, b) in original.glyphs.iter().zip(&moved.glyphs) {
                    assert_eq!(a.id, b.id);
                    assert_eq!(a.y, b.y);
                    assert!((b.x - a.x - dx).abs() < 0.0001);
                }
            }
            for run in mirrored.runs() {
                for (offset, affinity) in [
                    (run.source_range.start, TextAffinity::Downstream),
                    (run.source_range.end, TextAffinity::Upstream),
                ] {
                    let position = TextPosition::new(offset, affinity);
                    let caret = mirrored.layout().caret_geometry(position).unwrap();
                    let rect = caret.bounds();
                    let hit = mirrored
                        .layout()
                        .hit_test_text(Point::new(rect.x, rect.y + rect.height * 0.5))
                        .unwrap();
                    let hit_caret = mirrored.layout().caret_geometry(hit).unwrap();
                    assert!(
                        (hit_caret.bounds().x - rect.x).abs() < 0.001,
                        "{direction:?} {alignment:?} {offset} expected {rect:?}, hit {hit:?} got {:?}",
                        hit_caret.bounds()
                    );
                }
            }
            let range = TextRange::new(
                TextPosition::new(0, TextAffinity::Downstream),
                TextPosition::new(input.text().len(), TextAffinity::Upstream),
            );
            let width = |layout: &novadraw::text::StyledTextLayout| {
                layout
                    .layout()
                    .selection_geometry(range)
                    .unwrap()
                    .iter()
                    .map(|quad| quad.bounds().width)
                    .sum::<f64>()
            };
            assert!((width(&normal) - width(&mirrored)).abs() < 0.001);
            use novadraw::text::TextMovement;
            let mut position = mirrored
                .layout()
                .move_text_position(
                    TextPosition::new(0, TextAffinity::Downstream),
                    TextMovement::LineStart,
                )
                .unwrap();
            for _ in 0..input.text().len() * 2 {
                let next = mirrored
                    .layout()
                    .move_text_position(position, TextMovement::NextVisual)
                    .unwrap();
                let a = mirrored
                    .layout()
                    .caret_geometry(position)
                    .unwrap()
                    .bounds()
                    .x;
                let b = mirrored.layout().caret_geometry(next).unwrap().bounds().x;
                assert!(
                    b >= a - 0.001,
                    "visual move {position:?} -> {next:?}: {a} -> {b}"
                );
                if next == position {
                    break;
                }
                position = next;
            }
            let expected_end = mirrored
                .layout()
                .move_text_position(position, TextMovement::LineEnd)
                .unwrap();
            assert!(
                (mirrored
                    .layout()
                    .caret_geometry(expected_end)
                    .unwrap()
                    .bounds()
                    .x
                    - mirrored
                        .layout()
                        .caret_geometry(position)
                        .unwrap()
                        .bounds()
                        .x)
                    .abs()
                    < 0.001,
                "{direction:?} {alignment:?}: traversal stopped at {position:?} {:?}, expected end {expected_end:?} {:?}",
                mirrored.layout().caret_geometry(position).unwrap().bounds(),
                mirrored
                    .layout()
                    .caret_geometry(expected_end)
                    .unwrap()
                    .bounds(),
            );
        }
    }
}

#[test]
fn nested_override_and_noop_mutation_preserve_resolved_direction() {
    let mut tree = FigureTree::new();
    let parent = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 200.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let child = tree
        .builder()
        .add_child(
            parent,
            Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 60.0).expect("valid figure bounds")),
        )
        .unwrap();
    let leaf = tree
        .builder()
        .add_child(
            child,
            Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds")),
        )
        .unwrap();
    let mut runtime = Runtime::new(tree);
    let mirrored = FigureStyle {
        layout_direction: Some(LayoutDirection::RightToLeft),
        ..Default::default()
    };
    assert!(
        runtime
            .figure(parent)
            .unwrap()
            .set_style(mirrored.clone())
            .unwrap()
    );
    assert!(!runtime.figure(parent).unwrap().set_style(mirrored).unwrap());
    assert_eq!(
        runtime
            .tree()
            .resolved_style(leaf)
            .unwrap()
            .layout_direction,
        LayoutDirection::RightToLeft
    );
    runtime
        .figure(child)
        .unwrap()
        .set_style(FigureStyle {
            layout_direction: Some(LayoutDirection::LeftToRight),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        runtime
            .tree()
            .resolved_style(leaf)
            .unwrap()
            .layout_direction,
        LayoutDirection::LeftToRight
    );
    runtime
        .figure(child)
        .unwrap()
        .set_style(FigureStyle::default())
        .unwrap();
    assert_eq!(
        runtime
            .tree()
            .resolved_style(leaf)
            .unwrap()
            .layout_direction,
        LayoutDirection::RightToLeft
    );
}

#[test]
fn narrow_lines_and_inline_boxes_keep_baselines_and_checked_geometry() {
    let mut system = system();
    let input = StyledTextRequest::new(
        "ab cde fgh ijkl",
        TextStyle::new(FontDescriptor::default(), Color::BLACK),
        vec![],
        vec![TextInlineBox {
            id: 4,
            byte_offset: 3,
            width: 24.0,
            height: 17.0,
        }],
        TextConstraints::new(Some(65.0)).unwrap(),
    )
    .unwrap()
    .with_layout_direction(LayoutDirection::RightToLeft);
    let layout = system
        .measure(FontDescriptor::default())
        .layout_styled(&input)
        .unwrap();
    assert!(layout.layout().line_metrics().len() > 1);
    let inline = &layout.inline_placements()[0];
    assert_eq!(
        inline.bounds.y + inline.bounds.height,
        f64::from(layout.layout().line_metrics()[inline.line_index].baseline)
    );
    assert!(inline.bounds.x >= 0.0 && inline.bounds.x + inline.bounds.width <= 65.001);
    for run in layout.runs() {
        let caret = layout
            .layout()
            .caret_geometry(TextPosition::new(
                run.source_range.start,
                TextAffinity::Downstream,
            ))
            .unwrap();
        assert_eq!(caret.line_index(), run.line_index);
    }
}

#[test]
fn mirrored_scroll_pane_places_vertical_bar_on_left_and_restores_on_mutation() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 200.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let pane = tree
        .builder()
        .add_scroll_pane_to(root, Rectangle::new(0.0, 0.0, 200.0, 100.0))
        .unwrap();
    tree.builder()
        .set_scroll_pane_contents(
            &pane,
            Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 320.0).expect("valid figure bounds")),
        )
        .unwrap();
    let mut runtime = Runtime::new(tree);
    runtime
        .figure(root)
        .unwrap()
        .set_style(FigureStyle {
            layout_direction: Some(LayoutDirection::RightToLeft),
            ..Default::default()
        })
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(
        runtime
            .tree()
            .figure_bounds(pane.vertical_scroll_bar())
            .unwrap()
            .x,
        0.0
    );
    assert!(
        runtime
            .tree()
            .figure_bounds(pane.viewport().figure_id())
            .unwrap()
            .x
            > 0.0
    );
    runtime
        .input()
        .dispatch_scroll(novadraw::event::WheelEvent::new(100.0, 50.0, 1.0, -1.0));
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(pane.viewport().view_location(), Point::new(24.0, 24.0));
    runtime
        .figure(pane.pane_id())
        .unwrap()
        .set_focusable(true)
        .unwrap();
    runtime.focus().request(pane.pane_id()).unwrap();
    runtime.input().dispatch_key_pressed(
        novadraw::event::Key::ArrowRight,
        novadraw::event::KeyModifiers::default(),
    );
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(pane.viewport().view_location(), Point::new(0.0, 24.0));
    runtime
        .figure(root)
        .unwrap()
        .set_style(FigureStyle::default())
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert!(
        runtime
            .tree()
            .figure_bounds(pane.vertical_scroll_bar())
            .unwrap()
            .x
            > 0.0
    );
    assert_eq!(
        runtime
            .tree()
            .figure_bounds(pane.viewport().figure_id())
            .unwrap()
            .x,
        0.0
    );
    runtime.input().dispatch_key_pressed(
        novadraw::event::Key::ArrowRight,
        novadraw::event::KeyModifiers::default(),
    );
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(pane.viewport().view_location(), Point::new(24.0, 24.0));
    runtime
        .input()
        .dispatch_scroll(novadraw::event::WheelEvent::new(100.0, 50.0, 1.0, -1.0));
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(pane.viewport().view_location(), Point::new(0.0, 48.0));
}

#[test]
fn external_layout_reads_direction_through_frozen_query_and_revalidates() {
    use novadraw::{
        figure::{FigureMeasurement, MeasureConstraints},
        geometry::Dimension,
        layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery},
        tree::FigureId,
    };
    struct ExternalLayout(FigureId);
    impl LayoutManager for ExternalLayout {
        fn preferred_measurement(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &LayoutSceneQuery<'_>,
        ) -> Result<FigureMeasurement, LayoutError> {
            Ok(FigureMeasurement::new(20.0, 20.0, None))
        }
        fn minimum_size(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &LayoutSceneQuery<'_>,
        ) -> Result<Dimension, LayoutError> {
            Ok(Dimension::new(20.0, 20.0))
        }
        fn layout(
            &mut self,
            container: FigureId,
            query: &LayoutSceneQuery<'_>,
            out: &mut LayoutOutput,
        ) -> Result<(), LayoutError> {
            let area = query.container_bounds(container)?;
            let x = if query.layout_direction(container)?.is_mirrored() {
                area.width - 20.0
            } else {
                0.0
            };
            out.set_child_bounds(self.0, Rectangle::new(x, 0.0, 20.0, 20.0));
            Ok(())
        }
    }
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 200.0, 100.0).expect("valid figure bounds"),
        ))
        .unwrap();
    let child = tree
        .builder()
        .add_child(
            root,
            Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds")),
        )
        .unwrap();
    tree.builder()
        .set_layout_manager(root, Box::new(ExternalLayout(child)))
        .unwrap();
    let mut runtime = Runtime::new(tree);
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().x, 0.0);
    runtime
        .figure(root)
        .unwrap()
        .set_style(FigureStyle {
            layout_direction: Some(LayoutDirection::RightToLeft),
            ..Default::default()
        })
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(runtime.tree().figure_bounds(child).unwrap().x, 180.0);
}
