use novadraw::geometry::{Affine2D, Point};
use std::{any::TypeId, sync::Arc};
use uuid::Uuid;

use super::{ConstraintGeometry, ContainerEditError};
use crate::{
    Command, EditPartId, EditorNamespace, FeedbackVisual, InteractionRevision, ModelAdapter,
    ModelRevision, RequestKey, RequestPayload, RequestProtocol,
};

/// Opaque version of one host's installed policies and controlled configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyGeneration(pub(crate) Uuid);

/// Unique identity of one recomputation, never reused across pointer updates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateId(pub(crate) Uuid);

/// Validated source with its original model containment relation.
///
/// ```compile_fail
/// use novadraw_editor::container::MoveSource;
/// fn forge(source: &mut MoveSource<u32>) {
///     source.model = 0;
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoveSource<I> {
    pub(crate) part: EditPartId,
    pub(crate) model: I,
    pub(crate) parent: EditPartId,
    pub(crate) parent_model: I,
    pub(crate) index: usize,
}

impl<I: Copy> MoveSource<I> {
    /// Returns the original live source Part.
    pub const fn part(&self) -> EditPartId {
        self.part
    }
    /// Returns the stable source model identity.
    pub const fn model(&self) -> I {
        self.model
    }
    /// Returns the original parent Part.
    pub const fn parent(&self) -> EditPartId {
        self.parent
    }
    /// Returns the original parent model identity.
    pub const fn parent_model(&self) -> I {
        self.parent_model
    }
    /// Returns the original child index.
    pub const fn index(&self) -> usize {
        self.index
    }
}

/// Immutable, nonempty source snapshot created by the checked Viewer service.
///
/// ```compile_fail
/// use novadraw_editor::{ModelRevision, container::MoveBatch};
/// fn forge(batch: &mut MoveBatch<u32>, revision: ModelRevision) {
///     batch.revision = revision;
/// }
/// ```
#[derive(Clone, Debug)]
pub struct MoveBatch<I> {
    pub(crate) namespace: EditorNamespace,
    pub(crate) revision: ModelRevision,
    pub(crate) interaction: InteractionRevision,
    pub(crate) input: Arc<[EditPartId]>,
    pub(crate) sources: Arc<[MoveSource<I>]>,
    pub(crate) anchor: usize,
    pub(crate) grab: Point,
    pub(crate) grab_to_parent: Affine2D,
    pub(crate) geometry: Arc<[ConstraintGeometry]>,
    pub(crate) generations: Arc<[(EditPartId, PolicyGeneration)]>,
}

impl<I: Copy> MoveBatch<I> {
    /// Returns the owning Viewer namespace.
    pub const fn namespace(&self) -> EditorNamespace {
        self.namespace
    }
    /// Returns the baseline model revision.
    pub const fn revision(&self) -> ModelRevision {
        self.revision
    }
    /// Returns the initiating interaction revision.
    pub const fn interaction_revision(&self) -> InteractionRevision {
        self.interaction
    }
    /// Returns the full original ordered input before ancestor deduplication.
    pub fn input(&self) -> &[EditPartId] {
        &self.input
    }
    /// Returns the ordered containment operation set.
    pub fn sources(&self) -> &[MoveSource<I>] {
        &self.sources
    }
    /// Returns the unique drag anchor.
    pub fn anchor(&self) -> &MoveSource<I> {
        &self.sources[self.anchor]
    }
    /// Returns the grab point in the anchor constraint-local or explicit parent domain.
    pub const fn grab_point(&self) -> Point {
        self.grab
    }
    /// Returns the grab-domain to original parent-domain map.
    pub const fn grab_to_parent(&self) -> Affine2D {
        self.grab_to_parent
    }
}

/// Current target and transformed constraints, created from a single surface translation.
///
/// ```compile_fail
/// use novadraw_editor::{EditPartId, container::MoveCandidate};
/// fn forge(candidate: &mut MoveCandidate<u32>, target: EditPartId) {
///     candidate.target = target;
/// }
/// ```
#[derive(Clone, Debug)]
pub struct MoveCandidate<I> {
    pub(crate) id: CandidateId,
    pub(crate) batch: MoveBatch<I>,
    pub(crate) target: EditPartId,
    pub(crate) target_model: I,
    pub(crate) generation: PolicyGeneration,
    pub(crate) interaction: InteractionRevision,
    pub(crate) geometry: Arc<[ConstraintGeometry]>,
    pub(crate) pointer: Point,
    pub(crate) target_to_surface: Affine2D,
    pub(crate) translation: Affine2D,
    pub(crate) source_maps: Arc<[Affine2D]>,
}

impl<I: Copy + Eq> MoveCandidate<I> {
    /// Returns this computation's opaque identity.
    pub const fn id(&self) -> CandidateId {
        self.id
    }
    /// Returns the checked source snapshot.
    pub const fn batch(&self) -> &MoveBatch<I> {
        &self.batch
    }
    /// Returns the actual aggregation host.
    pub const fn target(&self) -> EditPartId {
        self.target
    }
    /// Returns the target's model identity.
    pub const fn target_model(&self) -> I {
        self.target_model
    }
    /// Returns the current host policy generation.
    pub const fn generation(&self) -> PolicyGeneration {
        self.generation
    }
    /// Returns this pointer update's interaction revision.
    pub const fn interaction_revision(&self) -> InteractionRevision {
        self.interaction
    }
    /// Returns constraints in the target child-content domain in source order.
    pub fn geometry(&self) -> &[ConstraintGeometry] {
        &self.geometry
    }
    /// Applies the default XY representation strategy with an explicit parent-domain tolerance.
    pub fn xy_constraints(
        &self,
        tolerance: super::GeometryTolerance,
    ) -> Result<Vec<novadraw::geometry::Rectangle>, ContainerEditError> {
        self.geometry
            .iter()
            .map(|geometry| geometry.xy_bounds(tolerance))
            .collect()
    }
    /// Returns the pointer in the target parent domain.
    pub const fn pointer(&self) -> Point {
        self.pointer
    }
    /// Returns the map used to project target-parent feedback to surface.
    pub const fn target_to_surface(&self) -> Affine2D {
        self.target_to_surface
    }
    /// Returns the one surface translation shared by every source.
    pub const fn translation(&self) -> Affine2D {
        self.translation
    }
    /// Iterates sources whose parent is already this target.
    pub fn local_sources(&self) -> impl Iterator<Item = &MoveSource<I>> {
        self.batch
            .sources
            .iter()
            .filter(|source| source.parent == self.target)
    }
    /// Iterates sources entering this target from a different parent.
    pub fn incoming_sources(&self) -> impl Iterator<Item = &MoveSource<I>> {
        self.batch
            .sources
            .iter()
            .filter(|source| source.parent != self.target)
    }
}

/// Finite semantic operations checked by the shared permission request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionKind {
    /// Change constraints or order within the existing parent.
    LocalModification,
    /// Remove a precise ordered subbatch from its existing parent.
    Removal,
}

/// Framework-created permission envelope for exactly one host and subbatch.
///
/// ```compile_fail
/// use novadraw_editor::{EditPartId, container::PermissionQuery};
/// fn forge(query: &mut PermissionQuery<u32>, host: EditPartId) {
///     query.host = host;
/// }
/// ```
#[derive(Clone, Debug)]
pub struct PermissionQuery<I> {
    pub(crate) candidate: CandidateId,
    pub(crate) full_candidate: Arc<MoveCandidate<I>>,
    pub(crate) protocol: TypeId,
    pub(crate) host: EditPartId,
    pub(crate) kind: PermissionKind,
    pub(crate) sources: Arc<[MoveSource<I>]>,
    pub(crate) revision: ModelRevision,
    pub(crate) generation: PolicyGeneration,
    pub(crate) interaction: InteractionRevision,
}

impl<I: Copy> PermissionQuery<I> {
    /// Returns the candidate authorized by this query.
    pub const fn candidate(&self) -> CandidateId {
        self.candidate
    }
    /// Returns the complete checked target and geometry served by this precise subbatch.
    pub fn move_candidate(&self) -> &MoveCandidate<I> {
        &self.full_candidate
    }
    /// Checks the active typed request family without diagnostic string comparisons.
    pub fn is_for<R: 'static>(&self) -> bool {
        self.protocol == TypeId::of::<R>()
    }
    /// Returns the actual host receiving this query.
    pub const fn host(&self) -> EditPartId {
        self.host
    }
    /// Returns the local-modification or removal operation.
    pub const fn kind(&self) -> PermissionKind {
        self.kind
    }
    /// Returns precisely the ordered metadata source subbatch.
    pub fn sources(&self) -> &[MoveSource<I>] {
        &self.sources
    }
    /// Returns the model revision checked by the framework.
    pub const fn revision(&self) -> ModelRevision {
        self.revision
    }
    /// Returns the issuing host's configuration version.
    pub const fn generation(&self) -> PolicyGeneration {
        self.generation
    }
    /// Returns the current interaction revision.
    pub const fn interaction_revision(&self) -> InteractionRevision {
        self.interaction
    }
}

/// A policy's decision, carrying no reusable authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PermissionDecision {
    /// Accept precisely the supplied permission query.
    Allow,
    /// Explicitly refuse this query and stop ancestor fallback.
    Reject(String),
}

/// Shared open-registry marker used by every built-in and external source parent.
pub struct CheckContainerPermission;

impl<A: ModelAdapter + 'static> RequestProtocol<A> for CheckContainerPermission {
    type Payload = PermissionQuery<A::ModelId>;
    type Contribution = PermissionDecision;
    fn key() -> RequestKey<Self> {
        RequestKey::new("check-container-permission")
    }
}

/// Framework-issued proof bound to the actual aggregation host and exact query.
///
/// ```compile_fail
/// use novadraw_editor::container::{PermissionQuery, PermissionReceipt};
/// fn forge(query: PermissionQuery<u32>) {
///     let receipt = PermissionReceipt { query };
/// }
/// ```
#[derive(Clone, Debug)]
pub struct PermissionReceipt<I> {
    pub(crate) query: PermissionQuery<I>,
}

impl<I: Copy> PermissionReceipt<I> {
    /// Returns the actual query authorized by checked policy aggregation.
    pub const fn query(&self) -> &PermissionQuery<I> {
        &self.query
    }
}

/// Owned declaration submitted by an application plan; not an authorization.
///
/// ```compile_fail
/// use novadraw_editor::container::MovePlanHeader;
/// fn forge(header: &mut MovePlanHeader<u32>) {
///     header.sources.clear();
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MovePlanHeader<I> {
    pub(crate) candidate: CandidateId,
    pub(crate) target: EditPartId,
    pub(crate) target_model: I,
    pub(crate) revision: ModelRevision,
    pub(crate) generation: PolicyGeneration,
    pub(crate) interaction: InteractionRevision,
    pub(crate) sources: Vec<MoveSource<I>>,
}

impl<I: Copy + Eq> MovePlanHeader<I> {
    /// Copies the complete declaration from the current candidate.
    pub fn from_candidate(candidate: &MoveCandidate<I>) -> Self {
        Self {
            candidate: candidate.id,
            target: candidate.target,
            target_model: candidate.target_model,
            revision: candidate.batch.revision,
            generation: candidate.generation,
            interaction: candidate.interaction,
            sources: candidate.batch.sources.to_vec(),
        }
    }
    /// Returns the declared candidate identity.
    pub const fn candidate(&self) -> CandidateId {
        self.candidate
    }
    /// Returns the declared target.
    pub const fn target(&self) -> EditPartId {
        self.target
    }
    /// Returns the declared target model.
    pub const fn target_model(&self) -> I {
        self.target_model
    }
    /// Returns the declared model revision.
    pub const fn revision(&self) -> ModelRevision {
        self.revision
    }
    /// Returns the declared target generation.
    pub const fn generation(&self) -> PolicyGeneration {
        self.generation
    }
    /// Returns the declared interaction revision.
    pub const fn interaction_revision(&self) -> InteractionRevision {
        self.interaction
    }
    /// Returns the exact declared operation sequence and original parents.
    pub fn sources(&self) -> &[MoveSource<I>] {
        &self.sources
    }
}

/// Application-owned read-only plan with a single atomic model command.
pub trait MovePlan<A: ModelAdapter>: 'static {
    /// Returns the plan's declaration; the checker reads and owns one copy.
    fn header(&self) -> &MovePlanHeader<A::ModelId>;
    /// Creates feedback already projected to the existing scaled or surface layer.
    fn feedback(&self) -> Result<Vec<FeedbackVisual>, ContainerEditError>;
    /// Builds one model-only command after cleanup and full framework validation.
    fn build_command(self, model: &A) -> Result<Option<Box<dyn Command<A>>>, ContainerEditError>;
}

/// Open protocol fixed for the complete lifetime of one container gesture.
pub trait ContainerMoveProtocol<A: ModelAdapter>: 'static {
    /// Application-specific immutable request input.
    type Payload: RequestPayload;
    /// Application-specific complete move plan.
    type Plan: MovePlan<A>;
    /// Request family installed through the ordinary typed policy registry.
    type Request: RequestProtocol<A, Payload = Self::Payload, Contribution = Self::Plan>;
    /// Creates input for one checked target candidate.
    fn payload(candidate: &MoveCandidate<A::ModelId>) -> Self::Payload;
    /// Reads model constraints, independent of visual extents.
    fn source_geometry(
        model: &A,
        source: &MoveSource<A::ModelId>,
    ) -> Result<ConstraintGeometry, ContainerEditError>;
}

/// Full framework validation result, constructible only by the shared checker.
///
/// ```compile_fail
/// use novadraw_editor::{ModelAdapter, container::{
///     CheckedMovePlan, ContainerMoveProtocol, MovePlanHeader, MoveCandidate, PermissionReceipt,
/// }};
/// fn forge<A: ModelAdapter, P: ContainerMoveProtocol<A>>(
///     plan: P::Plan, header: MovePlanHeader<A::ModelId>,
///     receipts: Vec<PermissionReceipt<A::ModelId>>, candidate: MoveCandidate<A::ModelId>,
/// ) {
///     let checked: CheckedMovePlan<A, P> = CheckedMovePlan { plan, header, receipts, candidate };
/// }
/// ```
pub struct CheckedMovePlan<A: ModelAdapter, P: ContainerMoveProtocol<A>> {
    pub(crate) plan: P::Plan,
    pub(crate) header: MovePlanHeader<A::ModelId>,
    pub(crate) receipts: Vec<PermissionReceipt<A::ModelId>>,
    pub(crate) candidate: MoveCandidate<A::ModelId>,
}

impl<A: ModelAdapter, P: ContainerMoveProtocol<A>> CheckedMovePlan<A, P> {
    /// Returns the single owned declaration used during validation.
    pub const fn header(&self) -> &MovePlanHeader<A::ModelId> {
        &self.header
    }
    /// Returns framework-issued authorizations for this candidate.
    pub fn receipts(&self) -> &[PermissionReceipt<A::ModelId>] {
        &self.receipts
    }
}
