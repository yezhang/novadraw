use crate::{DocumentVersion, RichTextDocumentSnapshot, RichTextLayoutSnapshot};
use novadraw::{
    figure::{LayoutDirection, PreparationBudgetHandle},
    text::{
        MeasureContext, TextConstraints, TextError, TextLayoutDiagnostics,
        shaping::TextLayoutCacheIdentity,
    },
};
use std::{cell::RefCell, sync::Arc};

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
        assert_eq!(cache.0.borrow().len(), 1);
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
            assert!(cache.0.borrow().len() <= SNAPSHOT_CAPACITY);
        }
        let fresh = prepare(&cache, &mut system, &document, 200.0, direction);
        assert!(!Arc::ptr_eq(&initial, &fresh));
        assert_eq!(initial.size(), fresh.size());
        assert_eq!(calls.get(), 5);
    }
}

#[derive(Default)]
pub(crate) struct SnapshotCache(RefCell<Vec<(Key, Arc<RichTextLayoutSnapshot>)>>);
impl SnapshotCache {
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
                let mut entries = self.0.borrow_mut();
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
        // No cache borrow crosses callbacks into the consumer's text engine.
        let snapshot = RichTextLayoutSnapshot::prepare(
            document,
            context,
            width.map(f64::from),
            budget,
            direction,
        )?;
        if let Some(key) = key
            && context.cache_identity() == Some(key.engine)
            && context.revision() == key.generation
        {
            let mut entries = self.0.borrow_mut();
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
