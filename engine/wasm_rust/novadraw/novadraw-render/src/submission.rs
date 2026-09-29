use std::sync::Arc;

use novadraw_geometry::Rectangle;
use uuid::Uuid;

use crate::command::{ImageData, RenderCommand};

#[derive(Debug, Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FrameId(u64);

impl FrameId {
    pub const INITIAL: Self = Self(1);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Self {
        match self.0.checked_add(1) {
            Some(value) => Self(value),
            None => Self::INITIAL,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackendSessionId {
    runtime_namespace: Uuid,
    generation: u64,
}

impl BackendSessionId {
    pub const fn initial(runtime_namespace: Uuid) -> Self {
        Self {
            runtime_namespace,
            generation: 1,
        }
    }

    pub const fn new(runtime_namespace: Uuid, generation: u64) -> Option<Self> {
        if generation == 0 {
            None
        } else {
            Some(Self {
                runtime_namespace,
                generation,
            })
        }
    }

    pub const fn runtime_namespace(self) -> Uuid {
        self.runtime_namespace
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn next(self) -> Option<Self> {
        match self.generation.checked_add(1) {
            Some(generation) => Some(Self {
                runtime_namespace: self.runtime_namespace,
                generation,
            }),
            None => None,
        }
    }
}

impl Default for BackendSessionId {
    fn default() -> Self {
        Self::initial(Uuid::nil())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceInfo {
    pub logical_width: f64,
    pub logical_height: f64,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub scale_factor: f64,
}

impl Default for SurfaceInfo {
    fn default() -> Self {
        Self {
            logical_width: 0.0,
            logical_height: 0.0,
            pixel_width: 0,
            pixel_height: 0,
            scale_factor: 1.0,
        }
    }
}

impl SurfaceInfo {
    pub fn is_renderable(self) -> bool {
        self.pixel_width > 0
            && self.pixel_height > 0
            && self.logical_width.is_finite()
            && self.logical_width > 0.0
            && self.logical_height.is_finite()
            && self.logical_height > 0.0
            && self.scale_factor.is_finite()
            && self.scale_factor > 0.0
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceId {
    namespace: Uuid,
    generation_key: u64,
}

impl ResourceId {
    pub const fn new(namespace: Uuid, generation_key: u64) -> Self {
        Self {
            namespace,
            generation_key,
        }
    }

    pub const fn namespace(self) -> Uuid {
        self.namespace
    }

    pub const fn generation_key(self) -> u64 {
        self.generation_key
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontData {
    bytes: Arc<Vec<u8>>,
}

impl FontData {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Arc::new(bytes),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[doc(hidden)]
    pub fn shared_bytes(&self) -> Arc<Vec<u8>> {
        Arc::clone(&self.bytes)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResourcePayload {
    Image(Arc<ImageData>),
    Font(Arc<FontData>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceUpdate {
    pub id: ResourceId,
    pub revision: u64,
    pub payload: ResourcePayload,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResourceDelta {
    pub ops: Vec<ResourceOp>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceOp {
    Upsert(ResourceUpdate),
    Remove(ResourceId),
}

impl ResourceDelta {
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn extend(&mut self, other: Self) {
        self.ops.extend(other.ops);
    }

    pub fn upsert(&mut self, update: ResourceUpdate) {
        self.ops.push(ResourceOp::Upsert(update));
    }

    pub fn remove(&mut self, id: ResourceId) {
        self.ops.push(ResourceOp::Remove(id));
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResourceSnapshot {
    pub ready: Vec<ResourceUpdate>,
}

impl ResourceSnapshot {
    pub fn is_empty(&self) -> bool {
        self.ready.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceSync {
    Delta(ResourceDelta),
    Snapshot(ResourceSnapshot),
}

impl Default for ResourceSync {
    fn default() -> Self {
        Self::Delta(ResourceDelta::default())
    }
}

impl ResourceSync {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Delta(delta) => delta.is_empty(),
            Self::Snapshot(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendSessionDecision {
    Initialize,
    Continue,
    Replace,
    RejectStale,
    RejectMissingSnapshot,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BackendSessionGate {
    active: Option<BackendSessionId>,
}

impl BackendSessionGate {
    pub const fn active_session(self) -> Option<BackendSessionId> {
        self.active
    }

    pub fn accept(
        &mut self,
        incoming: BackendSessionId,
        resources: &ResourceSync,
    ) -> BackendSessionDecision {
        let transition = match self.active {
            None => BackendSessionDecision::Initialize,
            Some(active) if incoming == active => BackendSessionDecision::Continue,
            Some(active)
                if incoming.runtime_namespace() == active.runtime_namespace()
                    && incoming.generation() < active.generation() =>
            {
                BackendSessionDecision::RejectStale
            }
            Some(_) => BackendSessionDecision::Replace,
        };
        match transition {
            BackendSessionDecision::Initialize | BackendSessionDecision::Replace
                if !matches!(resources, ResourceSync::Snapshot(_)) =>
            {
                BackendSessionDecision::RejectMissingSnapshot
            }
            BackendSessionDecision::Initialize | BackendSessionDecision::Replace => {
                self.active = Some(incoming);
                transition
            }
            _ => transition,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum DamageMode {
    #[default]
    None,
    Full,
    Partial,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DamageSet {
    mode: DamageMode,
    union: Option<Rectangle>,
    regions: Vec<Rectangle>,
}

impl DamageSet {
    pub fn is_empty(&self) -> bool {
        self.mode == DamageMode::None
    }

    pub fn is_full(&self) -> bool {
        self.mode == DamageMode::Full
    }

    pub fn mode(&self) -> DamageMode {
        self.mode
    }

    pub fn union(&self) -> Option<Rectangle> {
        self.union
    }

    pub fn regions(&self) -> &[Rectangle] {
        &self.regions
    }

    pub fn set_full(&mut self) {
        self.mode = DamageMode::Full;
        self.union = None;
        self.regions.clear();
    }

    pub fn set_union(&mut self, rect: Rectangle) {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            self.clear();
            return;
        }
        self.mode = DamageMode::Partial;
        self.union = Some(rect);
        self.regions.clear();
        self.regions.push(rect);
    }

    pub fn set_regions(&mut self, regions: Vec<Rectangle>) {
        let filtered: Vec<Rectangle> = regions
            .into_iter()
            .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
            .collect();

        if filtered.is_empty() {
            self.clear();
            return;
        }

        self.mode = DamageMode::Partial;
        let union = filtered
            .iter()
            .copied()
            .reduce(|acc, rect| acc.union(rect))
            .expect("filtered regions should not be empty");

        self.union = Some(union);
        self.regions = filtered;
    }

    pub fn clear(&mut self) {
        self.mode = DamageMode::None;
        self.union = None;
        self.regions.clear();
    }
}

#[derive(Debug, Clone)]
pub struct RenderSubmission {
    pub commands: Vec<RenderCommand>,
    pub damage: DamageSet,
    pub resources: ResourceSync,
    pub surface: SurfaceInfo,
    pub session_id: BackendSessionId,
    pub frame_id: FrameId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_mode_distinguishes_none_full_and_partial() {
        let mut damage = DamageSet::default();
        assert_eq!(damage.mode(), DamageMode::None);
        assert!(damage.is_empty());

        damage.set_full();
        assert_eq!(damage.mode(), DamageMode::Full);
        assert!(damage.is_full());
        assert!(!damage.is_empty());

        damage.set_union(Rectangle::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(damage.mode(), DamageMode::Partial);
        assert!(!damage.is_full());

        damage.clear();
        assert_eq!(damage.mode(), DamageMode::None);
        assert!(damage.is_empty());
    }

    #[test]
    fn surface_info_defaults_to_a_valid_logical_scale() {
        let surface = SurfaceInfo::default();

        assert_eq!(surface.scale_factor, 1.0);
        assert_eq!(surface.logical_width, 0.0);
        assert_eq!(surface.pixel_width, 0);
        assert!(!surface.is_renderable());
    }

    #[test]
    fn frame_ids_are_monotonic_and_skip_zero_after_wrap() {
        assert_eq!(FrameId::INITIAL.get(), 1);
        assert_eq!(FrameId::INITIAL.next().get(), 2);
        assert_eq!(FrameId::new(u64::MAX).next(), FrameId::INITIAL);
    }

    #[test]
    fn resource_delta_preserves_append_order() {
        let namespace = Uuid::nil();
        let image = ResourceUpdate {
            id: ResourceId::new(namespace, 1),
            revision: 1,
            payload: ResourcePayload::Image(Arc::new(ImageData::from_rgba(
                1,
                1,
                vec![255; 4],
                1.0,
            ))),
        };
        let font = ResourceUpdate {
            id: ResourceId::new(namespace, 3),
            revision: 1,
            payload: ResourcePayload::Font(Arc::new(FontData::new(vec![1, 2, 3]))),
        };
        let mut delta = ResourceDelta::default();
        delta.upsert(image.clone());
        delta.remove(ResourceId::new(namespace, 2));
        let mut later = ResourceDelta::default();
        later.upsert(font.clone());
        later.remove(ResourceId::new(namespace, 4));
        delta.extend(later);

        assert_eq!(
            delta.ops,
            vec![
                ResourceOp::Upsert(image),
                ResourceOp::Remove(ResourceId::new(namespace, 2)),
                ResourceOp::Upsert(font),
                ResourceOp::Remove(ResourceId::new(namespace, 4)),
            ]
        );
    }

    #[test]
    fn backend_session_ids_are_non_zero_and_do_not_wrap() {
        let namespace = Uuid::nil();
        assert_eq!(BackendSessionId::new(namespace, 0), None);
        let initial = BackendSessionId::initial(namespace);
        assert_eq!(initial.generation(), 1);
        assert_eq!(initial.next().unwrap().generation(), 2);
        assert_eq!(
            BackendSessionId::new(namespace, u64::MAX).unwrap().next(),
            None
        );
    }

    #[test]
    fn empty_snapshot_still_represents_cache_replacement() {
        assert!(!ResourceSync::Snapshot(ResourceSnapshot::default()).is_empty());
        assert!(ResourceSync::Delta(ResourceDelta::default()).is_empty());
    }

    #[test]
    fn backend_session_gate_requires_snapshot_baselines_and_rejects_stale_generations() {
        let first = BackendSessionId::initial(Uuid::from_u128(1));
        let second = first.next().unwrap();
        let other = BackendSessionId::initial(Uuid::from_u128(2));
        let delta = ResourceSync::Delta(ResourceDelta::default());
        let snapshot = ResourceSync::Snapshot(ResourceSnapshot::default());
        let mut gate = BackendSessionGate::default();

        assert_eq!(
            gate.accept(first, &delta),
            BackendSessionDecision::RejectMissingSnapshot
        );
        assert_eq!(gate.active_session(), None);
        assert_eq!(
            gate.accept(first, &snapshot),
            BackendSessionDecision::Initialize
        );
        assert_eq!(gate.accept(first, &delta), BackendSessionDecision::Continue);
        assert_eq!(
            gate.accept(second, &delta),
            BackendSessionDecision::RejectMissingSnapshot
        );
        assert_eq!(gate.active_session(), Some(first));
        assert_eq!(
            gate.accept(second, &snapshot),
            BackendSessionDecision::Replace
        );
        assert_eq!(
            gate.accept(first, &snapshot),
            BackendSessionDecision::RejectStale
        );
        assert_eq!(gate.active_session(), Some(second));
        assert_eq!(
            gate.accept(other, &delta),
            BackendSessionDecision::RejectMissingSnapshot
        );
        assert_eq!(gate.active_session(), Some(second));
        assert_eq!(
            gate.accept(other, &snapshot),
            BackendSessionDecision::Replace
        );
        assert_eq!(gate.active_session(), Some(other));
    }
}
