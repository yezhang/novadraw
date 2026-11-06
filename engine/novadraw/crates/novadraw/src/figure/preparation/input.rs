use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    error::Error,
    fmt,
    marker::PhantomData,
    rc::Rc,
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
    DependencyCycle,
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

/// One input protocol, with separate stage-specific read permissions and errors.
pub(crate) trait InputFamily: 'static {
    type Scene<'a>;
    type Error: From<PreparationInputError>
        + From<super::PreparationBudgetError>
        + From<crate::text::TextContentError>;
    fn root(scene: &Self::Scene<'_>) -> FigureId;
    fn constraints(
        scene: &Self::Scene<'_>,
    ) -> Option<(crate::MeasureConstraints, crate::MeasureConstraints)>;
    fn budget(scene: &Self::Scene<'_>) -> super::PreparationBudgetHandle;
    fn read_path(scene: &Self::Scene<'_>) -> InputReadPath;
    fn check_reader(scene: &Self::Scene<'_>) -> Result<(), Self::Error>;
    /// Transfer nested reads from the independent verification scope.
    fn complete_scope(
        scene: &Self::Scene<'_>,
        scoped: &Self::Scene<'_>,
    ) -> Result<(), Self::Error> {
        Self::check_reader(scoped)?;
        Self::check_reader(scene)
    }
    fn scope<'a>(
        scene: &Self::Scene<'a>,
        root: FigureId,
        source: FigureId,
        constraints: Option<(crate::MeasureConstraints, crate::MeasureConstraints)>,
    ) -> Result<Self::Scene<'a>, Self::Error>
    where
        Self: 'a;
}

pub(crate) struct PreparationInputFamily;
impl InputFamily for PreparationInputFamily {
    type Scene<'a> = PreparationScene<'a>;
    type Error = TextError;
    fn root(scene: &PreparationScene<'_>) -> FigureId {
        scene.root()
    }
    fn constraints(
        scene: &PreparationScene<'_>,
    ) -> Option<(crate::MeasureConstraints, crate::MeasureConstraints)> {
        scene.measurement_constraints()
    }
    fn budget(scene: &PreparationScene<'_>) -> super::PreparationBudgetHandle {
        scene.budget()
    }
    fn read_path(scene: &PreparationScene<'_>) -> InputReadPath {
        scene.input_read_path()
    }
    fn check_reader(scene: &PreparationScene<'_>) -> Result<(), TextError> {
        scene.check_input_reader()
    }
    fn complete_scope(
        scene: &PreparationScene<'_>,
        scoped: &PreparationScene<'_>,
    ) -> Result<(), TextError> {
        scoped.check_input_reader()?;
        scene.retain_input_proofs(&scoped.input_proofs()?)
    }
    fn scope<'a>(
        scene: &PreparationScene<'a>,
        root: FigureId,
        source: FigureId,
        constraints: Option<(crate::MeasureConstraints, crate::MeasureConstraints)>,
    ) -> Result<PreparationScene<'a>, TextError>
    where
        Self: 'a,
    {
        scene.input_scope(root, source, constraints)
    }
}

/// The active path is shared across independent verification scopes, never held
/// borrowed while a reader executes. Dropping the guard also unwinds after panic.
#[derive(Clone, Default)]
pub(crate) struct InputReadPath(Rc<RefCell<Vec<InputSubject>>>);

#[derive(PartialEq)]
struct InputSubject {
    root: FigureId,
    source: FigureId,
    registration: uuid::Uuid,
}

struct InputReadGuard(InputReadPath);
impl Drop for InputReadGuard {
    fn drop(&mut self) {
        self.0.0.borrow_mut().pop();
    }
}

impl InputReadPath {
    fn enter<F: InputFamily>(
        &self,
        registration: &Registration<F>,
        source: FigureId,
        scene: &F::Scene<'_>,
    ) -> Result<InputReadGuard, F::Error> {
        let subject = InputSubject {
            root: F::root(scene),
            source,
            registration: registration.identity,
        };
        let mut path = self.0.borrow_mut();
        F::budget(scene).consume("input active path", path.len() as u64 + 1)?;
        if path.contains(&subject) {
            return Err(PreparationInputError {
                source: Some(source),
                payload_type: registration.payload_type,
                name: registration.name,
                checkpoint: "reader entry",
                kind: PreparationInputErrorKind::DependencyCycle,
            }
            .into());
        }
        F::budget(scene).allocate(
            "input active path",
            std::mem::size_of::<InputSubject>() as u64,
        )?;
        path.push(subject);
        Ok(InputReadGuard(self.clone()))
    }
}

type Reader<F> = dyn Fn(
        FigureId,
        &<F as InputFamily>::Scene<'_>,
    ) -> Result<PreparationInputSnapshot, <F as InputFamily>::Error>
    + Send
    + Sync;

struct Registration<F: InputFamily> {
    identity: uuid::Uuid,
    payload_type: TypeId,
    name: &'static str,
    reader: Box<Reader<F>>,
    store: Option<crate::text::TextContentStore>,
}
impl<F: InputFamily> Registration<F> {
    fn read(
        &self,
        source: FigureId,
        scene: &F::Scene<'_>,
    ) -> Result<PreparationInputSnapshot, F::Error> {
        F::check_reader(scene)?;
        let _guard = F::read_path(scene).enter(self, source, scene)?;
        let value = (self.reader)(source, scene)?;
        F::check_reader(scene)?;
        Ok(value)
    }
}

/// Instance-owned readers. Clones retain the same registration identities.
/// Dropping the last registry owner retires its readers; old evidence remains readable.
pub(crate) struct TypedInputRegistry<F: InputFamily> {
    entries: HashMap<TypeId, Arc<Registration<F>>>,
}
impl<F: InputFamily> Clone for TypedInputRegistry<F> {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
        }
    }
}
impl<F: InputFamily> Default for TypedInputRegistry<F> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

/// Preparation-stage readers. Clones share registration identities; snapshots
/// retain owned values without keeping the registry or its readers alive.
#[derive(Clone, Default)]
pub struct PreparationInputRegistry(TypedInputRegistry<PreparationInputFamily>);
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
        self.0
            .register(key, move |source, scene| reader.read(source, scene), store)
    }

    pub(crate) fn capture<P: PreparationInputPayload>(
        &self,
        key: PreparationInputKey<P>,
        source: FigureId,
        scene: &PreparationScene<'_>,
    ) -> Result<InputProof, TextError> {
        self.0.capture(key, source, scene)
    }
}

impl<F: InputFamily> TypedInputRegistry<F> {
    pub(crate) fn register<P: PreparationInputPayload>(
        &mut self,
        key: PreparationInputKey<P>,
        reader: impl Fn(FigureId, &F::Scene<'_>) -> Result<P, F::Error> + Send + Sync + 'static,
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
                identity: registration,
                payload_type,
                name: key.name,
                store: store.clone(),
                reader: Box::new(move |source, scene| {
                    let value = reader(source, scene)?;
                    let units = value
                        .retained_units()
                        .checked_add(
                            std::mem::size_of::<P>() as u64
                                + std::mem::size_of::<SnapshotValue>() as u64
                                + 4 * std::mem::size_of::<usize>() as u64,
                        )
                        .ok_or_else(|| {
                            F::Error::from(PreparationInputError {
                                source: Some(source),
                                payload_type,
                                name: key.name,
                                checkpoint: "snapshot admission",
                                kind: PreparationInputErrorKind::AccountingOverflow,
                            })
                        })?;
                    F::budget(scene).allocate("preparation input snapshot", units)?;
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
        scene: &F::Scene<'_>,
    ) -> Result<TypedInputProof<F>, F::Error> {
        let entry = self.entries.get(&TypeId::of::<P>()).ok_or_else(|| {
            F::Error::from(PreparationInputError {
                source: Some(source),
                payload_type: TypeId::of::<P>(),
                name: key.name,
                checkpoint: "capture",
                kind: PreparationInputErrorKind::UnknownRegistration,
            })
        })?;
        F::budget(scene).allocate(
            "preparation input proof",
            (std::mem::size_of::<InputProofRecord<F>>() + 2 * std::mem::size_of::<usize>()) as u64,
        )?;
        let first = entry.read(source, scene)?;
        let retention = entry
            .store
            .as_ref()
            .map(|store| {
                store.reserve(
                    std::mem::size_of::<InputProofRecord<F>>() + 2 * std::mem::size_of::<usize>(),
                )
            })
            .transpose()?;
        Ok(TypedInputProof(Arc::new(InputProofRecord {
            _retention: retention,
            root: F::root(scene),
            source,
            payload_type: entry.payload_type,
            name: entry.name,
            registration: Arc::downgrade(entry),
            first,
            constraints: F::constraints(scene),
        })))
    }
}

/// Deferred evidence for a property-scoped layout request.
///
/// `first` must be an owned immutable value read by the owner when recording
/// the request. Construction neither reads the model nor grants publication
/// authority. The original registration is held weakly, including across clones.
#[derive(Clone)]
pub struct LayoutRevalidationGuard {
    source: FigureId,
    registration: Weak<Registration<PreparationInputFamily>>,
    payload_type: TypeId,
    name: &'static str,
    first: Arc<dyn InputValue>,
    units: u64,
}

impl fmt::Debug for LayoutRevalidationGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayoutRevalidationGuard")
            .field("source", &self.source)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl LayoutRevalidationGuard {
    pub fn new<P: PreparationInputPayload>(
        registry: &PreparationInputRegistry,
        key: PreparationInputKey<P>,
        source: FigureId,
        first: P,
    ) -> Result<Self, PreparationInputError> {
        let error = |kind| PreparationInputError {
            source: Some(source),
            payload_type: TypeId::of::<P>(),
            name: key.name,
            checkpoint: "layout request",
            kind,
        };
        let entry = registry
            .0
            .entries
            .get(&TypeId::of::<P>())
            .ok_or_else(|| error(PreparationInputErrorKind::UnknownRegistration))?;
        let units = first
            .retained_units()
            .checked_add(
                std::mem::size_of::<P>() as u64
                    + std::mem::size_of::<SnapshotValue>() as u64
                    + 4 * std::mem::size_of::<usize>() as u64,
            )
            .ok_or_else(|| error(PreparationInputErrorKind::AccountingOverflow))?;
        Ok(Self {
            source,
            registration: Arc::downgrade(entry),
            payload_type: entry.payload_type,
            name: entry.name,
            first: Arc::new(first),
            units,
        })
    }

    pub fn source(&self) -> FigureId {
        self.source
    }

    /// Admit the pending value before checking the original reader. A changed
    /// value rejects only the local candidate; errors retain their original kind.
    pub(crate) fn admit(
        &self,
        scene: &PreparationScene<'_>,
    ) -> Result<Option<InputProof>, TextError> {
        let error = |kind| PreparationInputError {
            source: Some(self.source),
            payload_type: self.payload_type,
            name: self.name,
            checkpoint: "layout request admission",
            kind,
        };
        scene.check(self.source)?;
        let entry = self.registration.upgrade().ok_or_else(|| {
            TextError::from(error(PreparationInputErrorKind::RetiredRegistration))
        })?;
        scene
            .budget()
            .allocate("preparation input snapshot", self.units)?;
        let proof_units = (std::mem::size_of::<InputProofRecord<PreparationInputFamily>>()
            + 2 * std::mem::size_of::<usize>()) as u64;
        scene
            .budget()
            .allocate("preparation input proof", proof_units)?;
        let reserve = |units| {
            entry
                .store
                .as_ref()
                .map(|store| {
                    let units = usize::try_from(units)
                        .map_err(|_| crate::text::TextContentError::AccountingOverflow)?;
                    store.reserve(units)
                })
                .transpose()
        };
        let first = PreparationInputSnapshot(Arc::new(SnapshotValue {
            value: self.first.clone(),
            registration: entry.identity,
            units: self.units,
            _retention: reserve(self.units)?,
        }));
        let proof = TypedInputProof(Arc::new(InputProofRecord {
            _retention: reserve(proof_units)?,
            root: scene.root(),
            source: self.source,
            payload_type: self.payload_type,
            name: self.name,
            registration: self.registration.clone(),
            first,
            constraints: scene.measurement_constraints(),
        }));
        let current = proof.read_current(scene, "layout request qualification")?;
        Ok((current == proof.first).then_some(proof))
    }
}

pub(crate) type InputProof = TypedInputProof<PreparationInputFamily>;
pub(crate) struct TypedInputProof<F: InputFamily>(Arc<InputProofRecord<F>>);
impl<F: InputFamily> Clone for TypedInputProof<F> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<F: InputFamily> std::ops::Deref for TypedInputProof<F> {
    type Target = InputProofRecord<F>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
pub(crate) struct InputProofRecord<F: InputFamily> {
    _retention: Option<crate::text::TextContentLease>,
    pub(crate) root: FigureId,
    source: FigureId,
    payload_type: TypeId,
    name: &'static str,
    registration: Weak<Registration<F>>,
    pub(crate) first: PreparationInputSnapshot,
    constraints: Option<(crate::MeasureConstraints, crate::MeasureConstraints)>,
}
impl<F: InputFamily> TypedInputProof<F> {
    /// Capture a new first value for a historical subject. The caller compares
    /// it with history, then merges it into this operation's first-value log.
    pub(crate) fn refreshed(
        &self,
        scene: &F::Scene<'_>,
        checkpoint: &'static str,
    ) -> Result<Self, F::Error> {
        let first = self.read_current(scene, checkpoint)?;
        let registration = self.registration.upgrade().ok_or_else(|| {
            self.error(checkpoint, PreparationInputErrorKind::RetiredRegistration)
        })?;
        let units = std::mem::size_of::<InputProofRecord<F>>() + 2 * std::mem::size_of::<usize>();
        F::budget(scene).allocate("refreshed input proof", units as u64)?;
        let retention = registration
            .store
            .as_ref()
            .map(|store| store.reserve(units))
            .transpose()?;
        Ok(Self(Arc::new(InputProofRecord {
            _retention: retention,
            root: self.root,
            source: self.source,
            payload_type: self.payload_type,
            name: self.name,
            registration: self.registration.clone(),
            first,
            constraints: self.constraints,
        })))
    }
    fn error(&self, checkpoint: &'static str, kind: PreparationInputErrorKind) -> F::Error {
        PreparationInputError {
            source: Some(self.source),
            payload_type: self.payload_type,
            name: self.name,
            checkpoint,
            kind,
        }
        .into()
    }
    pub(crate) fn same_subject(&self, other: &Self) -> bool {
        self.root == other.root
            && self.source == other.source
            && self.constraints == other.constraints
            && Weak::ptr_eq(&self.registration, &other.registration)
    }
    pub(crate) fn validate(
        &self,
        scene: &F::Scene<'_>,
        checkpoint: &'static str,
    ) -> Result<(), F::Error> {
        let current = self.read_current(scene, checkpoint)?;
        if current != self.first {
            return Err(self.error(
                checkpoint,
                PreparationInputErrorKind::Changed {
                    first: self.first.clone(),
                    current,
                },
            ));
        }
        Ok(())
    }

    pub(crate) fn read_current(
        &self,
        scene: &F::Scene<'_>,
        checkpoint: &'static str,
    ) -> Result<PreparationInputSnapshot, F::Error> {
        let _timing = F::budget(scene).time("preparation-input-validate");
        F::budget(scene).consume("preparation input verification", 1)?;
        let reader = self.registration.upgrade().ok_or_else(|| {
            self.error(checkpoint, PreparationInputErrorKind::RetiredRegistration)
        })?;
        let scoped = F::scope(scene, self.root, self.source, self.constraints)?;
        let current = reader.read(self.source, &scoped)?;
        F::complete_scope(scene, &scoped)?;
        F::budget(scene).check()?;
        Ok(current)
    }
}

/// Merge is all-or-nothing and never replaces the first value on conflict.
pub(crate) fn merge_proofs<F: InputFamily>(
    existing: &mut Vec<TypedInputProof<F>>,
    incoming: &[TypedInputProof<F>],
    budget: &super::PreparationBudgetHandle,
) -> Result<(), F::Error> {
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
        count as u64 * std::mem::size_of::<TypedInputProof<F>>() as u64,
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

    /// A second stage supplies no tree, Figure, measurement or preparation access.
    struct RestrictedFamily;
    struct RestrictedScene<'a> {
        root: FigureId,
        value: &'a AtomicU64,
        budget: super::super::PreparationBudgetHandle,
        path: InputReadPath,
    }
    impl InputFamily for RestrictedFamily {
        type Scene<'a> = RestrictedScene<'a>;
        type Error = TextError;
        fn root(scene: &RestrictedScene<'_>) -> FigureId {
            scene.root
        }
        fn constraints(
            _: &RestrictedScene<'_>,
        ) -> Option<(MeasureConstraints, MeasureConstraints)> {
            None
        }
        fn budget(scene: &RestrictedScene<'_>) -> super::super::PreparationBudgetHandle {
            scene.budget.clone()
        }
        fn read_path(scene: &RestrictedScene<'_>) -> InputReadPath {
            scene.path.clone()
        }
        fn check_reader(scene: &RestrictedScene<'_>) -> Result<(), TextError> {
            scene.budget.check()?;
            Ok(())
        }
        fn scope<'a>(
            scene: &RestrictedScene<'a>,
            root: FigureId,
            source: FigureId,
            _: Option<(MeasureConstraints, MeasureConstraints)>,
        ) -> Result<RestrictedScene<'a>, TextError>
        where
            Self: 'a,
        {
            if root != scene.root || source != root {
                return Err(TextError::InvalidMetric);
            }
            Ok(RestrictedScene {
                root,
                value: scene.value,
                budget: scene.budget.clone(),
                path: scene.path.clone(),
            })
        }
    }

    #[test]
    fn second_stage_shares_equality_retirement_and_retention_without_preparation_access() {
        let (_, root, _) = scene_fixture();
        let value = AtomicU64::new(1);
        let scene = RestrictedScene {
            root,
            value: &value,
            budget: super::super::PreparationBudgetHandle::default(),
            path: InputReadPath::default(),
        };
        let store = crate::text::TextContentStore::new(4096);
        let mut registry = TypedInputRegistry::<RestrictedFamily>::default();
        registry
            .register(
                VERSION,
                |_, scene: &RestrictedScene<'_>| Ok(Version(scene.value.load(Ordering::Relaxed))),
                Some(store.clone()),
            )
            .unwrap();
        let proof = registry.capture(VERSION, root, &scene).unwrap();
        let units = store.usage().unwrap().retained_units;
        assert!(units > 0);
        let evidence = proof.first.clone();
        assert_eq!(store.usage().unwrap().retained_units, units);
        let mut merged = Vec::new();
        merge_proofs(&mut merged, &[proof.clone()], &scene.budget).unwrap();
        proof.validate(&scene, "second stage").unwrap();
        value.store(2, Ordering::Relaxed);
        let current = registry.capture(VERSION, root, &scene).unwrap();
        assert!(merge_proofs(&mut merged, &[current], &scene.budget).is_err());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].first.value(VERSION).unwrap().0, 1);
        drop(registry);
        assert!(matches!(proof.validate(&scene, "retired"),
            Err(TextError::PreparationInput(error))
                if error.kind == PreparationInputErrorKind::RetiredRegistration));
        drop(merged);
        drop(proof);
        assert_eq!(evidence.value(VERSION).unwrap().0, 1);
        assert!(store.usage().unwrap().retained_units > 0);
        drop(evidence);
        assert_eq!(store.usage().unwrap().retained_units, 0);
    }

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

    #[test]
    fn recursive_input_is_rejected_before_reentering_reader_and_retry_is_clean() {
        let (tree, root, styles) = scene_fixture();
        let holder = Arc::new(std::sync::Mutex::new(None::<PreparationInputRegistry>));
        let weak = Arc::downgrade(&holder);
        let recursive = Arc::new(AtomicBool::new(true));
        let mode = recursive.clone();
        let calls = Arc::new(AtomicU64::new(0));
        let observed = calls.clone();
        let mut registry = PreparationInputRegistry::new();
        registry
            .register(VERSION, move |source, scene: &PreparationScene<'_>| {
                observed.fetch_add(1, Ordering::Relaxed);
                if mode.load(Ordering::Relaxed) {
                    let registry = weak
                        .upgrade()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .clone();
                    // Ignoring the nested result must not make this value admissible.
                    let _ = scene.input(&registry, VERSION, source);
                }
                Ok(Version(1))
            })
            .unwrap();
        *holder.lock().unwrap() = Some(registry.clone());
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        assert!(matches!(
            scene.input(&registry, VERSION, root),
            Err(TextError::PreparationInput(error))
                if error.kind == PreparationInputErrorKind::DependencyCycle
        ));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        recursive.store(false, Ordering::Relaxed);
        assert!(scene.input(&registry, VERSION, root).is_err());
        let retry = PreparationScene::new(&tree, root, &styles).unwrap();
        retry.input(&registry, VERSION, root).unwrap();
        retry.validate_input_proofs("retry").unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn ignored_nested_error_in_final_reader_cannot_publish_outer_value() {
        let (tree, root, styles) = scene_fixture();
        let failed = Arc::new(AtomicBool::new(false));
        let mode = failed.clone();
        let mut registry = PreparationInputRegistry::new();
        registry
            .register(VERSION, move |source, scene: &PreparationScene<'_>| {
                if mode.load(Ordering::Relaxed) {
                    let _ = scene.input(&PreparationInputRegistry::new(), VERSION, source);
                }
                Ok(Version(1))
            })
            .unwrap();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        scene.input(&registry, VERSION, root).unwrap();
        failed.store(true, Ordering::Relaxed);
        let error = scene.validate_input_proofs("final").unwrap_err();
        assert!(matches!(&error, TextError::PreparationInput(error)
            if error.kind == PreparationInputErrorKind::UnknownRegistration));
        failed.store(false, Ordering::Relaxed);
        assert_eq!(scene.validate_input_proofs("retry").unwrap_err(), error);
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
    fn deferred_guard_does_not_read_until_budgeted_admission() {
        let (tree, root, styles) = scene_fixture();
        let reads = Arc::new(AtomicU64::new(0));
        let observed = reads.clone();
        let mut registry = PreparationInputRegistry::new();
        registry
            .register(VERSION, move |_, _: &PreparationScene<'_>| {
                observed.fetch_add(1, Ordering::Relaxed);
                Ok(Version(1))
            })
            .unwrap();
        let guard = LayoutRevalidationGuard::new(&registry, VERSION, root, Version(1)).unwrap();
        assert_eq!(reads.load(Ordering::Relaxed), 0);
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let proof = guard.admit(&scene).unwrap().unwrap();
        assert_eq!(reads.load(Ordering::Relaxed), 1);
        assert_eq!(proof.first.value(VERSION).unwrap().0, 1);
        assert!(scene.input_proofs().unwrap().is_empty());
        scene.retain_input_proofs(&[proof]).unwrap();
        scene.validate_input_proofs("layout return").unwrap();
        assert_eq!(reads.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn deferred_guard_changes_before_adoption_upgrade_without_poisoning_scene() {
        let (tree, root, styles) = scene_fixture();
        let version = Arc::new(AtomicU64::new(2));
        let registry = registry(&version);
        let guard = LayoutRevalidationGuard::new(&registry, VERSION, root, Version(1)).unwrap();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        assert!(guard.admit(&scene).unwrap().is_none());
        assert!(scene.input_proofs().unwrap().is_empty());
        let current = scene.input(&registry, VERSION, root).unwrap();
        assert_eq!(current.value(VERSION).unwrap().0, 2);
        scene.validate_input_proofs("ordinary layout").unwrap();
    }

    #[test]
    fn adopted_guard_preserves_first_value_and_late_failure_is_sticky() {
        let (tree, root, styles) = scene_fixture();
        let version = Arc::new(AtomicU64::new(1));
        let registry = registry(&version);
        let guard = LayoutRevalidationGuard::new(&registry, VERSION, root, Version(1)).unwrap();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        let proof = guard.admit(&scene).unwrap().unwrap();
        scene.retain_input_proofs(&[proof]).unwrap();
        version.store(2, Ordering::Relaxed);
        let error = scene
            .validate_input_proofs("layout consumption")
            .unwrap_err();
        changed(&error, "layout consumption");
        version.store(1, Ordering::Relaxed);
        assert_eq!(scene.validate_input_proofs("publish").unwrap_err(), error);
    }

    #[test]
    fn deferred_guard_does_not_bind_a_replacement_registry() {
        let (tree, root, styles) = scene_fixture();
        let version = Arc::new(AtomicU64::new(1));
        let original = registry(&version);
        let guard = LayoutRevalidationGuard::new(&original, VERSION, root, Version(1)).unwrap();
        let clone = original.clone();
        drop(original);
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        assert!(guard.admit(&scene).unwrap().is_some());
        drop(clone);
        let _replacement = registry(&version);
        assert!(
            matches!(guard.admit(&scene), Err(TextError::PreparationInput(error))
            if error.kind == PreparationInputErrorKind::RetiredRegistration)
        );
    }

    #[test]
    fn deferred_guard_unknown_registration_is_a_typed_error() {
        let (_, root, _) = scene_fixture();
        let error = LayoutRevalidationGuard::new(
            &PreparationInputRegistry::new(),
            VERSION,
            root,
            Version(1),
        )
        .unwrap_err();
        assert_eq!(error.source, Some(root));
        assert_eq!(error.payload_type, TypeId::of::<Version>());
        assert_eq!(error.kind, PreparationInputErrorKind::UnknownRegistration);
    }

    #[test]
    fn deferred_guard_budget_failure_precedes_the_reader() {
        let (tree, root, styles) = scene_fixture();
        let reads = Arc::new(AtomicU64::new(0));
        let observed = reads.clone();
        let mut registry = PreparationInputRegistry::new();
        registry
            .register(VERSION, move |_, _: &PreparationScene<'_>| {
                observed.fetch_add(1, Ordering::Relaxed);
                Ok(Version(1))
            })
            .unwrap();
        let guard = LayoutRevalidationGuard::new(&registry, VERSION, root, Version(1)).unwrap();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        assert!(scene.budget().allocate("exhaust", u64::MAX).is_err());
        assert!(guard.admit(&scene).is_err());
        assert_eq!(reads.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn deferred_guard_reader_failure_is_not_a_local_mismatch() {
        let (tree, root, styles) = scene_fixture();
        let mut registry = PreparationInputRegistry::new();
        registry
            .register(VERSION, |_, _: &PreparationScene<'_>| {
                Err(TextError::InvalidMetric)
            })
            .unwrap();
        let guard = LayoutRevalidationGuard::new(&registry, VERSION, root, Version(1)).unwrap();
        let scene = PreparationScene::new(&tree, root, &styles).unwrap();
        assert!(matches!(guard.admit(&scene), Err(TextError::InvalidMetric)));
    }

    #[test]
    fn deferred_guard_rejects_accounting_overflow_before_pending_allocation() {
        #[derive(Eq, PartialEq)]
        struct Oversized;
        impl PreparationInputPayload for Oversized {
            fn retained_units(&self) -> u64 {
                u64::MAX
            }
        }
        let (_, root, _) = scene_fixture();
        let key = PreparationInputKey::<Oversized>::new("oversized");
        let mut registry = PreparationInputRegistry::new();
        registry
            .register(key, |_, _: &PreparationScene<'_>| Ok(Oversized))
            .unwrap();
        let error = LayoutRevalidationGuard::new(&registry, key, root, Oversized).unwrap_err();
        assert_eq!(error.kind, PreparationInputErrorKind::AccountingOverflow);
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
