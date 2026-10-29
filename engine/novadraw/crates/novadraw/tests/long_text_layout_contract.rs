use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use novadraw::{
    figure::{FigureStyle, flow::*},
    geometry::Rectangle,
    render::ResourceId,
    runtime::Runtime,
    text::{
        BuiltinFont, FontDescriptor, StyledTextLayout, StyledTextRequest, TextConstraints,
        TextError, TextLayout,
        shaping::{ParleyTextEngine, TextLayoutCacheIdentity, TextLayoutEngine},
    },
};

struct CountingEngine {
    inner: ParleyTextEngine,
    calls: Arc<AtomicUsize>,
    cacheable: bool,
}

struct ChangingOutline {
    revision: Arc<AtomicU64>,
    fail: Arc<AtomicBool>,
}
impl novadraw::text::outline::GlyphOutlineProvider for ChangingOutline {
    fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }
    fn outline(
        &mut self,
        _: novadraw::text::outline::FontInstanceRef<'_>,
        _: u32,
    ) -> Result<novadraw::text::outline::GlyphOutline, novadraw::text::outline::FontError> {
        if self.fail.load(Ordering::Relaxed) {
            return Err(novadraw::text::outline::FontError::InvalidFont);
        }
        let size = self.revision() as f64 * 1000.0;
        let mut path = novadraw::graphics::Path::new();
        path.move_to(-size, 0.0);
        path.line_to(size, 0.0);
        path.line_to(size, size);
        path.close();
        novadraw::text::outline::GlyphOutline::new(path, 1000)
    }
}

#[test]
fn external_outline_revision_replaces_ink_atomically_and_failure_retains_old_snapshot() {
    let revision = Arc::new(AtomicU64::new(1));
    let fail = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut runtime = Runtime::with_text_layout_engine(
        novadraw::tree::FigureTree::new(),
        Box::new(novadraw::text::outline::OutlineTextEngine::new(
            Box::new(CountingEngine {
                inner: ParleyTextEngine::new(),
                calls: calls.clone(),
                cacheable: true,
            }),
            Box::new(ChangingOutline {
                revision: revision.clone(),
                fail: fail.clone(),
            }),
        )),
    );
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let page = runtime
        .set_contents(Box::new(FlowPageFigure::new(Rectangle::new(
            0.0, 0.0, 150.0, 400.0,
        ))))
        .unwrap();
    let source = runtime
        .figure(page)
        .unwrap()
        .add(Box::new(StructuredTextFigure::new("unchanged shaping")))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let first = snapshot(&runtime, page);
    let first_ink = first.paragraphs[0].layout.layout().ink_bounds().unwrap();
    let first_visual = runtime
        .tree()
        .prepared_presentation(page)
        .unwrap()
        .unwrap()
        .visual_bounds();
    let shaping_calls = calls.load(Ordering::Relaxed);
    revision.store(2, Ordering::Relaxed);
    runtime.frames().stabilize_for_query().unwrap();
    let second = snapshot(&runtime, page);
    let second_visual = runtime
        .tree()
        .prepared_presentation(page)
        .unwrap()
        .unwrap()
        .visual_bounds();
    assert_ne!(first_visual, second_visual);
    assert!(second_visual.x <= second.paragraphs[0].layout.layout().ink_bounds().unwrap().x);
    assert_eq!(calls.load(Ordering::Relaxed), shaping_calls);
    assert_eq!(
        first.paragraphs[0].layout.layout().metrics(),
        second.paragraphs[0].layout.layout().metrics()
    );
    assert_ne!(
        first_ink,
        second.paragraphs[0].layout.layout().ink_bounds().unwrap()
    );
    assert_eq!(
        first.paragraphs[0].layout.layout().ink_bounds().unwrap(),
        first_ink
    );
    let second_ink = second.paragraphs[0].layout.layout().ink_bounds();
    runtime
        .figure(source)
        .unwrap()
        .update_component(SetStructuredFlowText(
            "source committed before outline failure".into(),
        ))
        .unwrap();
    revision.store(3, Ordering::Relaxed);
    fail.store(true, Ordering::Relaxed);
    assert!(runtime.frames().stabilize_for_query().is_err());
    let retained = snapshot(&runtime, page);
    assert!(Arc::ptr_eq(&second, &retained));
    assert_eq!(
        retained.paragraphs[0].layout.layout().ink_bounds(),
        second_ink
    );
    fail.store(false, Ordering::Relaxed);
    revision.store(4, Ordering::Relaxed);
    runtime.frames().stabilize_for_query().unwrap();
    let recovered = snapshot(&runtime, page);
    assert_eq!(
        recovered.paragraphs[0].layout.request().text(),
        "source committed before outline failure"
    );
    assert_ne!(
        recovered.paragraphs[0].layout.layout().ink_bounds(),
        second_ink
    );
}

impl TextLayoutEngine for CountingEngine {
    fn revision(&self) -> u64 {
        self.inner.revision()
    }
    fn cache_identity(&self) -> Option<TextLayoutCacheIdentity> {
        self.cacheable.then(|| self.inner.cache_identity().unwrap())
    }
    fn register_font(
        &mut self,
        id: ResourceId,
        revision: u64,
        bytes: &[u8],
    ) -> Result<(), TextError> {
        self.inner.register_font(id, revision, bytes)
    }
    fn remove_font(&mut self, id: ResourceId) {
        self.inner.remove_font(id);
    }
    fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        self.inner.layout(text, font, constraints)
    }
    fn layout_styled(
        &mut self,
        request: &StyledTextRequest,
    ) -> Result<StyledTextLayout, TextError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.layout_styled(request)
    }
}

fn fixture(
    cacheable: bool,
) -> (
    Runtime,
    novadraw::tree::FigureId,
    novadraw::tree::FigureId,
    Arc<AtomicUsize>,
) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut runtime = Runtime::with_text_layout_engine(
        novadraw::tree::FigureTree::new(),
        Box::new(CountingEngine {
            inner: ParleyTextEngine::new(),
            calls: calls.clone(),
            cacheable,
        }),
    );
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let page = runtime
        .set_contents(Box::new(
            FlowPageFigure::new(Rectangle::new(0.0, 0.0, 150.0, 400.0)).with_style(FigureStyle {
                font: Some(novadraw::text::FontDescriptor::parse("16px Inter").unwrap()),
                ..FigureStyle::default()
            }),
        ))
        .unwrap();
    let mut first = None;
    for text in [
        "first paragraph wrapping across lines",
        "second paragraph",
        "third paragraph",
    ] {
        let block = runtime
            .figure(page)
            .unwrap()
            .add(Box::new(BlockFlowFigure::default()))
            .unwrap();
        let source = runtime
            .figure(block)
            .unwrap()
            .add(Box::new(StructuredTextFigure::new(text)))
            .unwrap();
        first.get_or_insert(source);
    }
    (runtime, page, first.unwrap(), calls)
}

fn snapshot(runtime: &Runtime, page: novadraw::tree::FigureId) -> Arc<FlowSnapshot> {
    runtime
        .tree()
        .prepared_presentation(page)
        .unwrap()
        .unwrap()
        .shared_data()
        .unwrap()
}

#[test]
fn external_runtime_timing_preserves_exact_layout_and_drains_failed_attempts() {
    let (mut plain, plain_page, _, plain_calls) = fixture(true);
    let (mut timed, timed_page, _, timed_calls) = fixture(true);
    timed.frames().set_timing_enabled(true);
    plain.frames().stabilize_for_query().unwrap();
    timed.frames().stabilize_for_query().unwrap();
    assert!(plain.frames().take_timings().is_empty());
    assert!(
        plain
            .frames()
            .take_flow_layout_observations()
            .records
            .is_empty()
    );
    let left = snapshot(&plain, plain_page);
    let right = snapshot(&timed, timed_page);
    assert_eq!(left.size, right.size);
    assert_eq!(
        plain_calls.load(Ordering::Relaxed),
        timed_calls.load(Ordering::Relaxed)
    );
    for (a, b) in left.paragraphs.iter().zip(&right.paragraphs) {
        assert_eq!(a.origin, b.origin);
        assert_eq!(a.layout.request(), b.layout.request());
        assert_eq!(
            a.layout.layout().line_metrics(),
            b.layout.layout().line_metrics()
        );
        for (a, b) in a
            .layout
            .layout()
            .glyph_runs()
            .iter()
            .zip(b.layout.layout().glyph_runs())
        {
            assert_eq!(a.glyphs, b.glyphs);
        }
    }
    let timings = timed.frames().take_timings();
    let observations = timed.frames().take_flow_layout_observations();
    assert_eq!(observations.dropped, 0);
    assert_eq!(
        observations.records.iter().filter(|r| !r.cache_hit).count(),
        timed_calls.load(Ordering::Relaxed)
    );
    assert!(
        observations
            .records
            .iter()
            .all(|r| r.succeeded && r.engine_identity.is_some())
    );
    for paragraph in &right.paragraphs {
        assert!(
            observations
                .records
                .iter()
                .any(|r| *r.request == *paragraph.layout.request())
        );
    }
    assert!(
        timed
            .frames()
            .take_flow_layout_observations()
            .records
            .is_empty()
    );
    let total = timings
        .iter()
        .find(|entry| entry.stage == "stable-query")
        .unwrap();
    assert_eq!(total.calls, 1);
    assert_eq!(
        total.inclusive_ns,
        timings.iter().map(|entry| entry.exclusive_ns).sum::<u128>()
    );
    assert!(timed.frames().take_timings().is_empty());
    timed
        .frames()
        .set_preparation_budget(novadraw::figure::PreparationBudget::new(0, 0));
    assert!(timed.frames().stabilize_for_query().is_err());
    assert!(Arc::ptr_eq(&right, &snapshot(&timed, timed_page)));
    assert!(
        timed
            .frames()
            .take_timings()
            .iter()
            .any(|entry| entry.stage == "stabilization")
    );
    timed.frames().set_timing_enabled(false);
    assert!(timed.frames().stabilize_for_query().is_err());
    assert!(timed.frames().take_timings().is_empty());
}

#[test]
fn external_engine_reuses_unchanged_paragraphs_and_retains_old_geometry() {
    let (mut runtime, page, first, calls) = fixture(true);
    runtime.frames().stabilize_for_query().unwrap();
    let old = snapshot(&runtime, page);
    let before_calls = calls.load(Ordering::Relaxed);
    runtime
        .figure(first)
        .unwrap()
        .update_component(SetStructuredFlowText(
            "first paragraph wrapping across lines plus an appended word".into(),
        ))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let new = snapshot(&runtime, page);
    assert_eq!(calls.load(Ordering::Relaxed) - before_calls, 1);
    assert_ne!(old.revision, new.revision);
    assert_eq!(
        old.paragraphs[0].layout.request().text(),
        "first paragraph wrapping across lines"
    );
    for index in 1..3 {
        assert!(std::ptr::eq(
            old.paragraphs[index].layout.layout().glyph_runs().as_ptr(),
            new.paragraphs[index].layout.layout().glyph_runs().as_ptr(),
        ));
        assert!(std::ptr::eq(
            old.paragraphs[index]
                .layout
                .layout()
                .line_metrics()
                .as_ptr(),
            new.paragraphs[index]
                .layout
                .layout()
                .line_metrics()
                .as_ptr(),
        ));
    }
    assert!(new.paragraphs[1].origin.y() > old.paragraphs[1].origin.y());
    assert!(!Arc::ptr_eq(&old, &new));
    assert!(Arc::ptr_eq(&new, &snapshot(&runtime, page)));
}

#[test]
fn engine_without_cache_contract_recomputes_each_paragraph() {
    let (mut runtime, page, first, calls) = fixture(false);
    runtime.frames().stabilize_for_query().unwrap();
    let before_calls = calls.load(Ordering::Relaxed);
    runtime
        .figure(first)
        .unwrap()
        .update_component(SetStructuredFlowText("changed".into()))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert!(calls.load(Ordering::Relaxed) - before_calls >= 3);
    assert_eq!(snapshot(&runtime, page).statistics.paragraph_hits, 0);
}

#[test]
fn exact_width_change_reflows_all_paragraphs_and_font_revision_invalidates_cache() {
    let (mut runtime, page, _, calls) = fixture(true);
    runtime.frames().stabilize_for_query().unwrap();
    let old = snapshot(&runtime, page);
    let before_calls = calls.load(Ordering::Relaxed);
    runtime
        .figure(page)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 80.0, 400.0))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed) - before_calls, 3);
    let narrow = snapshot(&runtime, page);
    assert!(narrow.size.height > old.size.height);
    let before_calls = calls.load(Ordering::Relaxed);
    let font = runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    runtime
        .resources()
        .complete_font(
            font,
            novadraw::render::FontData::new(BuiltinFont::Inter.bytes().to_vec()),
        )
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed) - before_calls, 3);
}

#[test]
fn instance_caches_are_isolated_and_eviction_preserves_published_snapshots() {
    let (mut first_runtime, page, source, first_calls) = fixture(true);
    let (mut second_runtime, second_page, _, second_calls) = fixture(true);
    first_runtime.frames().stabilize_for_query().unwrap();
    let retained = snapshot(&first_runtime, page);
    let glyph_lease = retained.paragraphs[0].layout.layout().glyph_runs()[0]
        .glyphs
        .clone();
    let old_glyphs = glyph_lease.to_vec();
    second_runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(second_calls.load(Ordering::Relaxed), 3);
    let other = snapshot(&second_runtime, second_page);
    assert!(!std::ptr::eq(
        retained.paragraphs[0].layout.layout().glyph_runs().as_ptr(),
        other.paragraphs[0].layout.layout().glyph_runs().as_ptr(),
    ));
    for value in 0..257 {
        first_runtime
            .figure(source)
            .unwrap()
            .update_component(SetStructuredFlowText(format!("unique paragraph {value}")))
            .unwrap();
        first_runtime.frames().stabilize_for_query().unwrap();
    }
    let calls = first_calls.load(Ordering::Relaxed);
    first_runtime
        .figure(source)
        .unwrap()
        .update_component(SetStructuredFlowText(
            "first paragraph wrapping across lines".into(),
        ))
        .unwrap();
    first_runtime.frames().stabilize_for_query().unwrap();
    assert_eq!(first_calls.load(Ordering::Relaxed) - calls, 1);
    assert_eq!(
        retained.paragraphs[0].layout.request().text(),
        "first paragraph wrapping across lines"
    );
    assert_eq!(second_calls.load(Ordering::Relaxed), 3);
    assert_eq!(glyph_lease.as_slice(), old_glyphs);
    assert_eq!(
        glyph_lease.buffer().as_slice().as_ptr(),
        retained.paragraphs[0].layout.layout().glyph_runs()[0]
            .glyphs
            .buffer()
            .as_slice()
            .as_ptr(),
    );
    drop(first_runtime);
    drop(retained);
    assert_eq!(glyph_lease.as_slice(), old_glyphs);
}

#[test]
fn external_backend_retains_runtime_submission_after_source_retirement() {
    use novadraw::render::*;
    #[derive(Default)]
    struct RetainingBackend(Option<RenderSubmission>);
    impl RenderBackend for RetainingBackend {
        fn submit(&mut self, submission: &RenderSubmission) -> RenderResult {
            self.0 = Some(submission.clone());
            Ok(RenderDisposition::Presented)
        }
        fn resize(&mut self, _: u32, _: u32, _: f64) {}
    }
    let (mut runtime, page, _, _) = fixture(true);
    runtime.frames().stabilize_for_query().unwrap();
    let source = snapshot(&runtime, page);
    let frame = runtime
        .frames()
        .prepare_submission(
            SurfaceInfo {
                logical_width: 180.0,
                logical_height: 400.0,
                pixel_width: 180,
                pixel_height: 400,
                scale_factor: 1.0,
            },
            BackendCapabilities::RETAINED_PARTIAL,
        )
        .into_ready()
        .unwrap();
    let mut backend = RetainingBackend::default();
    let result = backend.submit(&frame);
    assert_eq!(
        runtime
            .frames()
            .complete_submission(frame.session_id, frame.frame_id, result)
            .unwrap(),
        novadraw::runtime::SubmissionAcknowledgement::Presented,
    );
    let runs: Vec<_> = backend
        .0
        .as_ref()
        .unwrap()
        .commands
        .iter()
        .filter_map(|command| {
            if let RenderCommandKind::DrawGlyphRun { run, .. } = &command.kind {
                Some(run)
            } else {
                None
            }
        })
        .collect();
    assert!(!runs.is_empty());
    for run in &runs {
        assert!(
            source
                .paragraphs
                .iter()
                .flat_map(|p| p.layout.layout().glyph_runs())
                .any(|original| {
                    original.glyphs.buffer().as_slice().as_ptr()
                        == run.glyphs.buffer().as_slice().as_ptr()
                        && Arc::ptr_eq(&original.normalized_coords, &run.normalized_coords)
                })
        );
    }
    let retained_glyphs: Vec<_> = runs.iter().map(|r| r.glyphs.to_vec()).collect();
    runtime
        .set_contents(Box::new(novadraw::figure::RectangleFigure::new(
            0.0, 0.0, 10.0, 10.0,
        )))
        .unwrap();
    assert!(runtime.tree().node(page).is_err());
    drop(runtime);
    drop(source);
    drop(frame);
    for (run, glyphs) in runs.iter().zip(retained_glyphs) {
        assert_eq!(run.glyphs.as_slice(), glyphs);
    }
    assert!(matches!(
        backend.0.as_ref().unwrap().resources,
        ResourceSync::Snapshot(_)
    ));
}
