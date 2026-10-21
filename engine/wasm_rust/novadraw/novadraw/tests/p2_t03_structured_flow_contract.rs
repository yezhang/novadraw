mod support;
use novadraw::{
    figure::{
        Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureMeasurement,
        FigureStyle, MeasureConstraints, flow::*,
    },
    geometry::{Insets, Point, Rectangle},
    graphics::Color,
    runtime::Runtime,
    text::{
        BuiltinFont, ParagraphAlignment, ParagraphDirection, TextAffinity, TextInteractionError,
    },
    tree::FigureId,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use support::record_full_submission;

fn fixture(width: f64) -> (Runtime, FigureId) {
    let mut runtime = Runtime::empty();
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let page = runtime
        .set_contents(Box::new(
            FlowPageFigure::new(Rectangle::new(0.0, 0.0, width, 400.0)).with_style(FigureStyle {
                font: Some("16px Inter".into()),
                ..FigureStyle::default()
            }),
        ))
        .unwrap();
    (runtime, page)
}
fn add(runtime: &mut Runtime, parent: FigureId, figure: impl Figure + 'static) -> FigureId {
    runtime
        .figure(parent)
        .unwrap()
        .add(Box::new(figure))
        .unwrap()
}
fn snapshot(runtime: &Runtime, page: FigureId) -> &FlowSnapshot {
    runtime
        .tree()
        .prepared_presentation(page)
        .unwrap()
        .unwrap()
        .data::<FlowSnapshot>()
        .unwrap()
}
fn position(snapshot: &FlowSnapshot, source: FigureId, offset: usize) -> SourceTextPosition {
    SourceTextPosition {
        source,
        byte_offset: offset,
        affinity: TextAffinity::Downstream,
        revision: snapshot.revision,
    }
}

#[test]
fn source_identity_styles_baselines_and_cross_node_selection_share_snapshot() {
    let (mut runtime, page) = fixture(300.0);
    let block = add(&mut runtime, page, BlockFlowFigure::default());
    let a = add(&mut runtime, block, StructuredTextFigure::new("small "));
    let group = add(
        &mut runtime,
        block,
        InlineFlowFigure::new().with_style(FigureStyle {
            font: Some("30px Inter".into()),
            foreground: Some(Color::RED),
            ..FigureStyle::default()
        }),
    );
    let b = add(&mut runtime, group, StructuredTextFigure::new("BIG"));
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    assert_eq!(s.paragraphs.len(), 1);
    let p = &s.paragraphs[0];
    assert_eq!(p.layout.request().text(), "small BIG");
    assert_eq!(p.layout.layout().line_metrics().len(), 1);
    assert!(
        p.layout
            .layout()
            .glyph_runs()
            .iter()
            .any(|r| r.font_size == 30.0)
    );
    let baseline = p.layout.layout().baseline();
    assert!(
        p.layout
            .layout()
            .glyph_runs()
            .iter()
            .all(|r| r.glyphs.iter().all(|g| g.y == baseline))
    );
    assert_eq!(p.layout.request().spans()[1].style.color(), Color::RED);
    assert!(
        !s.selection(position(s, a, 1), position(s, b, 2))
            .unwrap()
            .is_empty()
    );
    let fragment = s.fragments.iter().find(|f| f.source == b).unwrap();
    assert_eq!(
        s.hit_text(Point::new(
            fragment.bounds.x + fragment.bounds.width / 2.0,
            fragment.bounds.y + fragment.bounds.height / 2.0
        ))
        .unwrap()
        .source,
        b
    );
}

#[test]
fn multi_line_inline_blank_is_not_a_hit_and_caret_rejects_old_revision() {
    let (mut runtime, page) = fixture(130.0);
    let inline = add(&mut runtime, page, InlineFlowFigure::new());
    let text = add(
        &mut runtime,
        inline,
        StructuredTextFigure::new("one two three four five six seven"),
    );
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page).clone();
    assert!(s.fragments.iter().filter(|f| f.source == inline).count() > 1);
    let last = s.fragments.iter().rfind(|f| f.source == text).unwrap();
    let blank = Point::new(
        last.bounds.x + last.bounds.width + 2.0,
        last.bounds.y + last.bounds.height / 2.0,
    );
    assert!(
        !s.fragments
            .iter()
            .any(|f| f.source == inline && f.bounds.contains(blank))
    );
    assert_ne!(
        runtime.tree().hit_test_simple((blank.x(), blank.y())),
        Some(inline)
    );
    let old = position(&s, text, 2);
    runtime
        .figure(text)
        .unwrap()
        .update_component(SetStructuredFlowText::new("new text"))
        .unwrap();
    record_full_submission(&mut runtime);
    assert_eq!(
        snapshot(&runtime, page).caret(old),
        Err(TextInteractionError::StaleTextLayoutRevision)
    );
}

#[test]
fn adjacent_block_margins_collapse_and_newline_only_does_not_wrap() {
    let (mut runtime, page) = fixture(100.0);
    let first = add(
        &mut runtime,
        page,
        BlockFlowFigure::new(FlowBlockStyle {
            margin: Insets::new(0.0, 0.0, 20.0, 0.0),
            ..FlowBlockStyle::default()
        })
        .unwrap(),
    );
    add(&mut runtime, first, StructuredTextFigure::new("first"));
    let second = add(
        &mut runtime,
        page,
        BlockFlowFigure::new(FlowBlockStyle {
            margin: Insets::new(12.0, 0.0, 0.0, 0.0),
            wrap: false,
            ..FlowBlockStyle::default()
        })
        .unwrap(),
    );
    add(
        &mut runtime,
        second,
        StructuredTextFigure::new("a very long line that exceeds width\nsecond"),
    );
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    assert_eq!(s.paragraphs.len(), 2);
    let a = &s.paragraphs[0];
    let b = &s.paragraphs[1];
    assert!(
        (b.origin.y() - a.origin.y() - f64::from(a.layout.layout().height()) - 20.0).abs() < 0.01
    );
    assert_eq!(b.layout.layout().line_metrics().len(), 2);
}

#[test]
fn nested_margins_apply_once_at_the_outer_edges() {
    let (mut runtime, page) = fixture(200.0);
    let outer = add(
        &mut runtime,
        page,
        BlockFlowFigure::new(FlowBlockStyle {
            margin: Insets::new(10.0, 0.0, 30.0, 0.0),
            ..FlowBlockStyle::default()
        })
        .unwrap(),
    );
    let a = add(
        &mut runtime,
        outer,
        BlockFlowFigure::new(FlowBlockStyle {
            margin: Insets::new(20.0, 0.0, 5.0, 0.0),
            ..FlowBlockStyle::default()
        })
        .unwrap(),
    );
    add(&mut runtime, a, StructuredTextFigure::new("first"));
    let b = add(&mut runtime, outer, BlockFlowFigure::default());
    add(&mut runtime, b, StructuredTextFigure::new("second"));
    let next = add(
        &mut runtime,
        page,
        BlockFlowFigure::new(FlowBlockStyle {
            margin: Insets::new(12.0, 0.0, 0.0, 0.0),
            ..FlowBlockStyle::default()
        })
        .unwrap(),
    );
    add(&mut runtime, next, StructuredTextFigure::new("next"));
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    assert_eq!(s.paragraphs[0].origin.y(), 20.0);
    assert!(
        (s.paragraphs[1].origin.y()
            - s.paragraphs[0].origin.y()
            - f64::from(s.paragraphs[0].layout.layout().height())
            - 5.0)
            .abs()
            < 0.01
    );
    assert!(
        (s.paragraphs[2].origin.y()
            - s.paragraphs[1].origin.y()
            - f64::from(s.paragraphs[1].layout.layout().height())
            - 30.0)
            .abs()
            < 0.01
    );
}

struct ExternalText(String);
struct ExternalPayload(String);
impl FlowContributionPayload for ExternalPayload {}
const EXTERNAL: ContributionKey<ExternalPayload> = ContributionKey::new("consumer.text");
impl Figure for ExternalText {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }
    fn name(&self) -> &'static str {
        "ExternalText"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(FLOW, FlowCapability::of::<Self>())
    }
}
impl FlowContributor for ExternalText {
    fn contribute(
        &self,
        _: &FlowContributionContext<'_>,
        out: &mut FlowContributionSink,
    ) -> Result<(), FlowError> {
        out.append(EXTERNAL, ExternalPayload(self.0.clone()));
        Ok(())
    }
}
#[test]
fn external_contributor_uses_same_registry_source_mapping_and_layout() {
    let mut registry = FlowContributionRegistry::standard();
    registry
        .register(
            EXTERNAL,
            |p: &ExternalPayload, c: &FlowContributionContext<'_>, out: &mut FlowAssembly<'_>| {
                out.text(c.source, &p.0, c.text_style()?)
            },
        )
        .unwrap();
    let (mut runtime, _) = fixture(180.0);
    let page = runtime
        .set_contents(Box::new(
            FlowPageFigure::new(Rectangle::new(0.0, 0.0, 180.0, 200.0)).with_registry(registry),
        ))
        .unwrap();
    let text = add(&mut runtime, page, ExternalText("external consumer".into()));
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    assert_eq!(s.paragraphs[0].layout.request().text(), "external consumer");
    assert_eq!(s.paragraphs[0].sources[0].source, text);
    assert!(s.caret(position(s, text, 3)).is_ok());
}

struct MeasuredBox(Arc<Mutex<Vec<f64>>>);
impl Figure for MeasuredBox {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 90.0, 32.0)
    }
    fn name(&self) -> &'static str {
        "MeasuredBox"
    }
    fn intrinsic_measurement(&self, c: MeasureConstraints) -> FigureMeasurement {
        self.0.lock().unwrap().push(c.max_width().unwrap_or(0.0));
        FigureMeasurement::new(90.0, 32.0, Some(12.0))
    }
}
#[test]
fn adapter_remeasures_at_full_width_and_preserves_child_identity() {
    let (mut runtime, page) = fixture(140.0);
    add(
        &mut runtime,
        page,
        StructuredTextFigure::new("prefix text "),
    );
    let adapter = add(&mut runtime, page, FlowAdapterFigure::new());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let child = add(&mut runtime, adapter, MeasuredBox(calls.clone()));
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    let widths = calls.lock().unwrap();
    assert!(widths.iter().any(|w| *w < 90.0));
    assert!(widths.contains(&140.0));
    let placement = s.placements[&child];
    let p = &s.paragraphs[0];
    let inline = &p.layout.inline_placements()[0];
    let baseline =
        f64::from(p.layout.layout().line_metrics()[inline.line_index].baseline) + p.origin.y();
    assert!((placement.y + placement.height - baseline).abs() < 0.01);
    assert_eq!(
        runtime
            .tree()
            .hit_test_simple((placement.x + 2.0, placement.y + 2.0)),
        Some(child)
    );
}

#[test]
fn rtl_leading_and_physical_left_are_distinct() {
    let (mut runtime, page) = fixture(180.0);
    for alignment in [ParagraphAlignment::Leading, ParagraphAlignment::Left] {
        let block = add(
            &mut runtime,
            page,
            BlockFlowFigure::new(FlowBlockStyle {
                direction: ParagraphDirection::RightToLeft,
                alignment,
                ..FlowBlockStyle::default()
            })
            .unwrap(),
        );
        add(&mut runtime, block, StructuredTextFigure::new("abc"));
    }
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    assert!(
        s.paragraphs[0].layout.layout().glyph_runs()[0].glyphs[0].x
            > s.paragraphs[1].layout.layout().glyph_runs()[0].glyphs[0].x + 100.0
    );
}

struct ExternalBorder {
    painted: Arc<Mutex<Vec<FlowBorderFragment>>>,
    invalid: Arc<AtomicBool>,
}
impl FlowFragmentBorder for ExternalBorder {
    fn metrics(
        &self,
        c: &mut novadraw::text::MeasureContext<'_>,
        style: &novadraw::text::TextStyle,
    ) -> Result<FlowBorderMetrics, FlowError> {
        assert_eq!(c.font(), style.font());
        Ok(FlowBorderMetrics {
            leading_width: if self.invalid.load(Ordering::Relaxed) {
                f64::NAN
            } else {
                10.0
            },
            trailing_width: 12.0,
            height: f64::from(style.font().size()),
        })
    }
    fn paint_fragment(
        &self,
        f: &FlowBorderFragment,
        c: &mut novadraw::graphics::PaintContext<'_>,
    ) -> Result<(), novadraw::graphics::GraphicsError> {
        self.painted.lock().unwrap().push(f.clone());
        c.set_stroke_paint(Color::RED);
        c.stroke_rect(f.bounds)
    }
}
#[test]
fn external_fragment_border_reserves_first_last_boxes_and_keeps_old_snapshot_on_failure() {
    let (mut runtime, page) = fixture(130.0);
    let painted = Arc::new(Mutex::new(Vec::new()));
    let invalid = Arc::new(AtomicBool::new(false));
    let group = add(
        &mut runtime,
        page,
        InlineFlowFigure::new().with_fragment_border(ExternalBorder {
            painted: painted.clone(),
            invalid: invalid.clone(),
        }),
    );
    add(
        &mut runtime,
        group,
        StructuredTextFigure::new("one two three four five six seven eight"),
    );
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page).clone();
    let borders = s.border_fragments().collect::<Vec<_>>();
    assert!(borders.len() > 1);
    assert_eq!(borders.iter().filter(|b| b.leading).count(), 1);
    assert_eq!(borders.iter().filter(|b| b.trailing).count(), 1);
    assert_eq!(borders.first().unwrap().leading_box.unwrap().width, 10.0);
    assert_eq!(borders.last().unwrap().trailing_box.unwrap().width, 12.0);
    assert_eq!(painted.lock().unwrap().len(), borders.len());
    let old_bounds = runtime.tree().node(group).unwrap().bounds();
    invalid.store(true, Ordering::Relaxed);
    runtime
        .figure(group)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(Color::RED),
            ..FigureStyle::default()
        })
        .unwrap();
    runtime.frames().request_full_redraw();
    assert!(support::prepare_next_submission(&mut runtime).is_none());
    assert_eq!(runtime.tree().node(group).unwrap().bounds(), old_bounds);
    assert_eq!(snapshot(&runtime, page).revision, s.revision);
}

#[test]
fn completed_font_resource_invalidates_source_caret_revision() {
    let (mut runtime, page) = fixture(180.0);
    let text = add(
        &mut runtime,
        page,
        StructuredTextFigure::new("font revision"),
    );
    record_full_submission(&mut runtime);
    let old = position(snapshot(&runtime, page), text, 1);
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::NotoSansSc)
        .unwrap();
    record_full_submission(&mut runtime);
    assert_eq!(
        snapshot(&runtime, page).caret(old),
        Err(TextInteractionError::StaleTextLayoutRevision)
    );
}

#[test]
fn source_moved_out_of_flow_releases_the_old_presentation_owner() {
    let (mut runtime, _) = fixture(180.0);
    let root = runtime
        .set_contents(Box::new(novadraw::figure::RectangleFigure::new(
            0.0, 0.0, 300.0, 300.0,
        )))
        .unwrap();
    let page = add(
        &mut runtime,
        root,
        FlowPageFigure::new(Rectangle::new(0.0, 0.0, 180.0, 100.0)),
    );
    let text = add(
        &mut runtime,
        page,
        StructuredTextFigure::new("move outside flow"),
    );
    record_full_submission(&mut runtime);
    let old = position(snapshot(&runtime, page), text, 1);
    runtime.figure(text).unwrap().reparent(root).unwrap();
    record_full_submission(&mut runtime);
    assert!(
        runtime
            .tree()
            .prepared_presentation(text)
            .unwrap()
            .is_none()
    );
    assert!(!snapshot(&runtime, page).placements.contains_key(&text));
    assert_eq!(
        snapshot(&runtime, page).caret(old),
        Err(TextInteractionError::StaleTextLayoutRevision)
    );
}

#[test]
fn nested_ordinary_borders_reserve_each_inset_once_and_keep_coordinates() {
    use novadraw::figure::border::LineBorder;
    let (mut runtime, _) = fixture(200.0);
    let page = runtime
        .set_contents(Box::new(
            FlowPageFigure::new(Rectangle::new(0.0, 0.0, 200.0, 300.0))
                .with_border(LineBorder::new(Color::BLACK, 10.0)),
        ))
        .unwrap();
    let outer = add(
        &mut runtime,
        page,
        BlockFlowFigure::default().with_border(LineBorder::new(Color::RED, 3.0)),
    );
    let inner = add(
        &mut runtime,
        outer,
        BlockFlowFigure::default().with_border(LineBorder::new(Color::BLUE, 5.0)),
    );
    let text = add(
        &mut runtime,
        inner,
        StructuredTextFigure::new("border contents"),
    );
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    let p = &s.paragraphs[0];
    assert_eq!(p.origin, Point::new(18.0, 18.0));
    assert_eq!(s.placements[&outer].x, 10.0);
    assert_eq!(s.placements[&outer].y, 10.0);
    assert_eq!(s.placements[&outer].width, 180.0);
    assert_eq!(s.placements[&inner].x, 13.0);
    assert_eq!(s.placements[&inner].y, 13.0);
    assert_eq!(s.placements[&inner].width, 174.0);
    assert_eq!(
        runtime
            .tree()
            .preferred_measurement(page, MeasureConstraints::UNBOUNDED)
            .unwrap()
            .width,
        200.0
    );
    for source in [outer, inner] {
        let measured = runtime
            .tree()
            .preferred_measurement(source, MeasureConstraints::UNBOUNDED)
            .unwrap();
        assert_eq!(measured.width, s.placements[&source].width);
        assert!((measured.height - s.placements[&source].height).abs() < 0.01);
    }
    assert!(
        (s.placements[&outer].height - f64::from(p.layout.layout().height()) - 16.0).abs() < 0.01
    );
    assert_eq!(runtime.tree().node(inner).unwrap().bounds().x, 0.0);
    let fragment = s.fragments.iter().find(|f| f.source == text).unwrap();
    assert_eq!(
        runtime
            .tree()
            .hit_test_simple((fragment.bounds.x + 1.0, fragment.bounds.y + 1.0)),
        Some(text)
    );
    let old_revision = s.revision;
    runtime
        .figure(inner)
        .unwrap()
        .update_component(SetFlowBlockStyle(FlowBlockStyle {
            padding: Insets::uniform(2.0),
            ..FlowBlockStyle::default()
        }))
        .unwrap();
    record_full_submission(&mut runtime);
    assert_ne!(snapshot(&runtime, page).revision, old_revision);
    assert_eq!(
        snapshot(&runtime, page).paragraphs[0].origin,
        Point::new(20.0, 20.0)
    );
}

#[test]
fn fragment_border_spanning_blocks_reserves_only_global_first_and_last() {
    let (mut runtime, page) = fixture(150.0);
    let painted = Arc::new(Mutex::new(Vec::new()));
    let group = add(
        &mut runtime,
        page,
        InlineFlowFigure::new().with_fragment_border(ExternalBorder {
            painted,
            invalid: Arc::new(AtomicBool::new(false)),
        }),
    );
    for text in ["first paragraph", "second paragraph"] {
        let block = add(&mut runtime, group, BlockFlowFigure::default());
        add(&mut runtime, block, StructuredTextFigure::new(text));
    }
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    let borders = s.border_fragments().collect::<Vec<_>>();
    assert_eq!(borders.iter().filter(|f| f.leading).count(), 1);
    assert_eq!(borders.iter().filter(|f| f.trailing).count(), 1);
    assert_eq!(
        borders.iter().filter(|f| f.leading_box.is_some()).count(),
        1
    );
    assert_eq!(
        borders.iter().filter(|f| f.trailing_box.is_some()).count(),
        1
    );
}

#[test]
fn box_only_fragment_border_keeps_adapter_and_first_last_markers() {
    let (mut runtime, page) = fixture(150.0);
    let group = add(
        &mut runtime,
        page,
        InlineFlowFigure::new().with_fragment_border(ExternalBorder {
            painted: Arc::new(Mutex::new(Vec::new())),
            invalid: Arc::new(AtomicBool::new(false)),
        }),
    );
    let adapter = add(&mut runtime, group, FlowAdapterFigure::new());
    let child = add(
        &mut runtime,
        adapter,
        MeasuredBox(Arc::new(Mutex::new(Vec::new()))),
    );
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    let border = s.border_fragments().next().unwrap();
    assert!(border.leading && border.trailing);
    assert_eq!(border.leading_box.unwrap().width, 10.0);
    assert_eq!(border.trailing_box.unwrap().width, 12.0);
    assert_eq!(s.placements[&child].x, 10.0);
}

struct ExternalPage(FlowLayoutService);
impl Figure for ExternalPage {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 180.0, 200.0)
    }
    fn name(&self) -> &'static str {
        "ExternalPage"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        use novadraw::figure::*;
        out.register(FLOW, FlowCapability::of::<Self>())?;
        out.register(CONTAINER, ContainerCapability::of::<Self>())?;
        out.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl novadraw::figure::FigureContainer for ExternalPage {}
impl FlowContributor for ExternalPage {
    fn contribute(
        &self,
        _: &FlowContributionContext<'_>,
        out: &mut FlowContributionSink,
    ) -> Result<(), FlowError> {
        out.append(GROUP_CONTRIBUTION, GroupContribution);
        Ok(())
    }
}
impl novadraw::figure::FigurePreparation for ExternalPage {
    fn prepare(
        &self,
        _: &mut novadraw::text::MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<novadraw::figure::FigurePresentation, novadraw::text::TextError> {
        Err(novadraw::text::TextError::InvalidPreparedScene(
            "subtree required".into(),
        ))
    }
    fn prepare_scene(
        &self,
        c: &mut novadraw::text::MeasureContext<'_>,
        b: Rectangle,
        scene: &novadraw::figure::PreparationScene<'_>,
    ) -> Result<novadraw::figure::PreparedScene, novadraw::text::TextError> {
        let first = self
            .0
            .prepare(c, b, scene)
            .map_err(|e| novadraw::text::TextError::InvalidPreparedScene(e.to_string()))?;
        let second = self
            .0
            .prepare(c, b, scene)
            .map_err(|e| novadraw::text::TextError::InvalidPreparedScene(e.to_string()))?;
        assert!(std::ptr::eq(
            first.root_presentation().data::<FlowSnapshot>().unwrap(),
            second.root_presentation().data::<FlowSnapshot>().unwrap(),
        ));
        let root = first.root_presentation().clone();
        Ok(first.with_root_presentation(root))
    }
}
#[test]
fn external_page_uses_public_service_and_survives_source_removal() {
    let (mut runtime, _) = fixture(180.0);
    let page = runtime
        .set_contents(Box::new(ExternalPage(FlowLayoutService::default())))
        .unwrap();
    let text = add(
        &mut runtime,
        page,
        StructuredTextFigure::new("external page"),
    );
    record_full_submission(&mut runtime);
    let old = position(snapshot(&runtime, page), text, 1);
    assert!(snapshot(&runtime, page).caret(old).is_ok());
    runtime.figure(page).unwrap().remove(text).unwrap();
    record_full_submission(&mut runtime);
    assert!(!snapshot(&runtime, page).placements.contains_key(&text));
    assert_eq!(
        snapshot(&runtime, page).caret(old),
        Err(TextInteractionError::StaleTextLayoutRevision)
    );
}

#[test]
fn owner_measured_block_border_is_added_once_to_preferred_measurement() {
    use novadraw::figure::border::TitleBarBorder;
    let (mut runtime, page) = fixture(200.0);
    let block = add(
        &mut runtime,
        page,
        BlockFlowFigure::default().with_border(TitleBarBorder::new("Title", Color::BLUE)),
    );
    add(&mut runtime, block, StructuredTextFigure::new("body"));
    record_full_submission(&mut runtime);
    let s = snapshot(&runtime, page);
    let measured = runtime
        .tree()
        .preferred_measurement(block, MeasureConstraints::UNBOUNDED)
        .unwrap();
    assert_eq!(measured.width, s.placements[&block].width);
    assert!((measured.height - s.placements[&block].height).abs() < 0.01);
    assert_eq!(
        s.paragraphs[0].origin.y(),
        runtime.tree().node(block).unwrap().insets().top
    );
}
