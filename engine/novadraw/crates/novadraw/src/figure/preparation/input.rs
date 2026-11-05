use std::{
    any::{Any, TypeId},
    collections::HashMap,
    error::Error,
    fmt,
    marker::PhantomData,
    sync::{Arc, Weak},
};

use crate::{FigureId, text::TextError};

use super::PreparationScene;

/// Behavior identity is the payload type; the name is only diagnostic.
pub struct PreparationInputKey<P> {
    name: &'static str,
    marker: PhantomData<fn() -> P>,
}
impl<P> PreparationInputKey<P> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }
}
impl<P> Copy for PreparationInputKey<P> {}
impl<P> Clone for PreparationInputKey<P> {
    fn clone(&self) -> Self {
        *self
    }
}

/// An owned immutable value, compared in full. Equality must be pure and bounded.
/// Shared mutable state is not a snapshot, even when its pointer compares equal.
pub trait PreparationInputPayload: Any + Eq + Send + Sync {
    /// Owned backing retained by this value, excluding the fixed snapshot record.
    fn retained_units(&self) -> u64;
}

/// A registered reader must be pure, bounded and preserve the original failure.
/// It must reserve its temporary allocations through the scene's budget.
/// Its state must not be written concurrently with Runtime stabilization.
pub trait PreparationInputReader<P: PreparationInputPayload>: Send + Sync {
    fn read(&self, source: FigureId, scene: &PreparationScene<'_>) -> Result<P, TextError>;
}
impl<P, F> PreparationInputReader<P> for F
where
    P: PreparationInputPayload,
    F: Fn(FigureId, &PreparationScene<'_>) -> Result<P, TextError> + Send + Sync,
{
    fn read(&self, source: FigureId, scene: &PreparationScene<'_>) -> Result<P, TextError> {
        self(source, scene)
    }
}

trait InputValue: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn equals(&self, other: &dyn InputValue) -> bool;
}
impl<P: PreparationInputPayload> InputValue for P {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn equals(&self, other: &dyn InputValue) -> bool {
        other.as_any().downcast_ref::<P>() == Some(self)
    }
}

/// Owned evidence. It keeps no reader, tree or Runtime alive.
#[derive(Clone)]
pub struct PreparationInputSnapshot(Arc<SnapshotValue>);
struct SnapshotValue {
    value: Arc<dyn InputValue>,
    registration: uuid::Uuid,
    units: u64,
    _retention: Option<crate::text::TextContentLease>,
}
impl PreparationInputSnapshot {
    pub fn value<P: PreparationInputPayload>(&self, _key: PreparationInputKey<P>) -> Option<&P> {
        self.0.value.as_any().downcast_ref()
    }
    pub(crate) fn retained_units(&self) -> u64 {
        self.0.units
    }
}
impl fmt::Debug for PreparationInputSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparationInputSnapshot(..)")
    }
}
impl PartialEq for PreparationInputSnapshot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.registration == other.0.registration
                && (Arc::ptr_eq(&self.0.value, &other.0.value)
                    || self.0.value.equals(other.0.value.as_ref())))
    }
}
impl Eq for PreparationInputSnapshot {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparationInputErrorKind {
    AccountingOverflow,
    DuplicateRegistration,
    UnknownRegistration,
    RetiredRegistration,
    Changed {
        first: PreparationInputSnapshot,
        current: PreparationInputSnapshot,
    },
}

/// A typed input failure retains full first/current values without formatting arbitrary payloads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparationInputError {
    pub source: Option<FigureId>,
    pub payload_type: TypeId,
    pub name: &'static str,
    pub checkpoint: &'static str,
    pub kind: PreparationInputErrorKind,
}
impl fmt::Display for PreparationInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "preparation input {} at {}: {:?}",
            self.name, self.checkpoint, self.kind
        )
    }
}
impl Error for PreparationInputError {}
impl From<PreparationInputError> for TextError {
    fn from(error: PreparationInputError) -> Self {
        Self::PreparationInput(Box::new(error))
    }
}

type Reader = dyn Fn(FigureId, &PreparationScene<'_>) -> Result<PreparationInputSnapshot, TextError>
    + Send
    + Sync;

struct Registration {
    payload_type: TypeId,
    name: &'static str,
    reader: Box<Reader>,
    store: Option<crate::text::TextContentStore>,
}

/// Instance-owned readers. Clones retain the same registration identities.
/// Dropping the last registry owner retires its readers; old evidence remains readable.
#[derive(Clone, Default)]
pub struct PreparationInputRegistry {
    entries: HashMap<TypeId, Arc<Registration>>,
}
impl PreparationInputRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register<P: PreparationInputPayload>(
        &mut self,
        key: PreparationInputKey<P>,
        reader: impl PreparationInputReader<P> + 'static,
    ) -> Result<(), PreparationInputError> {
        self.register_with_store(key, reader, None)
    }

    pub(crate) fn register_with_store<P: PreparationInputPayload>(
        &mut self,
        key: PreparationInputKey<P>,
        reader: impl PreparationInputReader<P> + 'static,
        store: Option<crate::text::TextContentStore>,
    ) -> Result<(), PreparationInputError> {
        let payload_type = TypeId::of::<P>();
        if self.entries.contains_key(&payload_type) {
            // Registration errors have no Figure owner.
            return Err(PreparationInputError {
                source: None,
                payload_type,
                name: key.name,
                checkpoint: "registration",
                kind: PreparationInputErrorKind::DuplicateRegistration,
            });
        }
        let registration = uuid::Uuid::new_v4();
        self.entries.insert(
            payload_type,
            Arc::new(Registration {
                payload_type,
                name: key.name,
                store: store.clone(),
                reader: Box::new(move |source, scene| {
                    let value = reader.read(source, scene)?;
                    let units = value
                        .retained_units()
                        .checked_add(
                            std::mem::size_of::<P>() as u64
                                + std::mem::size_of::<SnapshotValue>() as u64
                                + 4 * std::mem::size_of::<usize>() as u64,
                        )
                        .ok_or_else(|| {
                            TextError::from(PreparationInputError {
                                source: Some(source),
                                payload_type,
                                name: key.name,
                                checkpoint: "snapshot admission",
                                kind: PreparationInputErrorKind::AccountingOverflow,
                            })
                        })?;
                    scene
                        .budget()
                        .allocate("preparation input snapshot", units)?;
                    // Incoming payload buffers are reader-owned temporary work.
                    // The shared immutable snapshot is admitted before its allocation.
                    let retention = store
                        .as_ref()
                        .map(|store| {
                            let units = usize::try_from(units)
                                .map_err(|_| crate::text::TextContentError::AccountingOverflow)?;
                            store.reserve(units)
                        })
                        .transpose()?;
                    Ok(PreparationInputSnapshot(Arc::new(SnapshotValue {
                        value: Arc::new(value),
                        registration,
                        units,
                        _retention: retention,
                    })))
                }),
            }),
        );
        Ok(())
    }

    pub(crate) fn capture<P: PreparationInputPayload>(
        &self,
        key: PreparationInputKey<P>,
        source: FigureId,
        scene: &PreparationScene<'_>,
    ) -> Result<InputProof, TextError> {
        let entry = self.entries.get(&TypeId::of::<P>()).ok_or_else(|| {
            TextError::from(PreparationInputError {
                source: Some(source),
                payload_type: TypeId::of::<P>(),
                name: key.name,
                checkpoint: "capture",
                kind: PreparationInputErrorKind::UnknownRegistration,
            })
        })?;
        scene.budget().allocate(
            "preparation input proof",
            (std::mem::size_of::<InputProofRecord>() + 2 * std::mem::size_of::<usize>()) as u64,
        )?;
        let first = (entry.reader)(source, scene)?;
        let retention = entry
            .store
            .as_ref()
            .map(|store| {
                store.reserve(
                    std::mem::size_of::<InputProofRecord>() + 2 * std::mem::size_of::<usize>(),
                )
            })
            .transpose()?;
        Ok(InputProof(Arc::new(InputProofRecord {
            _retention: retention,
            root: scene.root(),
            source,
            payload_type: entry.payload_type,
            name: entry.name,
            registration: Arc::downgrade(entry),
            first,
            constraints: scene.measurement_constraints(),
        })))
    }
}

#[derive(Clone)]
pub(crate) struct InputProof(Arc<InputProofRecord>);
impl std::ops::Deref for InputProof {
    type Target = InputProofRecord;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
pub(crate) struct InputProofRecord {
    _retention: Option<crate::text::TextContentLease>,
    pub(crate) root: FigureId,
    source: FigureId,
    payload_type: TypeId,
    name: &'static str,
    registration: Weak<Registration>,
    pub(crate) first: PreparationInputSnapshot,
    constraints: Option<(crate::MeasureConstraints, crate::MeasureConstraints)>,
}
impl InputProof {
    fn error(&self, checkpoint: &'static str, kind: PreparationInputErrorKind) -> TextError {
        PreparationInputError {
            source: Some(self.source),
            payload_type: self.payload_type,
            name: self.name,
            checkpoint,
            kind,
        }
        .into()
    }
    fn same_subject(&self, other: &Self) -> bool {
        self.root == other.root
            && self.source == other.source
            && self.constraints == other.constraints
            && Weak::ptr_eq(&self.registration, &other.registration)
    }
    pub(crate) fn validate(
        &self,
        scene: &PreparationScene<'_>,
        checkpoint: &'static str,
    ) -> Result<(), TextError> {
        let _timing = scene.budget().time("preparation-input-validate");
        scene
            .budget()
            .consume("preparation input verification", 1)?;
        let reader = self.registration.upgrade().ok_or_else(|| {
            self.error(checkpoint, PreparationInputErrorKind::RetiredRegistration)
        })?;
        let scoped = scene.input_scope(self.root, self.source, self.constraints)?;
        let current = (reader.reader)(self.source, &scoped)?;
        if current != self.first {
            return Err(self.error(
                checkpoint,
                PreparationInputErrorKind::Changed {
                    first: self.first.clone(),
                    current,
                },
            ));
        }
        scene.budget().check()?;
        Ok(())
    }
}

/// Merge is all-or-nothing and never replaces the first value on conflict.
pub(crate) fn merge_proofs(
    existing: &mut Vec<InputProof>,
    incoming: &[InputProof],
    budget: &super::PreparationBudgetHandle,
) -> Result<(), TextError> {
    let _timing = budget.time("preparation-input-merge");
    for proof in incoming {
        budget.consume(
            "preparation input merge",
            existing.len() as u64 + incoming.len() as u64,
        )?;
        if let Some(first) = existing.iter().find(|first| first.same_subject(proof))
            && first.first != proof.first
        {
            return Err(first.error(
                "merge",
                PreparationInputErrorKind::Changed {
                    first: first.first.clone(),
                    current: proof.first.clone(),
                },
            ));
        }
        for earlier in incoming {
            if earlier.same_subject(proof) && earlier.first != proof.first {
                return Err(earlier.error(
                    "merge",
                    PreparationInputErrorKind::Changed {
                        first: earlier.first.clone(),
                        current: proof.first.clone(),
                    },
                ));
            }
        }
    }
    let count = incoming
        .iter()
        .filter(|proof| !existing.iter().any(|first| first.same_subject(proof)))
        .count();
    budget.allocate(
        "preparation input merge",
        count as u64 * std::mem::size_of::<InputProof>() as u64,
    )?;
    for proof in incoming {
        if !existing.iter().any(|first| first.same_subject(proof)) {
            existing.push(proof.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Dimension, Figure, FigureMeasurement, MeasureConstraints, Rectangle,
        figure::{
            FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureDrawing,
            FigurePreparation, FigurePresentation, PREPARATION, PreparationCapability,
            PreparationMeasureError, PreparationMeasureErrorKind, PreparationMeasurement,
            PreparedScene, RectangleFigure,
        },
        graphics::{GraphicsError, PaintContext},
        render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
        runtime::{FramePreparation, FramePreparationError, Runtime},
        text::{FontDescriptor, MeasureContext, TextSystem},
        tree::FigureTree,
    };
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    #[derive(Eq, PartialEq)]
    struct Version(u64);
    impl PreparationInputPayload for Version {
        fn retained_units(&self) -> u64 {
            0
        }
    }
    const VERSION: PreparationInputKey<Version> = PreparationInputKey::new("test.version");

    fn registry(version: &Arc<AtomicU64>) -> PreparationInputRegistry {
        let version = version.clone();
        let mut registry = PreparationInputRegistry::new();
        registry
            .register(VERSION, move |_: FigureId, _: &PreparationScene<'_>| {
                Ok(Version(version.load(Ordering::Relaxed)))
            })
            .unwrap();
        registry
    }
    fn scene_fixture() -> (
        FigureTree,
        FigureId,
        HashMap<FigureId, crate::figure::ResolvedStyle>,
    ) {
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let styles = HashMap::from([(root, tree.resolved_style(root).unwrap())]);
        (tree, root, styles)
    }
    fn changed(error: &TextError, checkpoint: &'static str) {
        assert!(matches!(error, TextError::PreparationInput(error)
            if error.checkpoint == checkpoint
            && matches!(&error.kind, PreparationInputErrorKind::Changed { first, current }
                if first.value(VERSION).unwrap().0 == 1 && current.value(VERSION).unwrap().0 == 2)));
    }

    #[test]
    fn full_first_value_is_retained_and_ignored_conflicts_stay_failed() {
        let (tree, root, styles) = scene_fixture();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let version = Arc::new(AtomicU64::new(1));
        let registry = registry(&version);
        let snapshot = scene.input(&registry, VERSION, root).unwrap();
        version.store(2, Ordering::Relaxed);
        let error = scene.input(&registry, VERSION, root).unwrap_err();
        changed(&error, "merge");
        assert_eq!(scene.validate_input_proofs("later").unwrap_err(), error);
        assert_eq!(snapshot.value(VERSION).unwrap().0, 1);
    }

    #[test]
    fn borrowed_checkpoints_keep_nested_reader_logs_independent() {
        #[derive(Eq, PartialEq)]
        struct Outer(u64);
        impl PreparationInputPayload for Outer {
            fn retained_units(&self) -> u64 {
                0
            }
        }
        const OUTER: PreparationInputKey<Outer> = PreparationInputKey::new("test.outer");
        let (tree, root, styles) = scene_fixture();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let version = Arc::new(AtomicU64::new(1));
        let inner = registry(&version);
        let mut outer = PreparationInputRegistry::new();
        outer
            .register(OUTER, move |source, scoped: &PreparationScene<'_>| {
                let value = scoped.input(&inner, VERSION, source)?;
                Ok(Outer(value.value(VERSION).unwrap().0))
            })
            .unwrap();
        scene.input(&outer, OUTER, root).unwrap();
        scene.validate_input_proofs("nested return").unwrap();
        version.store(2, Ordering::Relaxed);
        let failure = scene.validate_input_proofs("changed nested").unwrap_err();
        assert!(matches!(failure, TextError::PreparationInput(_)));
        assert_eq!(scene.validate_input_proofs("latched").unwrap_err(), failure);
    }

    #[test]
    fn retired_reader_is_not_called_and_old_evidence_survives() {
        let (tree, root, styles) = scene_fixture();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let version = Arc::new(AtomicU64::new(1));
        let registry = registry(&version);
        let clone = registry.clone();
        let evidence = scene.input(&registry, VERSION, root).unwrap();
        drop(registry);
        scene.validate_input_proofs("live clone").unwrap();
        drop(clone);
        assert!(matches!(scene.validate_input_proofs("retired"),
            Err(TextError::PreparationInput(error))
                if error.kind == PreparationInputErrorKind::RetiredRegistration));
        assert_eq!(evidence.value(VERSION).unwrap().0, 1);
    }

    #[test]
    fn reader_error_is_preserved_and_cannot_be_ignored() {
        let (tree, root, styles) = scene_fixture();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let mut registry = PreparationInputRegistry::new();
        let calls = Arc::new(AtomicU64::new(0));
        let reader_calls = calls.clone();
        registry
            .register(VERSION, move |_: FigureId, _: &PreparationScene<'_>| {
                reader_calls.fetch_add(1, Ordering::Relaxed);
                Err::<Version, _>(TextError::NoUsableFont)
            })
            .unwrap();
        assert_eq!(
            scene.input(&registry, VERSION, root),
            Err(TextError::NoUsableFont)
        );
        assert_eq!(
            scene.validate_input_proofs("return"),
            Err(TextError::NoUsableFont)
        );
        assert_eq!(
            scene.input(&registry, VERSION, root),
            Err(TextError::NoUsableFont)
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn equal_generations_do_not_hide_changes_in_the_full_typed_value() {
        #[derive(Eq, PartialEq)]
        struct CompleteInput {
            generation: u64,
            value: u64,
        }
        impl PreparationInputPayload for CompleteInput {
            fn retained_units(&self) -> u64 {
                0
            }
        }
        let key = PreparationInputKey::<CompleteInput>::new("test.complete");
        let (tree, root, styles) = scene_fixture();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let value = Arc::new(AtomicU64::new(1));
        let reader_value = value.clone();
        let mut registry = PreparationInputRegistry::new();
        registry
            .register(key, move |_: FigureId, _: &PreparationScene<'_>| {
                Ok(CompleteInput {
                    generation: 7,
                    value: reader_value.load(Ordering::Relaxed),
                })
            })
            .unwrap();
        let first = scene.input(&registry, key, root).unwrap();
        assert!(first.value(VERSION).is_none());
        value.store(2, Ordering::Relaxed);
        let error = scene.validate_input_proofs("full value").unwrap_err();
        let TextError::PreparationInput(error) = error else {
            panic!("{error:?}")
        };
        let PreparationInputErrorKind::Changed { first, current } = error.kind else {
            panic!("{error:?}");
        };
        assert_eq!(
            first.value(key).unwrap().generation,
            current.value(key).unwrap().generation
        );
        assert_eq!(first.value(key).unwrap().value, 1);
        assert_eq!(current.value(key).unwrap().value, 2);
    }

    #[test]
    fn foreign_identity_and_exhausted_budget_do_not_invoke_the_reader() {
        let (tree, root, styles) = scene_fixture();
        let (_, foreign, _) = scene_fixture();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let calls = Arc::new(AtomicU64::new(0));
        let reader_calls = calls.clone();
        let mut registry = PreparationInputRegistry::new();
        registry
            .register(VERSION, move |_: FigureId, _: &PreparationScene<'_>| {
                reader_calls.fetch_add(1, Ordering::Relaxed);
                Ok(Version(1))
            })
            .unwrap();
        assert!(scene.input(&registry, VERSION, foreign).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert!(scene.validate_input_proofs("foreign return").is_err());
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        assert!(scene.budget().allocate("exhaust", u64::MAX).is_err());
        assert!(scene.input(&registry, VERSION, root).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert!(scene.validate_input_proofs("return").is_err());
    }

    struct Empty;
    impl FigureDrawing for Empty {
        fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
            Ok(())
        }
    }
    fn presentation(width: f64) -> FigurePresentation {
        FigurePresentation::new(
            FigureMeasurement::new(width, 1.0, None),
            Dimension::ZERO,
            Rectangle::ZERO,
            Arc::new(Empty),
        )
        .unwrap()
    }
    struct Producer {
        inputs: PreparationInputRegistry,
        mutate: Option<(Arc<AtomicBool>, Arc<AtomicU64>)>,
        drawing_width: Option<f64>,
    }
    impl Figure for Producer {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }
        fn name(&self) -> &'static str {
            "ProofProducer"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(PREPARATION, PreparationCapability::of::<Self>())
        }
    }
    impl FigurePreparation for Producer {
        fn measure_scene(
            &self,
            _: &mut MeasureContext<'_>,
            constraints: MeasureConstraints,
            scene: &PreparationScene<'_>,
        ) -> Result<PreparationMeasurement, PreparationMeasureError> {
            let value = scene
                .input(&self.inputs, VERSION, scene.root())
                .map_err(|error| PreparationMeasureError {
                    figure: scene.root(),
                    outer_constraints: constraints,
                    content_constraints: Some(constraints),
                    kind: PreparationMeasureErrorKind::Text(Box::new(error)),
                })?;
            Ok(PreparationMeasurement {
                preferred: FigureMeasurement::new(
                    value.value(VERSION).unwrap().0 as f64,
                    1.0,
                    None,
                ),
                minimum: Dimension::ZERO,
            })
        }
        fn prepare(
            &self,
            _: &mut MeasureContext<'_>,
            _: Rectangle,
        ) -> Result<FigurePresentation, TextError> {
            Ok(presentation(1.0))
        }
        fn prepare_scene(
            &self,
            _: &mut MeasureContext<'_>,
            _: Rectangle,
            scene: &PreparationScene<'_>,
        ) -> Result<PreparedScene, TextError> {
            if let Some((armed, version)) = &self.mutate {
                if armed.swap(false, Ordering::Relaxed) {
                    version.store(2, Ordering::Relaxed);
                }
                return Ok(PreparedScene::new(presentation(1.0)));
            }
            let value = scene.input(&self.inputs, VERSION, scene.root())?;
            // This fixture can keep drawing metrics stable while measurement inputs change.
            let width = self
                .drawing_width
                .unwrap_or(value.value(VERSION).unwrap().0 as f64);
            Ok(PreparedScene::new(presentation(width)))
        }
    }

    #[test]
    fn measurement_cache_retains_proof_and_rechecks_before_return() {
        let version = Arc::new(AtomicU64::new(1));
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(Producer {
                inputs: registry(&version),
                mutate: None,
                drawing_width: None,
            }))
            .unwrap();
        let styles = HashMap::from([(root, tree.resolved_style(root).unwrap())]);
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let mut text = TextSystem::new();
        let mut context = text.measure(FontDescriptor::default());
        let first = scene
            .measure_figure(&mut context, root, MeasureConstraints::UNBOUNDED)
            .unwrap();
        assert_eq!(first.preferred.width, 1.0);
        assert_eq!(
            scene
                .measure_figure(&mut context, root, MeasureConstraints::UNBOUNDED)
                .unwrap(),
            first
        );
        version.store(2, Ordering::Relaxed);
        let error = scene
            .measure_figure(&mut context, root, MeasureConstraints::UNBOUNDED)
            .unwrap_err();
        match error.kind {
            PreparationMeasureErrorKind::Text(error) => changed(&error, "measurement cache return"),
            other => panic!("{other:?}"),
        }
    }

    fn frame(runtime: &mut Runtime) -> Result<(), FramePreparationError> {
        match runtime.frames().prepare_submission(
            SurfaceInfo {
                logical_width: 100.0,
                logical_height: 100.0,
                pixel_width: 100,
                pixel_height: 100,
                scale_factor: 1.0,
            },
            BackendCapabilities::RETAINED_PARTIAL,
        ) {
            FramePreparation::Ready(submission) => {
                let _ = runtime
                    .frames()
                    .complete_submission(
                        submission.session_id,
                        submission.frame_id,
                        Ok(RenderDisposition::Presented),
                    )
                    .unwrap();
                Ok(())
            }
            FramePreparation::Error(error) => Err(error),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn batch_check_rejects_a_later_callback_and_restores_published_result() {
        let version = Arc::new(AtomicU64::new(1));
        let armed = Arc::new(AtomicBool::new(false));
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let first = tree
            .builder()
            .add_child(
                root,
                Box::new(Producer {
                    inputs: registry(&version),
                    mutate: None,
                    drawing_width: None,
                }),
            )
            .unwrap();
        let second = tree
            .builder()
            .add_child(
                root,
                Box::new(Producer {
                    inputs: PreparationInputRegistry::new(),
                    mutate: Some((armed.clone(), version.clone())),
                    drawing_width: None,
                }),
            )
            .unwrap();
        let mut runtime = Runtime::new(tree);
        frame(&mut runtime).unwrap();
        let old = runtime
            .tree()
            .prepared_presentation(first)
            .unwrap()
            .unwrap()
            .clone();
        armed.store(true, Ordering::Relaxed);
        runtime
            .figure(second)
            .unwrap()
            .set_bounds(Rectangle::new(1.0, 0.0, 11.0, 10.0))
            .unwrap();
        let error = frame(&mut runtime).unwrap_err();
        match error {
            FramePreparationError::Text(error) => changed(&error, "preparation batch publication"),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            runtime
                .tree()
                .prepared_presentation(first)
                .unwrap()
                .unwrap()
                .measurement(),
            old.measurement()
        );
        assert_eq!(old.measurement().width, 1.0);
        frame(&mut runtime).unwrap();
        assert_eq!(
            runtime
                .tree()
                .prepared_presentation(first)
                .unwrap()
                .unwrap()
                .measurement()
                .width,
            2.0
        );
    }

    struct MutatingLayout {
        version: Arc<AtomicU64>,
        armed: Arc<AtomicBool>,
        fixed_width: Option<f64>,
    }
    impl crate::layout::LayoutManager for MutatingLayout {
        fn preferred_measurement(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &crate::layout::LayoutSceneQuery<'_>,
        ) -> Result<FigureMeasurement, crate::layout::LayoutError> {
            Ok(FigureMeasurement::new(100.0, 100.0, None))
        }
        fn minimum_size(
            &self,
            _: FigureId,
            _: MeasureConstraints,
            _: &crate::layout::LayoutSceneQuery<'_>,
        ) -> Result<Dimension, crate::layout::LayoutError> {
            Ok(Dimension::ZERO)
        }
        fn layout(
            &mut self,
            container: FigureId,
            query: &crate::layout::LayoutSceneQuery<'_>,
            output: &mut crate::layout::LayoutOutput,
        ) -> Result<(), crate::layout::LayoutError> {
            let child = query.children(container)?[0].0;
            let measured = query.preferred_measurement(child, MeasureConstraints::UNBOUNDED)?;
            if self.armed.swap(false, Ordering::Relaxed) {
                self.version.store(2, Ordering::Relaxed);
            }
            output.set_child_bounds(
                child,
                Rectangle::new(
                    0.0,
                    0.0,
                    self.fixed_width.unwrap_or(measured.width),
                    measured.height,
                ),
            );
            Ok(())
        }
    }
    #[test]
    fn layout_consumer_rejects_changed_input_before_applying_output() {
        let version = Arc::new(AtomicU64::new(1));
        let armed = Arc::new(AtomicBool::new(false));
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(Producer {
                    inputs: registry(&version),
                    mutate: None,
                    drawing_width: None,
                }),
            )
            .unwrap();
        let mut runtime = Runtime::new(tree);
        runtime
            .figure(root)
            .unwrap()
            .set_layout_manager(Box::new(MutatingLayout {
                version: version.clone(),
                armed: armed.clone(),
                fixed_width: None,
            }))
            .unwrap();
        frame(&mut runtime).unwrap();
        let old = runtime.tree().figure_bounds(child).unwrap();
        armed.store(true, Ordering::Relaxed);
        runtime
            .figure(root)
            .unwrap()
            .set_bounds(Rectangle::new(0.0, 0.0, 101.0, 100.0))
            .unwrap();
        let error = frame(&mut runtime).unwrap_err();
        let mut cause: &(dyn Error + 'static) = &error;
        let found = loop {
            if let Some(input) = cause.downcast_ref::<PreparationInputError>() {
                break input;
            }
            cause = cause.source().expect("input error retained as source");
        };
        assert_eq!(found.checkpoint, "layout consumption");
        assert!(matches!(
            found.kind,
            PreparationInputErrorKind::Changed { .. }
        ));
        assert_eq!(runtime.tree().figure_bounds(child).unwrap(), old);
        frame(&mut runtime).unwrap();
        assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 2.0);
    }

    struct ValidationRoot {
        version: Arc<AtomicU64>,
        armed: Arc<AtomicBool>,
    }

    #[test]
    fn cached_layout_rechecks_inputs_when_drawing_metrics_stay_equal() {
        let version = Arc::new(AtomicU64::new(1));
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(Producer {
                    inputs: registry(&version),
                    mutate: None,
                    drawing_width: Some(1.0),
                }),
            )
            .unwrap();
        let mut runtime = Runtime::new(tree);
        runtime
            .figure(root)
            .unwrap()
            .set_layout_manager(Box::new(MutatingLayout {
                version: version.clone(),
                armed: Arc::new(AtomicBool::new(false)),
                fixed_width: Some(10.0),
            }))
            .unwrap();
        frame(&mut runtime).unwrap();
        let old_bounds = runtime.tree().figure_bounds(child).unwrap();
        let old_presentation = runtime
            .tree()
            .prepared_presentation(child)
            .unwrap()
            .unwrap()
            .clone();
        let old_proofs = runtime
            .tree()
            .node(root)
            .unwrap()
            .layout
            .input_proofs
            .clone();
        version.store(2, Ordering::Relaxed);
        frame(&mut runtime).unwrap();
        assert_eq!(runtime.tree().figure_bounds(child).unwrap(), old_bounds);
        assert_eq!(
            runtime
                .tree()
                .prepared_presentation(child)
                .unwrap()
                .unwrap()
                .measurement(),
            old_presentation.measurement()
        );
        assert!(
            old_proofs
                .iter()
                .all(|p| p.first.value(VERSION).unwrap().0 == 1)
        );
        let current = &runtime.tree().node(root).unwrap().layout.input_proofs;
        assert!(!current.is_empty());
        assert!(
            current
                .iter()
                .all(|p| p.first.value(VERSION).unwrap().0 == 2)
        );
        runtime.frames().stabilize_for_query().unwrap();
    }

    #[test]
    fn clearing_or_replacing_layout_manager_retires_only_its_measurement_proofs() {
        let version = Arc::new(AtomicU64::new(1));
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap(),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(Producer {
                    inputs: registry(&version),
                    mutate: None,
                    drawing_width: None,
                }),
            )
            .unwrap();
        let mut runtime = Runtime::new(tree);
        for replacement in [true, false] {
            runtime
                .figure(root)
                .unwrap()
                .set_layout_manager(Box::new(MutatingLayout {
                    version: version.clone(),
                    armed: Arc::new(AtomicBool::new(false)),
                    fixed_width: None,
                }))
                .unwrap();
            frame(&mut runtime).unwrap();
            assert!(
                !runtime
                    .tree()
                    .node(root)
                    .unwrap()
                    .layout
                    .input_proofs
                    .is_empty()
            );
            let prepared_proof_count = runtime
                .tree()
                .node(child)
                .unwrap()
                .prepared
                .as_ref()
                .unwrap()
                .input_proofs
                .len();
            assert!(prepared_proof_count > 0);
            if replacement {
                runtime
                    .figure(root)
                    .unwrap()
                    .set_layout_manager(Box::new(crate::StackLayout))
                    .unwrap();
            } else {
                runtime
                    .figure(root)
                    .unwrap()
                    .clear_layout_manager()
                    .unwrap();
            }
            assert!(
                runtime
                    .tree()
                    .node(root)
                    .unwrap()
                    .layout
                    .input_proofs
                    .is_empty()
            );
            assert_eq!(
                runtime
                    .tree()
                    .node(child)
                    .unwrap()
                    .prepared
                    .as_ref()
                    .unwrap()
                    .input_proofs
                    .len(),
                prepared_proof_count,
            );
            frame(&mut runtime).unwrap();
            assert!(
                runtime
                    .tree()
                    .node(root)
                    .unwrap()
                    .layout
                    .input_proofs
                    .is_empty()
            );
        }
    }
    impl Figure for ValidationRoot {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        }
        fn name(&self) -> &'static str {
            "ValidationRoot"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(
                crate::figure::CONTAINER,
                crate::figure::ContainerCapability::of::<Self>(),
            )?;
            out.register(
                crate::figure::LIFECYCLE,
                crate::figure::LifecycleCapability::of::<Self>(),
            )
        }
    }
    impl crate::figure::FigureContainer for ValidationRoot {}
    impl crate::figure::FigureLifecycle for ValidationRoot {
        fn validate(&mut self, _: Rectangle) {
            if self.armed.swap(false, Ordering::Relaxed) {
                self.version.store(2, Ordering::Relaxed);
            }
        }
    }

    #[test]
    fn runtime_final_check_retains_consumed_measurement_proof_across_later_preparation() {
        let version = Arc::new(AtomicU64::new(1));
        let armed = Arc::new(AtomicBool::new(false));
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(ValidationRoot {
                version: version.clone(),
                armed: armed.clone(),
            }))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(Producer {
                    inputs: registry(&version),
                    mutate: None,
                    drawing_width: Some(1.0),
                }),
            )
            .unwrap();
        let mut runtime = Runtime::new(tree);
        runtime
            .figure(root)
            .unwrap()
            .set_layout_manager(Box::new(MutatingLayout {
                version: version.clone(),
                armed: Arc::new(AtomicBool::new(false)),
                fixed_width: Some(10.0),
            }))
            .unwrap();
        frame(&mut runtime).unwrap();
        let old = runtime.tree().figure_bounds(child).unwrap();
        let old_presentation = runtime
            .tree()
            .prepared_presentation(child)
            .unwrap()
            .unwrap()
            .clone();
        armed.store(true, Ordering::Relaxed);
        runtime
            .figure(root)
            .unwrap()
            .set_bounds(Rectangle::new(0.0, 0.0, 101.0, 100.0))
            .unwrap();
        let error = frame(&mut runtime).unwrap_err();
        match error {
            FramePreparationError::Text(error) => changed(&error, "runtime publication"),
            other => panic!("{other:?}"),
        }
        // Source geometry is rebased, but the old successful drawing is restored.
        assert_eq!(old.width, 10.0);
        assert_eq!(old_presentation.measurement().width, 1.0);
        assert_eq!(
            runtime
                .tree()
                .prepared_presentation(child)
                .unwrap()
                .unwrap()
                .measurement(),
            old_presentation.measurement()
        );
        frame(&mut runtime).unwrap();
        assert_eq!(runtime.tree().figure_bounds(child).unwrap().width, 10.0);
    }
}
