use super::{
    AnimationBudgetKind, AnimationError, DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS,
    DEFAULT_MAX_ACTIVE_ANIMATIONS,
};

/// Independent upper bounds for one complete geometry materialization.
///
/// A plan reserves its proven upper bound, rather than borrowing a balance according to
/// provider execution order. All dimensions use checked arithmetic.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AnimationWork {
    /// Distinct geometry nodes, including read-only ancestors.
    pub nodes: usize,
    /// Declared dependency edges.
    pub edges: usize,
    /// Motion and derived provider calls.
    pub calls: usize,
    /// Derived geometry output items.
    pub items: usize,
    /// Owned source and peak presentation storage in bytes.
    pub bytes: usize,
    /// Matrix, clip and envelope operations.
    pub operations: usize,
}

impl AnimationWork {
    pub(crate) fn combine(self, retired: Self, candidate: Self) -> Result<Self, AnimationError> {
        let mut result = Self::default();
        for ((live, retired), (candidate, (output, kind))) in
            self.dimensions().into_iter().zip(retired.dimensions()).zip(
                candidate
                    .dimensions()
                    .into_iter()
                    .zip(result.dimensions_mut()),
            )
        {
            *output = live
                .checked_sub(retired)
                .and_then(|n| n.checked_add(candidate))
                .ok_or(AnimationError::BudgetExceeded(kind))?;
        }
        Ok(result)
    }

    pub(crate) fn validate(self, limit: Self) -> Result<(), AnimationError> {
        for ((actual, limit), kind) in self
            .dimensions()
            .into_iter()
            .zip(limit.dimensions())
            .zip(Self::kinds())
        {
            if actual > limit {
                return Err(AnimationError::BudgetExceeded(kind));
            }
        }
        Ok(())
    }

    fn dimensions(self) -> [usize; 6] {
        [
            self.nodes,
            self.edges,
            self.calls,
            self.items,
            self.bytes,
            self.operations,
        ]
    }

    fn dimensions_mut(&mut self) -> [(&mut usize, AnimationBudgetKind); 6] {
        let [nodes, edges, calls, items, bytes, operations] = Self::kinds();
        [
            (&mut self.nodes, nodes),
            (&mut self.edges, edges),
            (&mut self.calls, calls),
            (&mut self.items, items),
            (&mut self.bytes, bytes),
            (&mut self.operations, operations),
        ]
    }

    fn kinds() -> [AnimationBudgetKind; 6] {
        [
            AnimationBudgetKind::GeometryNodes,
            AnimationBudgetKind::DependencyEdges,
            AnimationBudgetKind::ProviderCalls,
            AnimationBudgetKind::GeometryItems,
            AnimationBudgetKind::SnapshotBytes,
            AnimationBudgetKind::GeometryOperations,
        ]
    }

    pub(crate) fn for_tracks(tracks: usize) -> Self {
        Self {
            calls: tracks,
            ..Self::default()
        }
    }
}

/// Default finite node capacity shared by admission and staging.
pub const DEFAULT_MAX_ANIMATION_GEOMETRY_NODES: usize = 65_536;
/// Default finite owned geometry storage capacity.
pub const DEFAULT_MAX_ANIMATION_GEOMETRY_BYTES: usize = 64 * 1024 * 1024;
/// Three geometry passes plus root/ancestor normalization. Each selected node has at
/// most five compositions, eight self/client corner transforms, two clip intersections
/// and one self envelope; selected edges add four corner transforms, one clip and one
/// envelope. Unselected ancestor child-order entries are charged separately as edges.
pub(crate) const GEOMETRY_OPERATIONS_PER_NODE: usize = 96;

fn default_work() -> AnimationWork {
    AnimationWork {
        nodes: DEFAULT_MAX_ANIMATION_GEOMETRY_NODES,
        edges: DEFAULT_MAX_ANIMATION_GEOMETRY_NODES,
        calls: DEFAULT_MAX_ANIMATION_GEOMETRY_NODES,
        items: DEFAULT_MAX_ANIMATION_GEOMETRY_NODES,
        bytes: DEFAULT_MAX_ANIMATION_GEOMETRY_BYTES,
        operations: DEFAULT_MAX_ANIMATION_GEOMETRY_NODES * GEOMETRY_OPERATIONS_PER_NODE,
    }
}

/// Runtime limits for timeline ownership and candidate sampling.
///
/// Active tracks stay reserved until their complete owner terminates. Staging is independent:
/// preparing a replacement never borrows the old owner's reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnimationLimits {
    /// Maximum number of active timeline owners.
    pub active_animations: usize,
    /// Maximum number of tracks reserved by active owners.
    pub active_tracks: usize,
    /// Maximum number of candidate tracks prepared by one admission or retarget.
    pub staging_tracks: usize,
    /// Sum of proven per-owner geometry and sampling bounds.
    pub active_work: AnimationWork,
    /// Independent capacity available to a replacement candidate.
    pub staging_work: AnimationWork,
    /// Independent collection bytes reserved by externally retained geometry captures.
    pub capture_bytes: usize,
    /// Independent bytes retained by submission, in-flight, and recovery ownership.
    pub submission_bytes: usize,
}

impl Default for AnimationLimits {
    fn default() -> Self {
        Self {
            active_animations: DEFAULT_MAX_ACTIVE_ANIMATIONS,
            active_tracks: DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS,
            staging_tracks: DEFAULT_MAX_ACTIVE_ANIMATION_TRACKS,
            active_work: default_work(),
            staging_work: default_work(),
            capture_bytes: DEFAULT_MAX_ANIMATION_GEOMETRY_BYTES,
            submission_bytes: DEFAULT_MAX_ANIMATION_GEOMETRY_BYTES,
        }
    }
}

/// Pull diagnostics for live timeline reservations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AnimationReservations {
    /// Number of owners holding active capacity.
    pub active_animations: usize,
    /// Sum of the track bounds reserved at admission.
    pub active_tracks: usize,
    /// Sum of the owners' proven materialization bounds.
    pub work: AnimationWork,
    /// Collection bytes retained by owned captures, including captures sharing the same geometry.
    pub capture_bytes: usize,
    /// Bytes retained until the last submission or recovery owner releases its lease.
    pub submission_bytes: usize,
}

impl AnimationLimits {
    pub(crate) fn validate_live(self, live: AnimationReservations) -> Result<(), AnimationError> {
        self.validate_replacement(live, AnimationReservations::default(), 0, false)?;
        if live.capture_bytes > self.capture_bytes || live.submission_bytes > self.submission_bytes
        {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::SnapshotBytes,
            ));
        }
        live.work.validate(self.active_work)
    }

    pub(crate) fn validate_staging(self, tracks: usize) -> Result<(), AnimationError> {
        if tracks > self.staging_tracks {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::StagingTracks,
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_replacement(
        self,
        live: AnimationReservations,
        retired: AnimationReservations,
        new_tracks: usize,
        new_owner: bool,
    ) -> Result<(), AnimationError> {
        let owners = live
            .active_animations
            .checked_sub(retired.active_animations)
            .and_then(|value| value.checked_add(usize::from(new_owner)))
            .ok_or(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveAnimations,
            ))?;
        let tracks = live
            .active_tracks
            .checked_sub(retired.active_tracks)
            .and_then(|value| value.checked_add(new_tracks))
            .ok_or(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveTracks,
            ))?;
        if owners > self.active_animations {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveAnimations,
            ));
        }
        if tracks > self.active_tracks {
            return Err(AnimationError::BudgetExceeded(
                AnimationBudgetKind::ActiveTracks,
            ));
        }
        Ok(())
    }
}
