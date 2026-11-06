use crate::{DocumentVersion, RichTextDocumentSnapshot, RichTextLayoutSnapshot};
use novadraw::{
    figure::{LayoutDirection, PreparationBudgetHandle},
    text::{
        MeasureContext, TextConstraints, TextError, TextLayoutDiagnostics,
        shaping::TextLayoutCacheIdentity,
    },
};
use std::sync::{Arc, Mutex};

/// Adjacent measurement and publication widths; never an unbounded width history.
const SNAPSHOT_CAPACITY: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    document: DocumentVersion,
    engine: TextLayoutCacheIdentity,
    generation: u64,
    width: Option<u32>,
    direction: LayoutDirection,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParagraphInput, RichTextDocumentBuilder};
    use novadraw::{
        figure::PreparationBudget,
        graphics::Color,
        render::ResourceId,
        text::{
            BuiltinFont, FontDescriptor, StyledTextLayout, StyledTextRequest, TextLayout,
            TextSystem,
            shaping::{ParleyTextEngine, TextLayoutEngine},
        },
    };
    use std::{cell::Cell, rc::Rc};

    struct CountingEngine {
        inner: ParleyTextEngine,
        calls: Rc<Cell<usize>>,
        cacheable: bool,
    }
    impl TextLayoutEngine for CountingEngine {
        fn revision(&self) -> u64 {
            self.inner.revision()
        }
        fn cache_identity(&self) -> Option<TextLayoutCacheIdentity> {
            self.cacheable.then(|| self.inner.cache_identity().unwrap())
        }
        fn layout_styled(
            &mut self,
            request: &StyledTextRequest,
        ) -> Result<StyledTextLayout, TextError> {
            self.calls.set(self.calls.get() + 1);
            self.inner.layout_styled(request)
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
    }
    fn system(cacheable: bool) -> (TextSystem, Rc<Cell<usize>>) {
        let calls = Rc::new(Cell::new(0));
        let mut system = TextSystem::with_engine(Box::new(CountingEngine {
            inner: ParleyTextEngine::new(),
            calls: calls.clone(),
            cacheable,
        }));
        system.register_builtin_font(BuiltinFont::Inter).unwrap();
        (system, calls)
    }
    fn document() -> RichTextDocumentSnapshot {
        let mut builder = RichTextDocumentBuilder::default();
        builder
            .push(ParagraphInput::new(
                "alpha beta gamma delta epsilon",
                novadraw::text::TextStyle::new(FontDescriptor::default(), Color::BLACK),
            ))
            .unwrap();
        builder.build()
    }
    fn three_paragraphs() -> RichTextDocumentSnapshot {
        let mut builder = RichTextDocumentBuilder::default();
        for text in [
            "first paragraph",
            "middle paragraph with several words",
            "tail 🙂 paragraph",
        ] {
            builder
                .push(ParagraphInput::new(
                    text,
                    novadraw::text::TextStyle::new(FontDescriptor::default(), Color::BLACK),
                ))
                .unwrap();
        }
        builder.build()
    }

    #[test]
    fn paragraph_edits_reuse_unchanged_layouts_and_match_full_reflow() {
        use novadraw::text::{TextAffinity, TextPosition};
        let (mut system, calls) = system(true);
        let cache = SnapshotCache::default();
        let source = three_paragraphs();
        let direction = LayoutDirection::LeftToRight;
        let initial = prepare(&cache, &mut system, &source, 120.0, direction);
        assert_eq!(calls.get(), 3);
        let middle = source.paragraphs()[1].id();
        let tail = source.paragraphs()[2].id();
        let old_position = initial
            .position(tail, TextPosition::new(0, TextAffinity::Downstream))
            .unwrap();
        let mut document = source.clone();
        let mut candidate_cache = cache.fork();
        for (range, text) in [(0..0, "extra words extra words "), (0..5, "x"), (0..1, "")] {
            document = document
                .edit_plain_paragraph(document.version(), middle, range, text)
                .unwrap();
            let before = calls.get();
            let incremental = prepare(&candidate_cache, &mut system, &document, 120.0, direction);
            assert_eq!(calls.get() - before, 1);
            assert!(incremental.shares_paragraph_layout(0, &initial));
            assert!(incremental.shares_paragraph_layout(2, &initial));
            let full = prepare(
                &SnapshotCache::default(),
                &mut system,
                &document,
                120.0,
                direction,
            );
            assert_eq!(incremental.size(), full.size());
            for i in 0..=3 {
                assert_eq!(
                    incremental.height_index().offset(i),
                    full.height_index().offset(i)
                );
            }
            let position = incremental
                .position(tail, TextPosition::new(0, TextAffinity::Downstream))
                .unwrap();
            let reference = full
                .position(tail, TextPosition::new(0, TextAffinity::Downstream))
                .unwrap();
            assert_eq!(
                incremental.caret_bounds(&position).unwrap(),
                full.caret_bounds(&reference).unwrap()
            );
            let first = document.paragraphs()[0].id();
            let anchor = incremental
                .position(first, TextPosition::new(0, TextAffinity::Downstream))
                .unwrap();
            let reference_anchor = full
                .position(first, TextPosition::new(0, TextAffinity::Downstream))
                .unwrap();
            let actual: Vec<_> = incremental
                .selection_geometry(&anchor, &position)
                .unwrap()
                .into_iter()
                .map(|q| q.bounds())
                .collect();
            let expected: Vec<_> = full
                .selection_geometry(&reference_anchor, &reference)
                .unwrap()
                .into_iter()
                .map(|q| q.bounds())
                .collect();
            assert_eq!(actual, expected);
            assert!(incremental.caret_bounds(&old_position).is_err());
            candidate_cache = candidate_cache.fork();
        }
        // Abandoning a draft/composition preserves the source's exact publication.
        assert!(Arc::ptr_eq(
            &initial,
            &prepare(&cache, &mut system, &source, 120.0, direction)
        ));
        assert!(initial.caret_bounds(&old_position).is_ok());
    }

    #[test]
    fn paragraph_style_metrics_and_environment_invalidate_reuse() {
        let (mut system, calls) = system(true);
        let cache = SnapshotCache::default();
        let document = three_paragraphs();
        let direction = LayoutDirection::LeftToRight;
        prepare(&cache, &mut system, &document, 120.0, direction);
        let mut input = document.paragraphs()[1].input();
        input.space_after = 17.0;
        input.alignment = novadraw::text::ParagraphAlignment::Center;
        let changed = document
            .replace_paragraph(document.version(), document.paragraphs()[1].id(), input)
            .unwrap();
        let candidate = cache.fork();
        let before = calls.get();
        prepare(&candidate, &mut system, &changed, 120.0, direction);
        assert_eq!(calls.get() - before, 1);
        let before = calls.get();
        prepare(&candidate, &mut system, &changed, 80.0, direction);
        assert_eq!(calls.get() - before, 3);
        let before = calls.get();
        prepare(
            &candidate,
            &mut system,
            &changed,
            80.0,
            LayoutDirection::RightToLeft,
        );
        assert_eq!(calls.get() - before, 3);
        system
            .register_builtin_font(BuiltinFont::JetBrainsMono)
            .unwrap();
        let before = calls.get();
        prepare(&candidate, &mut system, &changed, 80.0, direction);
        assert_eq!(calls.get() - before, 3);
    }
    fn prepare(
        cache: &SnapshotCache,
        system: &mut TextSystem,
        document: &RichTextDocumentSnapshot,
        width: f64,
        direction: LayoutDirection,
    ) -> Arc<RichTextLayoutSnapshot> {
        cache
            .prepare(
                document,
                &mut system.measure(FontDescriptor::default()),
                Some(width),
                PreparationBudgetHandle::new(PreparationBudget::default()),
                direction,
            )
            .unwrap()
    }

    #[test]
    fn repeated_constraints_reuse_but_width_direction_and_document_changes_recompute() {
        let (mut system, calls) = system(true);
        let cache = SnapshotCache::default();
        let document = document();
        let direction = LayoutDirection::LeftToRight;
        let wide = prepare(&cache, &mut system, &document, 200.0, direction);
        let repeated = prepare(&cache, &mut system, &document, 200.0, direction);
        assert!(Arc::ptr_eq(&wide, &repeated));
        assert_eq!(calls.get(), 1);
        let narrow = prepare(&cache, &mut system, &document, 70.0, direction);
        assert!(narrow.size().height > wide.size().height);
        assert!(!Arc::ptr_eq(&wide, &narrow));
        prepare(&cache, &mut system, &document, 200.0, direction);
        assert_eq!(calls.get(), 2);
        prepare(
            &cache,
            &mut system,
            &document,
            200.0,
            LayoutDirection::RightToLeft,
        );
        assert_eq!(calls.get(), 3);
        let changed = document
            .edit_plain_paragraph(
                document.version(),
                document.paragraphs()[0].id(),
                0..0,
                "changed ",
            )
            .unwrap();
        let edited = prepare(&cache, &mut system, &changed, 200.0, direction);
        assert_eq!(calls.get(), 4);
        assert_eq!(edited.document_version(), changed.version());
        assert_eq!(wide.document_version(), document.version());
        assert_eq!(cache.entries().len(), 1);
    }

    #[test]
    fn caret_queries_require_current_layout_and_translate_paragraph_origin() {
        use novadraw::text::{TextAffinity, TextPosition};
        let (mut system, _) = system(true);
        let cache = SnapshotCache::default();
        let mut builder = RichTextDocumentBuilder::default();
        for text in ["alpha beta gamma delta", "second paragraph"] {
            let mut input = ParagraphInput::new(
                text,
                novadraw::text::TextStyle::new(FontDescriptor::default(), Color::BLACK),
            );
            input.space_after = 11.0;
            builder.push(input).unwrap();
        }
        let document = builder.build();
        let second = document.paragraphs()[1].id();
        let direction = LayoutDirection::LeftToRight;
        let wide = prepare(&cache, &mut system, &document, 200.0, direction);
        let position = wide
            .position(second, TextPosition::new(0, TextAffinity::Downstream))
            .unwrap();
        let bounds = wide.caret_bounds(&position).unwrap();
        assert_eq!(bounds.y, wide.height_index().offset(1).unwrap());
        assert!(bounds.height > 0.0);
        assert!(
            wide.position(
                second,
                TextPosition::new(usize::MAX, TextAffinity::Downstream)
            )
            .is_err()
        );
        let narrow = prepare(&cache, &mut system, &document, 50.0, direction);
        assert!(narrow.caret_bounds(&position).is_err());
        let changed = document
            .edit_plain_paragraph(document.version(), second, 0..0, "new ")
            .unwrap();
        let edited = prepare(&cache, &mut system, &changed, 200.0, direction);
        assert!(edited.caret_bounds(&position).is_err());
        assert!(wide.caret_bounds(&position).is_ok()); // Original immutable snapshot stays valid.
    }

    #[test]
    fn engine_identity_generation_and_opt_out_control_reuse() {
        let (mut first, first_calls) = system(true);
        let (mut second, second_calls) = system(true);
        let cache = SnapshotCache::default();
        let document = document();
        let direction = LayoutDirection::LeftToRight;
        let initial = prepare(&cache, &mut first, &document, 200.0, direction);
        let other = prepare(&cache, &mut second, &document, 200.0, direction);
        assert!(!Arc::ptr_eq(&initial, &other));
        assert_eq!(second_calls.get(), 1);
        first
            .register_builtin_font(BuiltinFont::JetBrainsMono)
            .unwrap();
        let new_generation = prepare(&cache, &mut first, &document, 200.0, direction);
        assert!(!Arc::ptr_eq(&initial, &new_generation));
        assert_eq!(first_calls.get(), 2);
        let (mut opted_out, calls) = system(false);
        let one = prepare(&cache, &mut opted_out, &document, 200.0, direction);
        let two = prepare(&cache, &mut opted_out, &document, 200.0, direction);
        assert!(!Arc::ptr_eq(&one, &two));
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn failed_budget_and_invalid_width_preserve_successful_cache_and_capacity_is_bounded() {
        let (mut system, calls) = system(true);
        let cache = SnapshotCache::default();
        let document = document();
        let direction = LayoutDirection::LeftToRight;
        let initial = prepare(&cache, &mut system, &document, 200.0, direction);
        for width in [200.0, 70.0] {
            let result = cache.prepare(
                &document,
                &mut system.measure(FontDescriptor::default()),
                Some(width),
                PreparationBudgetHandle::new(PreparationBudget::new(0, 0)),
                direction,
            );
            assert!(result.is_err());
        }
        // Lookup succeeds, then the new layout fails its shared preparation budget.
        assert!(
            cache
                .prepare(
                    &document,
                    &mut system.measure(FontDescriptor::default()),
                    Some(70.0),
                    PreparationBudgetHandle::new(PreparationBudget::new(2, u64::MAX)),
                    direction,
                )
                .is_err()
        );
        let result = cache.prepare(
            &document,
            &mut system.measure(FontDescriptor::default()),
            Some(f64::NAN),
            PreparationBudgetHandle::new(PreparationBudget::default()),
            direction,
        );
        assert!(result.is_err());
        assert!(Arc::ptr_eq(
            &initial,
            &prepare(&cache, &mut system, &document, 200.0, direction)
        ));
        assert_eq!(calls.get(), 1);
        for width in [190.0, 180.0, 170.0] {
            prepare(&cache, &mut system, &document, width, direction);
            assert!(cache.entries().len() <= SNAPSHOT_CAPACITY);
        }
        let fresh = prepare(&cache, &mut system, &document, 200.0, direction);
        assert!(!Arc::ptr_eq(&initial, &fresh));
        assert_eq!(initial.size(), fresh.size());
        assert_eq!(calls.get(), 5);
    }
}

type SnapshotEntries = Vec<(Key, Arc<RichTextLayoutSnapshot>)>;

#[derive(Clone, Default)]
pub(crate) struct SnapshotCache(Arc<Mutex<SnapshotEntries>>);
impl SnapshotCache {
    /// Isolated eviction state with shared immutable layouts for a candidate.
    pub(crate) fn fork(&self) -> Self {
        Self(Arc::new(Mutex::new(self.entries().clone())))
    }
    fn entries(&self) -> std::sync::MutexGuard<'_, SnapshotEntries> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    pub(crate) fn prepare(
        &self,
        document: &RichTextDocumentSnapshot,
        context: &mut MeasureContext<'_>,
        width: Option<f64>,
        budget: PreparationBudgetHandle,
        direction: LayoutDirection,
    ) -> Result<Arc<RichTextLayoutSnapshot>, TextError> {
        let width = width.map(|w| w as f32);
        TextConstraints::new(width)?;
        budget.consume("richtext cache lookup", 1)?;
        let key = context.cache_identity().map(|engine| Key {
            document: document.version(),
            engine,
            generation: context.revision(),
            width: width.map(f32::to_bits),
            direction,
        });
        if let Some(key) = key {
            let hit = {
                let mut entries = self.entries();
                entries
                    .iter()
                    .position(|(cached, _)| *cached == key)
                    .map(|index| {
                        let entry = entries.remove(index);
                        let snapshot = entry.1.clone();
                        entries.push(entry);
                        snapshot
                    })
            };
            if let Some(snapshot) = hit {
                let _scope = TextLayoutDiagnostics::default().time("richtext.cache-hit");
                budget.check()?;
                return Ok(snapshot);
            }
        }
        let previous = key.and_then(|key| {
            self.entries()
                .iter()
                .rev()
                .find(|(cached, _)| {
                    cached.engine == key.engine
                        && cached.generation == key.generation
                        && cached.width == key.width
                        && cached.direction == key.direction
                })
                .map(|(_, snapshot)| snapshot.clone())
        });
        // No cache lock crosses callbacks into the consumer's text engine.
        let snapshot = RichTextLayoutSnapshot::prepare(
            document,
            context,
            width.map(f64::from),
            budget,
            direction,
            previous.as_deref(),
        )?;
        if let Some(key) = key
            && context.cache_identity() == Some(key.engine)
            && context.revision() == key.generation
        {
            let mut entries = self.entries();
            // Retire obsolete document/font generations before keeping a candidate.
            entries.retain(|(cached, _)| {
                cached.document == key.document
                    && cached.engine == key.engine
                    && cached.generation == key.generation
            });
            if entries.len() == SNAPSHOT_CAPACITY {
                entries.remove(0);
            }
            entries.push((key, snapshot.clone()));
        }
        Ok(snapshot)
    }
}
