use novadraw_render::{
    BackendCapabilities, BuiltinFont, FontDescriptor, ImageData, RenderCommandKind, ResourceId,
    SurfaceInfo, TextConstraints, TextEngine, TextError, TextLayout, TextLayoutEngine,
};
use novadraw_scene::{
    Alignment, AnchorGeometry, AnchorGeometryKey, Border, FigureStyle, FigureTree,
    ImageDisplayState, ImageFigure, LabelFigure, LineBorder, PropertyValue, Rectangle,
    RectangleFigure, Runtime, TextPlacement, TitleBarBorder,
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
fn label_uses_runtime_shaping_for_measurement_truncation_and_paint() {
    const TEXT: &str = "A grapheme-safe label that must truncate";
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)));
    let label = runtime.add_figure(
        root,
        Box::new(LabelFigure::new(TEXT).with_bounds(Rectangle::new(20.0, 20.0, 72.0, 24.0))),
    );

    runtime.refresh_label_layouts().unwrap();
    let layout = runtime.label_text_layout(label).unwrap();
    assert!(layout.is_truncated());
    assert_eq!(layout.key().text(), TEXT);
    assert!(TEXT.is_char_boundary(layout.visible_range().end));
    assert!(layout.full_width() > layout.width());
    let preferred = runtime.tree().preferred_size(label, -1.0, -1.0).unwrap();
    let minimum = runtime.tree().minimum_size(label, -1.0, -1.0).unwrap();
    assert!(preferred.0 > minimum.0);
    assert!(preferred.1 >= minimum.1);

    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();
    assert!(
        submission
            .commands
            .iter()
            .any(|command| { matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }) })
    );
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
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)));
    let label = runtime.add_figure(
        root,
        Box::new(LabelFigure::new("cache").with_bounds(Rectangle::new(20.0, 20.0, 120.0, 30.0))),
    );

    runtime.refresh_label_layouts().unwrap();
    let initial = layouts.load(Ordering::Relaxed);
    runtime.refresh_label_layouts().unwrap();
    assert_eq!(layouts.load(Ordering::Relaxed), initial);

    runtime.set_label_alignment(label, Alignment::End).unwrap();
    runtime.refresh_label_layouts().unwrap();
    assert_eq!(layouts.load(Ordering::Relaxed), initial);

    runtime.set_label_text(label, "cache invalidated").unwrap();
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
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)));
    let label = runtime.add_figure(
        root,
        Box::new(
            LabelFigure::new("Icon")
                .with_icon(image)
                .with_border(LineBorder::new(novadraw_core::Color::BLACK, 2.0))
                .with_bounds(Rectangle::new(20.0, 20.0, 160.0, 40.0)),
        ),
    );
    let replacement = runtime.register_image();
    runtime
        .complete_image(
            replacement,
            ImageData::from_rgba(10, 10, vec![128; 10 * 10 * 4], 1.0),
        )
        .unwrap();

    assert!(runtime.set_label_icon(label, Some(replacement)).unwrap());
    assert!(!runtime.set_label_icon(label, Some(replacement)).unwrap());
    assert!(
        runtime
            .set_label_text_placement(label, TextPlacement::South)
            .unwrap()
    );
    assert!(runtime.set_label_icon_text_gap(label, 9.0).unwrap());
    assert!(runtime.set_label_alignment(label, Alignment::End).unwrap());
    assert!(!runtime.set_label_alignment(label, Alignment::End).unwrap());
    assert_eq!(
        runtime.set_label_icon_text_gap(label, -1.0),
        Err(novadraw_scene::ShapeMutationError::NegativeMetric)
    );
    assert!(runtime.tree().notification_effects().iter().any(|effect| {
        matches!(
            effect,
            novadraw_scene::NotificationEffect::EmitProperty(change)
                if change.block_id == label
                    && change.property == "icon_text_gap"
                    && change.new_value == PropertyValue::Number(9.0)
        )
    }));

    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
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
    assert_eq!(runtime.tree().insets(label), Some((2.0, 2.0, 2.0, 2.0)));
    let Some(AnchorGeometry::Rectangle(icon_bounds)) =
        runtime.anchor_geometry(label, &AnchorGeometryKey::icon())
    else {
        panic!("Label must expose its icon region");
    };
    assert_eq!((icon_bounds.width, icon_bounds.height), (10.0, 10.0));
    assert!(icon_bounds.x >= 2.0 && icon_bounds.y >= 2.0);
}

#[test]
fn title_bar_border_uses_resolved_font_metrics_and_glyph_commands() {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let title_bar = TitleBarBorder::new(
        "Runtime title",
        novadraw_core::Color::rgba(0.1, 0.3, 0.6, 1.0),
    )
    .with_alignment(Alignment::Start);
    let root = runtime.set_contents(Box::new(
        RectangleFigure::new(0.0, 0.0, 320.0, 120.0).with_border(title_bar),
    ));

    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();

    assert!(runtime.tree().insets(root).unwrap().0 > 0.0);
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
fn shared_title_bar_border_keeps_metrics_per_owner() {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 180.0)));
    let child = runtime.add_figure(
        root,
        Box::new(RectangleFigure::new(20.0, 60.0, 200.0, 100.0)),
    );
    runtime.set_figure_style(
        root,
        FigureStyle {
            font: Some("12px Inter Variable".to_string()),
            ..FigureStyle::default()
        },
    );
    runtime.set_figure_style(
        child,
        FigureStyle {
            font: Some("30px Inter Variable".to_string()),
            ..FigureStyle::default()
        },
    );
    let shared: Arc<dyn Border> = Arc::new(TitleBarBorder::new(
        "Shared",
        novadraw_core::Color::rgba(0.1, 0.3, 0.6, 1.0),
    ));
    runtime
        .replace_border(root, Some(Arc::clone(&shared)))
        .unwrap();
    runtime
        .replace_border(child, Some(Arc::clone(&shared)))
        .unwrap();

    runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();

    let root_top = runtime.tree().insets(root).unwrap().0;
    let child_top = runtime.tree().insets(child).unwrap().0;
    assert!(child_top > root_top);
}

#[test]
fn font_failure_recovery_and_removal_refresh_label_and_title_snapshots() {
    let mut runtime = Runtime::empty();
    let inter = runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    runtime
        .register_builtin_font(BuiltinFont::NotoSansSc)
        .unwrap();
    let root = runtime.set_contents(Box::new(
        RectangleFigure::new(0.0, 0.0, 320.0, 180.0)
            .with_border(TitleBarBorder::new("Title", novadraw_core::Color::BLACK)),
    ));
    let label = runtime.add_figure(
        root,
        Box::new(LabelFigure::new("Label").with_bounds(Rectangle::new(20.0, 60.0, 120.0, 30.0))),
    );

    let initial = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();
    runtime.complete_submission(initial.frame_id, novadraw_render::RenderOutcome::Presented);
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
        .unwrap();
    runtime.complete_submission(failed.frame_id, novadraw_render::RenderOutcome::Presented);
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
            novadraw_render::FontData::new(BuiltinFont::Inter.bytes().to_vec()),
        )
        .unwrap();
    let recovered = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();
    runtime.complete_submission(
        recovered.frame_id,
        novadraw_render::RenderOutcome::Presented,
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
        .unwrap();
    runtime.complete_submission(removed.frame_id, novadraw_render::RenderOutcome::Presented);
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
    let root = runtime.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 100.0)));
    let image = runtime.register_image();
    let figure = runtime.add_figure(
        root,
        Box::new(ImageFigure::new(image).with_bounds(Rectangle::new(20.0, 20.0, 40.0, 30.0))),
    );
    assert_eq!(
        runtime.image_display_state(figure),
        Ok(ImageDisplayState::Pending)
    );
    assert!(runtime.set_image_alignment(figure, Alignment::End).unwrap());
    assert!(!runtime.set_image_alignment(figure, Alignment::End).unwrap());

    let pending = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();
    assert!(
        pending
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::FillRect { .. }))
    );
    assert!(
        runtime.complete_submission(pending.frame_id, novadraw_render::RenderOutcome::Presented)
    );

    runtime
        .complete_image(image, ImageData::from_rgba(2, 1, vec![255; 8], 1.0))
        .unwrap();
    let ready = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
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
    assert!(runtime.complete_submission(ready.frame_id, novadraw_render::RenderOutcome::Presented));

    runtime
        .fail_resource(image.resource_id(), "load failed")
        .unwrap();
    let failed = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
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
        &novadraw_scene::ResourceStatus::Failed {
            reason: "load failed".to_string()
        }
    );
}
