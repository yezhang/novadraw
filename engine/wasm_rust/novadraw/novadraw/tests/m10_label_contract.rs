use novadraw::render::{
    BackendCapabilities, BuiltinFont, FontDescriptor, ImageData, RenderCommandKind, ResourceId,
    SurfaceInfo, TextConstraints, TextEngine, TextError, TextLayout, TextLayoutEngine,
};
use novadraw::{
    Alignment, AnchorGeometry, AnchorGeometryKey, Border, CompoundBorder, FigureStyle, FigureTree,
    ImageDisplayState, ImageFigure, Insets, LabelFigure, LineBorder, MarginBorder,
    MeasureConstraints, Rectangle, RectangleFigure, Runtime, StackLayout, TextPlacement,
    TitleBarBorder,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct CountingTextEngine {
    inner: TextEngine,
    layouts: Arc<AtomicUsize>,
}

impl TextLayoutEngine for CountingTextEngine {
    fn revision(&self) -> u64 {
        self.inner.revision()
    }

    fn register_font(
        &mut self,
        resource_id: ResourceId,
        revision: u64,
        bytes: &[u8],
    ) -> Result<(), TextError> {
        self.inner.register_font(resource_id, revision, bytes)
    }

    fn remove_font(&mut self, resource_id: ResourceId) {
        self.inner.remove_font(resource_id);
    }

    fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        self.layouts.fetch_add(1, Ordering::Relaxed);
        self.inner.layout(text, font, constraints)
    }
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 320.0,
        logical_height: 180.0,
        pixel_width: 320,
        pixel_height: 180,
        scale_factor: 1.0,
    }
}

#[test]
fn deep_label_style_refresh_visits_each_tree_node_once() {
    const LABEL_COUNT: usize = 128;

    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)));
    let mut parent = root;
    for index in 0..LABEL_COUNT {
        parent = tree
            .builder()
            .add_child(
                parent,
                Box::new(
                    LabelFigure::new(format!("label-{index}"))
                        .with_bounds(Rectangle::new(0.0, 0.0, 80.0, 20.0)),
                ),
            )
            .unwrap();
    }
    let mut runtime = Runtime::new(tree);
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();

    runtime.refresh_label_layouts().unwrap();

    let stats = runtime.text_layout_stats();
    assert_eq!(stats.label_figures_refreshed, LABEL_COUNT as u64);
    assert_eq!(
        stats.label_style_nodes_visited,
        LABEL_COUNT as u64 + 2,
        "synthetic root, contents, and each Label must be visited once"
    );
}

#[test]
fn non_text_tree_skips_style_propagation_work() {
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)))
        .unwrap();

    runtime.prepare_frame().expect("first frame");

    assert_eq!(runtime.text_layout_stats(), Default::default());
}

#[test]
fn label_uses_runtime_shaping_for_measurement_truncation_and_paint() {
    const TEXT: &str = "A grapheme-safe label that must truncate";
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)))
        .expect("valid Runtime mutation");
    let label = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            LabelFigure::new(TEXT).with_bounds(Rectangle::new(20.0, 20.0, 72.0, 24.0)),
        ))
        .expect("valid Runtime mutation");

    runtime.refresh_label_layouts().unwrap();
    let layout = runtime.label_text_layout(label).unwrap();
    assert!(layout.is_truncated());
    assert_eq!(layout.key().text(), TEXT);
    assert!(TEXT.is_char_boundary(layout.visible_range().end));
    assert!(layout.full_width() > layout.width());
    let preferred = runtime
        .tree()
        .preferred_measurement(label, MeasureConstraints::UNBOUNDED)
        .unwrap();
    let minimum = runtime
        .tree()
        .minimum_size(label, MeasureConstraints::UNBOUNDED)
        .unwrap();
    assert!(preferred.width > minimum.width);
    assert!(preferred.height >= minimum.height);

    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert!(
        submission
            .commands
            .iter()
            .any(|command| { matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }) })
    );
}

#[test]
fn first_submission_shapes_label_after_parent_layout() {
    const TEXT: &str = "Hello world";
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)))
        .expect("valid Runtime mutation");
    runtime
        .container(root)
        .unwrap()
        .set_layout_manager(Box::new(StackLayout::new()))
        .unwrap();
    let label = runtime
        .container(root)
        .unwrap()
        .add(Box::new(LabelFigure::new(TEXT)))
        .expect("valid Runtime mutation");

    runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .expect("the stable first frame must be submitted");

    assert_eq!(
        runtime.tree().figure_bounds(label),
        Some(Rectangle::new(0.0, 0.0, 320.0, 180.0))
    );
    let layout = runtime.label_text_layout(label).unwrap();
    assert!(!layout.is_truncated());
    assert_eq!(layout.visible_range(), 0..TEXT.len());
}

#[test]
fn label_cache_reshapes_only_when_measurement_inputs_change() {
    let layouts = Arc::new(AtomicUsize::new(0));
    let mut runtime = Runtime::with_text_layout_engine(
        FigureTree::new(),
        Box::new(CountingTextEngine {
            inner: TextEngine::new(),
            layouts: Arc::clone(&layouts),
        }),
    );
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)))
        .expect("valid Runtime mutation");
    let label = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            LabelFigure::new("cache").with_bounds(Rectangle::new(20.0, 20.0, 120.0, 30.0)),
        ))
        .expect("valid Runtime mutation");

    runtime.refresh_label_layouts().unwrap();
    let initial = layouts.load(Ordering::Relaxed);
    runtime.refresh_label_layouts().unwrap();
    assert_eq!(layouts.load(Ordering::Relaxed), initial);

    runtime
        .label(label)
        .unwrap()
        .set_alignment(Alignment::End)
        .unwrap();
    runtime.refresh_label_layouts().unwrap();
    assert_eq!(layouts.load(Ordering::Relaxed), initial);

    runtime
        .label(label)
        .unwrap()
        .set_text("cache invalidated")
        .unwrap();
    runtime.refresh_label_layouts().unwrap();
    assert!(layouts.load(Ordering::Relaxed) > initial);
}

#[test]
fn label_icon_gap_placement_and_typed_mutations_are_transactional() {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let image = runtime.register_image();
    runtime
        .complete_image(
            image,
            ImageData::from_rgba(12, 8, vec![255; 12 * 8 * 4], 1.0),
        )
        .unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)))
        .expect("valid Runtime mutation");
    let label = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            LabelFigure::new("Icon")
                .with_icon(image)
                .with_border(LineBorder::new(novadraw::Color::BLACK, 2.0))
                .with_bounds(Rectangle::new(20.0, 20.0, 160.0, 40.0)),
        ))
        .expect("valid Runtime mutation");
    let replacement = runtime.register_image();
    runtime
        .complete_image(
            replacement,
            ImageData::from_rgba(10, 10, vec![128; 10 * 10 * 4], 1.0),
        )
        .unwrap();

    assert!(
        runtime
            .label(label)
            .unwrap()
            .set_icon(Some(replacement))
            .unwrap()
    );
    assert!(
        !runtime
            .label(label)
            .unwrap()
            .set_icon(Some(replacement))
            .unwrap()
    );
    assert!(
        runtime
            .label(label)
            .unwrap()
            .set_text_placement(TextPlacement::South)
            .unwrap()
    );
    assert!(
        runtime
            .label(label)
            .unwrap()
            .set_icon_text_gap(9.0)
            .unwrap()
    );
    assert!(
        runtime
            .label(label)
            .unwrap()
            .set_alignment(Alignment::End)
            .unwrap()
    );
    assert!(
        !runtime
            .label(label)
            .unwrap()
            .set_alignment(Alignment::End)
            .unwrap()
    );
    assert_eq!(
        runtime.label(label).unwrap().set_icon_text_gap(-1.0),
        Err(novadraw::ShapeMutationError::NegativeMetric)
    );
    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert!(
        submission
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::Image { .. }))
    );
    assert!(
        submission
            .commands
            .iter()
            .any(|command| { matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }) })
    );
    assert_eq!(runtime.tree().insets(label), Some(Insets::uniform(2.0)));
    let Some(AnchorGeometry::Rectangle(icon_bounds)) =
        runtime.anchor_geometry(label, &AnchorGeometryKey::icon())
    else {
        panic!("Label must expose its icon region");
    };
    assert_eq!((icon_bounds.width, icon_bounds.height), (10.0, 10.0));
    assert!(icon_bounds.x >= 2.0 && icon_bounds.y >= 2.0);
}

#[test]
fn text_placement_positions_text_relative_to_icon_in_all_four_directions() {
    const GAP: f64 = 7.0;

    for placement in [
        TextPlacement::East,
        TextPlacement::West,
        TextPlacement::North,
        TextPlacement::South,
    ] {
        let mut runtime = Runtime::empty();
        runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        let image = runtime.register_image();
        runtime
            .complete_image(
                image,
                ImageData::from_rgba(12, 8, vec![255; 12 * 8 * 4], 1.0),
            )
            .unwrap();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)))
            .expect("valid Runtime mutation");
        let label = runtime
            .container(root)
            .unwrap()
            .add(Box::new(
                LabelFigure::new("Placement")
                    .with_icon(image)
                    .with_bounds(Rectangle::new(0.0, 0.0, 160.0, 80.0)),
            ))
            .expect("valid Runtime mutation");
        runtime
            .label(label)
            .unwrap()
            .set_text_placement(placement)
            .unwrap();
        runtime
            .label(label)
            .unwrap()
            .set_icon_text_gap(GAP)
            .unwrap();

        let submission = runtime
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();
        let text = runtime.label_text_layout(label).unwrap();
        let text_origin = submission
            .commands
            .iter()
            .find_map(|command| match &command.kind {
                RenderCommandKind::DrawGlyphRun { origin, .. } => Some(*origin),
                _ => None,
            })
            .expect("Label must emit a glyph run");
        let image_bounds = submission
            .commands
            .iter()
            .find_map(|command| match &command.kind {
                RenderCommandKind::Image { dest_rect, .. } => Some(*dest_rect),
                _ => None,
            })
            .expect("Label must emit an image");
        let Some(AnchorGeometry::Rectangle(icon_geometry)) =
            runtime.anchor_geometry(label, &AnchorGeometryKey::icon())
        else {
            panic!("Label must expose its icon region");
        };
        assert_eq!(*icon_geometry, image_bounds);

        match placement {
            TextPlacement::East => {
                assert_eq!(text_origin.x(), image_bounds.x + image_bounds.width + GAP)
            }
            TextPlacement::West => {
                assert_eq!(
                    text_origin.x() + f64::from(text.width()) + GAP,
                    image_bounds.x
                )
            }
            TextPlacement::North => {
                assert_eq!(
                    text_origin.y() + f64::from(text.height()) + GAP,
                    image_bounds.y
                )
            }
            TextPlacement::South => {
                assert_eq!(image_bounds.y + image_bounds.height + GAP, text_origin.y())
            }
        }
    }
}

#[test]
fn title_bar_border_uses_resolved_font_metrics_and_glyph_commands() {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let title_bar = TitleBarBorder::new("Runtime title", novadraw::Color::rgba(0.1, 0.3, 0.6, 1.0))
        .with_alignment(Alignment::Start);
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 320.0, 120.0).with_border(title_bar),
        ))
        .expect("valid Runtime mutation");

    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();

    assert!(runtime.tree().insets(root).unwrap().top > 0.0);
    assert!(
        submission
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::FillRect { .. }))
    );
    assert!(
        submission
            .commands
            .iter()
            .any(|command| { matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }) })
    );
    assert_eq!(
        runtime.tree().border_is_effectively_opaque(root),
        Some(true)
    );
}

#[test]
fn compound_border_resolves_title_bar_snapshots_at_every_nesting_position() {
    let borders: Vec<(Arc<dyn Border>, usize)> = vec![
        (
            Arc::new(CompoundBorder::new(
                LineBorder::new(novadraw::Color::BLACK, 2.0),
                TitleBarBorder::new("Inner", novadraw::Color::RED),
            )),
            1,
        ),
        (
            Arc::new(CompoundBorder::new(
                TitleBarBorder::new("Outer", novadraw::Color::RED),
                LineBorder::new(novadraw::Color::BLACK, 2.0),
            )),
            1,
        ),
        (
            Arc::new(CompoundBorder::new(
                MarginBorder::new(novadraw::Color::TRANSPARENT, 1.0),
                CompoundBorder::new(
                    LineBorder::new(novadraw::Color::BLACK, 2.0),
                    TitleBarBorder::new("Nested", novadraw::Color::RED),
                ),
            )),
            1,
        ),
        (
            Arc::new(CompoundBorder::new(
                TitleBarBorder::new("First", novadraw::Color::RED),
                CompoundBorder::new(
                    LineBorder::new(novadraw::Color::BLACK, 2.0),
                    TitleBarBorder::new("Second", novadraw::Color::BLUE),
                ),
            )),
            2,
        ),
    ];

    for (border, expected_glyph_runs) in borders {
        let mut runtime = Runtime::empty();
        runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 120.0)))
            .expect("valid Runtime mutation");
        runtime.border(root).unwrap().replace(Some(border)).unwrap();

        let submission = runtime
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
            .into_ready()
            .unwrap();

        let insets = runtime.tree().insets(root).unwrap();
        assert!(insets.top > 2.0);
        assert_eq!(
            runtime
                .tree()
                .preferred_measurement(root, MeasureConstraints::UNBOUNDED)
                .map(|measurement| measurement.size()),
            Some(novadraw::Dimension::new(
                320.0 + insets.width(),
                120.0 + insets.height(),
            ))
        );
        assert_eq!(
            submission
                .commands
                .iter()
                .filter(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
                .count(),
            expected_glyph_runs
        );
        assert!(runtime.title_bar_text_layout(root).is_ok());
    }
}

#[test]
fn shared_compound_title_bar_border_keeps_metrics_per_owner() {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)))
        .expect("valid Runtime mutation");
    let child = runtime
        .container(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 60.0, 200.0, 100.0)))
        .expect("valid Runtime mutation");
    runtime
        .figure(root)
        .unwrap()
        .set_style(FigureStyle {
            font: Some("12px Inter Variable".to_string()),
            ..FigureStyle::default()
        })
        .expect("valid Runtime mutation");
    runtime
        .figure(child)
        .unwrap()
        .set_style(FigureStyle {
            font: Some("30px Inter Variable".to_string()),
            ..FigureStyle::default()
        })
        .expect("valid Runtime mutation");
    let shared: Arc<dyn Border> = Arc::new(CompoundBorder::new(
        LineBorder::new(novadraw::Color::BLACK, 2.0),
        TitleBarBorder::new("Shared", novadraw::Color::rgba(0.1, 0.3, 0.6, 1.0)),
    ));
    runtime
        .border(root)
        .unwrap()
        .replace(Some(Arc::clone(&shared)))
        .unwrap();
    runtime
        .border(child)
        .unwrap()
        .replace(Some(Arc::clone(&shared)))
        .unwrap();

    runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();

    let root_top = runtime.tree().insets(root).unwrap().top;
    let child_top = runtime.tree().insets(child).unwrap().top;
    assert!(child_top > root_top);
    assert_eq!(runtime.tree().insets(root).unwrap().left, 2.0);
    assert_eq!(runtime.tree().insets(child).unwrap().left, 2.0);
}

#[test]
fn font_failure_recovery_and_removal_refresh_label_and_title_snapshots() {
    let mut runtime = Runtime::empty();
    let inter = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    runtime
        .register_builtin_font(BuiltinFont::NotoSansSc)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 320.0, 180.0)
                .with_border(TitleBarBorder::new("Title", novadraw::Color::BLACK)),
        ))
        .expect("valid Runtime mutation");
    let label = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            LabelFigure::new("Label").with_bounds(Rectangle::new(20.0, 60.0, 120.0, 30.0)),
        ))
        .expect("valid Runtime mutation");

    let initial = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    runtime.complete_submission(
        initial.session_id,
        initial.frame_id,
        novadraw::render::RenderOutcome::Presented,
    );
    let initial_label_revision = runtime
        .label_text_layout(label)
        .unwrap()
        .key()
        .engine_revision();
    let initial_title_revision = runtime
        .title_bar_text_layout(root)
        .unwrap()
        .key()
        .engine_revision();

    runtime
        .fail_resource(inter.resource_id(), "font unavailable")
        .unwrap();
    let failed = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    runtime.complete_submission(
        failed.session_id,
        failed.frame_id,
        novadraw::render::RenderOutcome::Presented,
    );
    assert!(
        runtime
            .label_text_layout(label)
            .unwrap()
            .key()
            .engine_revision()
            > initial_label_revision
    );
    assert!(
        runtime
            .title_bar_text_layout(root)
            .unwrap()
            .key()
            .engine_revision()
            > initial_title_revision
    );

    runtime
        .complete_font(
            inter,
            novadraw::render::FontData::new(BuiltinFont::Inter.bytes().to_vec()),
        )
        .unwrap();
    let recovered = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    runtime.complete_submission(
        recovered.session_id,
        recovered.frame_id,
        novadraw::render::RenderOutcome::Presented,
    );
    let recovered_revision = runtime
        .label_text_layout(label)
        .unwrap()
        .key()
        .engine_revision();
    assert!(recovered_revision > initial_label_revision);

    runtime.remove_resource(inter.resource_id()).unwrap();
    let removed = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    runtime.complete_submission(
        removed.session_id,
        removed.frame_id,
        novadraw::render::RenderOutcome::Presented,
    );
    assert!(
        runtime
            .label_text_layout(label)
            .unwrap()
            .key()
            .engine_revision()
            > recovered_revision
    );
}

#[test]
fn image_figure_tracks_pending_ready_and_failed_resource_states() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)))
        .expect("valid Runtime mutation");
    let image = runtime.register_image();
    let figure = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            ImageFigure::new(image).with_bounds(Rectangle::new(20.0, 20.0, 40.0, 30.0)),
        ))
        .expect("valid Runtime mutation");
    assert_eq!(
        runtime.image_display_state(figure),
        Ok(ImageDisplayState::Pending)
    );
    assert!(
        runtime
            .image(figure)
            .unwrap()
            .set_alignment(Alignment::End)
            .unwrap()
    );
    assert!(
        !runtime
            .image(figure)
            .unwrap()
            .set_alignment(Alignment::End)
            .unwrap()
    );

    let pending = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert!(
        pending
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::FillRect { .. }))
    );
    assert!(runtime.complete_submission(
        pending.session_id,
        pending.frame_id,
        novadraw::render::RenderOutcome::Presented
    ));

    runtime
        .complete_image(image, ImageData::from_rgba(2, 1, vec![255; 8], 1.0))
        .unwrap();
    let ready = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_eq!(
        runtime.image_display_state(figure),
        Ok(ImageDisplayState::Ready)
    );
    assert!(
        ready
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::Image { .. }))
    );
    assert!(runtime.complete_submission(
        ready.session_id,
        ready.frame_id,
        novadraw::render::RenderOutcome::Presented
    ));

    runtime
        .fail_resource(image.resource_id(), "load failed")
        .unwrap();
    let failed = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_eq!(
        runtime.image_display_state(figure),
        Ok(ImageDisplayState::Failed)
    );
    assert!(
        failed
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::FillRect { .. }))
    );
    assert_eq!(
        runtime.resource_status(image.resource_id()).unwrap(),
        &novadraw::ResourceStatus::Failed {
            reason: "load failed".to_string()
        }
    );
}

#[test]
fn removing_ready_image_clears_all_shared_figure_references() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)))
        .expect("valid Runtime mutation");
    let image = runtime.register_image();
    runtime
        .complete_image(image, ImageData::from_rgba(2, 1, vec![255; 8], 1.0))
        .unwrap();
    let first = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            ImageFigure::new(image).with_bounds(Rectangle::new(20.0, 20.0, 40.0, 30.0)),
        ))
        .expect("valid Runtime mutation");
    let second = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            ImageFigure::new(image).with_bounds(Rectangle::new(80.0, 20.0, 40.0, 30.0)),
        ))
        .expect("valid Runtime mutation");

    let ready = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    assert_eq!(
        ready
            .commands
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::Image { .. }))
            .count(),
        2
    );
    assert!(runtime.complete_submission(
        ready.session_id,
        ready.frame_id,
        novadraw::render::RenderOutcome::Presented
    ));

    runtime.remove_resource(image.resource_id()).unwrap();
    let removed = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();

    assert_eq!(
        runtime.image_display_state(first),
        Ok(ImageDisplayState::Unavailable)
    );
    assert_eq!(
        runtime.image_display_state(second),
        Ok(ImageDisplayState::Unavailable)
    );
    assert!(
        removed
            .commands
            .iter()
            .all(|command| !matches!(command.kind, RenderCommandKind::Image { .. }))
    );
    assert!(matches!(
        &removed.resources,
        novadraw::render::ResourceSync::Delta(delta)
            if matches!(
                delta.ops.as_slice(),
                [novadraw::render::ResourceOp::Remove(id)] if *id == image.resource_id()
            )
    ));
    assert!(runtime.complete_submission(
        removed.session_id,
        removed.frame_id,
        novadraw::render::RenderOutcome::Presented
    ));
    assert!(matches!(
        runtime.prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL),
        novadraw::FramePreparation::Idle
    ));
}
