use std::error::Error;
use std::fmt;
use std::sync::Arc;

use novadraw_render::{
    FontData, ImageData, ImageResourceRef, ResourceDelta, ResourceId, ResourcePayload,
    ResourceSnapshot, ResourceUpdate,
};
use slotmap::{Key, KeyData, SlotMap, new_key_type};
use uuid::Uuid;

use crate::FigureId;

new_key_type! {
    struct ResourceKey;
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImageId(ResourceId);

impl ImageId {
    pub const fn resource_id(self) -> ResourceId {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FontId(ResourceId);

impl FontId {
    pub const fn resource_id(self) -> ResourceId {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    Image,
    Font,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceStatus {
    Pending,
    Ready { revision: u64 },
    Failed { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceError {
    WrongNamespace,
    UnknownResource,
    KindMismatch {
        expected: ResourceKind,
        actual: ResourceKind,
    },
    UnknownFigure,
    InvalidFontData,
    RevisionExhausted,
}

impl fmt::Display for ResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongNamespace => formatter.write_str("resource belongs to another runtime"),
            Self::UnknownResource => formatter.write_str("resource does not exist"),
            Self::KindMismatch { expected, actual } => {
                write!(
                    formatter,
                    "resource kind mismatch: expected {expected:?}, actual {actual:?}"
                )
            }
            Self::UnknownFigure => formatter.write_str("dependent figure is not attached"),
            Self::InvalidFontData => formatter.write_str("font data is empty or invalid"),
            Self::RevisionExhausted => formatter.write_str("resource revision is exhausted"),
        }
    }
}

impl Error for ResourceError {}

struct ResourceEntry {
    kind: ResourceKind,
    status: ResourceStatus,
    revision: u64,
    payload: Option<ResourcePayload>,
    dependents: Vec<FigureId>,
}

pub struct ResourceRegistry {
    namespace: Uuid,
    entries: SlotMap<ResourceKey, ResourceEntry>,
    pending_delta: ResourceDelta,
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self::with_namespace(Uuid::new_v4())
    }

    pub fn with_namespace(namespace: Uuid) -> Self {
        Self {
            namespace,
            entries: SlotMap::with_key(),
            pending_delta: ResourceDelta::default(),
        }
    }

    pub fn namespace(&self) -> Uuid {
        self.namespace
    }

    pub fn register_image(&mut self) -> ImageId {
        ImageId(self.register(ResourceKind::Image))
    }

    pub fn register_font(&mut self) -> FontId {
        FontId(self.register(ResourceKind::Font))
    }

    pub fn status(&self, id: ResourceId) -> Result<&ResourceStatus, ResourceError> {
        let key = self.key(id)?;
        self.entries
            .get(key)
            .map(|entry| &entry.status)
            .ok_or(ResourceError::UnknownResource)
    }

    pub(crate) fn image_ref(&self, id: ImageId) -> Option<ImageResourceRef> {
        let key = self.key(id.resource_id()).ok()?;
        let entry = self.entries.get(key)?;
        let ResourceStatus::Ready { revision } = entry.status else {
            return None;
        };
        let ResourcePayload::Image(image) = entry.payload.as_ref()? else {
            return None;
        };
        Some(ImageResourceRef::new(
            id.resource_id(),
            revision,
            image.width,
            image.height,
            image.scale,
        ))
    }

    pub(crate) fn kind(&self, id: ResourceId) -> Result<ResourceKind, ResourceError> {
        let key = self.key(id)?;
        self.entries
            .get(key)
            .map(|entry| entry.kind)
            .ok_or(ResourceError::UnknownResource)
    }

    pub(crate) fn next_revision(
        &self,
        id: ResourceId,
        expected: ResourceKind,
    ) -> Result<u64, ResourceError> {
        let key = self.key(id)?;
        let entry = self
            .entries
            .get(key)
            .ok_or(ResourceError::UnknownResource)?;
        if entry.kind != expected {
            return Err(ResourceError::KindMismatch {
                expected,
                actual: entry.kind,
            });
        }
        entry
            .revision
            .checked_add(1)
            .ok_or(ResourceError::RevisionExhausted)
    }

    pub fn add_dependency(
        &mut self,
        id: ResourceId,
        figure: FigureId,
    ) -> Result<(), ResourceError> {
        let key = self.key(id)?;
        let entry = self
            .entries
            .get_mut(key)
            .ok_or(ResourceError::UnknownResource)?;
        if !entry.dependents.contains(&figure) {
            entry.dependents.push(figure);
        }
        Ok(())
    }

    pub fn remove_dependency(
        &mut self,
        id: ResourceId,
        figure: FigureId,
    ) -> Result<bool, ResourceError> {
        let key = self.key(id)?;
        let entry = self
            .entries
            .get_mut(key)
            .ok_or(ResourceError::UnknownResource)?;
        let old_len = entry.dependents.len();
        entry.dependents.retain(|candidate| *candidate != figure);
        Ok(entry.dependents.len() != old_len)
    }

    pub fn complete_image(
        &mut self,
        id: ImageId,
        image: ImageData,
    ) -> Result<Vec<FigureId>, ResourceError> {
        self.complete(
            id.resource_id(),
            ResourceKind::Image,
            ResourcePayload::Image(Arc::new(image)),
        )
    }

    pub fn complete_font(
        &mut self,
        id: FontId,
        font: FontData,
    ) -> Result<Vec<FigureId>, ResourceError> {
        self.complete(
            id.resource_id(),
            ResourceKind::Font,
            ResourcePayload::Font(Arc::new(font)),
        )
    }

    pub fn fail(
        &mut self,
        id: ResourceId,
        reason: impl Into<String>,
    ) -> Result<Vec<FigureId>, ResourceError> {
        let key = self.key(id)?;
        let entry = self
            .entries
            .get_mut(key)
            .ok_or(ResourceError::UnknownResource)?;
        if matches!(entry.status, ResourceStatus::Ready { .. }) {
            self.pending_delta.remove(id);
        }
        entry.status = ResourceStatus::Failed {
            reason: reason.into(),
        };
        entry.payload = None;
        Ok(entry.dependents.clone())
    }

    pub fn remove(&mut self, id: ResourceId) -> Result<Vec<FigureId>, ResourceError> {
        let key = self.key(id)?;
        let entry = self
            .entries
            .remove(key)
            .ok_or(ResourceError::UnknownResource)?;
        if matches!(entry.status, ResourceStatus::Ready { .. }) {
            self.pending_delta.remove(id);
        }
        Ok(entry.dependents)
    }

    pub fn retain_dependencies(&mut self, mut retain: impl FnMut(FigureId) -> bool) {
        for entry in self.entries.values_mut() {
            entry.dependents.retain(|figure| retain(*figure));
        }
    }

    pub fn take_delta(&mut self) -> ResourceDelta {
        std::mem::take(&mut self.pending_delta)
    }

    pub fn restore_delta(&mut self, mut earlier: ResourceDelta) {
        earlier.extend(std::mem::take(&mut self.pending_delta));
        self.pending_delta = earlier;
    }

    pub fn has_pending_delta(&self) -> bool {
        !self.pending_delta.is_empty()
    }

    pub(crate) fn take_ready_snapshot(&mut self) -> ResourceSnapshot {
        self.pending_delta = ResourceDelta::default();
        let mut ready = self
            .entries
            .iter()
            .filter_map(|(key, entry)| {
                let ResourceStatus::Ready { revision } = entry.status else {
                    return None;
                };
                Some(ResourceUpdate {
                    id: ResourceId::new(self.namespace, key.data().as_ffi()),
                    revision,
                    payload: entry
                        .payload
                        .clone()
                        .expect("Ready resource must own a payload"),
                })
            })
            .collect::<Vec<_>>();
        ready.sort_by_key(|update| update.id);
        ResourceSnapshot { ready }
    }

    fn register(&mut self, kind: ResourceKind) -> ResourceId {
        let key = self.entries.insert(ResourceEntry {
            kind,
            status: ResourceStatus::Pending,
            revision: 0,
            payload: None,
            dependents: Vec::new(),
        });
        ResourceId::new(self.namespace, key.data().as_ffi())
    }

    fn complete(
        &mut self,
        id: ResourceId,
        expected: ResourceKind,
        payload: ResourcePayload,
    ) -> Result<Vec<FigureId>, ResourceError> {
        let key = self.key(id)?;
        let entry = self
            .entries
            .get_mut(key)
            .ok_or(ResourceError::UnknownResource)?;
        if entry.kind != expected {
            return Err(ResourceError::KindMismatch {
                expected,
                actual: entry.kind,
            });
        }
        let revision = entry
            .revision
            .checked_add(1)
            .ok_or(ResourceError::RevisionExhausted)?;
        entry.revision = revision;
        entry.status = ResourceStatus::Ready { revision };
        entry.payload = Some(payload.clone());
        self.pending_delta.upsert(ResourceUpdate {
            id,
            revision,
            payload,
        });
        Ok(entry.dependents.clone())
    }

    fn key(&self, id: ResourceId) -> Result<ResourceKey, ResourceError> {
        if id.namespace() != self.namespace {
            return Err(ResourceError::WrongNamespace);
        }
        Ok(ResourceKey::from(KeyData::from_ffi(id.generation_key())))
    }
}

#[cfg(test)]
mod tests {
    use novadraw_render::{ImageData, ResourceOp};
    use slotmap::KeyData;

    use super::*;

    fn figure(value: u64) -> FigureId {
        FigureId::from(KeyData::from_ffi(value))
    }

    #[test]
    fn removed_slot_gets_a_new_generation_and_rejects_the_old_id() {
        let mut registry = ResourceRegistry::with_namespace(Uuid::nil());
        let first = registry.register_image();
        registry.remove(first.resource_id()).unwrap();
        let second = registry.register_image();

        assert_ne!(first, second);
        assert_eq!(
            registry.status(first.resource_id()),
            Err(ResourceError::UnknownResource)
        );
        assert_eq!(
            registry.status(second.resource_id()),
            Ok(&ResourceStatus::Pending)
        );
    }

    #[test]
    fn completion_queues_payload_and_returns_dependents() {
        let mut registry = ResourceRegistry::with_namespace(Uuid::nil());
        let image = registry.register_image();
        let dependent = figure(1);
        registry
            .add_dependency(image.resource_id(), dependent)
            .unwrap();

        let dependents = registry
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255, 0, 0, 255], 1.0))
            .unwrap();
        let delta = registry.take_delta();

        assert_eq!(dependents, vec![dependent]);
        assert_eq!(delta.ops.len(), 1);
        let ResourceOp::Upsert(update) = &delta.ops[0] else {
            panic!("completion must queue an upsert");
        };
        assert_eq!(update.id, image.resource_id());
        assert_eq!(update.revision, 1);
        assert_eq!(
            registry.status(image.resource_id()),
            Ok(&ResourceStatus::Ready { revision: 1 })
        );
    }

    #[test]
    fn dependency_reconciliation_removes_detached_figures() {
        let mut registry = ResourceRegistry::with_namespace(Uuid::nil());
        let image = registry.register_image();
        let detached = figure(1);
        let attached = figure(2);
        registry
            .add_dependency(image.resource_id(), detached)
            .unwrap();
        registry
            .add_dependency(image.resource_id(), attached)
            .unwrap();

        registry.retain_dependencies(|figure| figure == attached);
        let dependents = registry
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255; 4], 1.0))
            .unwrap();

        assert_eq!(dependents, vec![attached]);
    }

    #[test]
    fn failed_ready_resource_releases_backend_copy() {
        let mut registry = ResourceRegistry::with_namespace(Uuid::nil());
        let image = registry.register_image();
        registry
            .complete_image(
                image,
                ImageData::from_rgba(1, 1, vec![255, 255, 255, 255], 1.0),
            )
            .unwrap();
        registry.take_delta();

        registry
            .fail(image.resource_id(), "decode invalidated")
            .unwrap();

        assert_eq!(
            registry.take_delta().ops,
            vec![ResourceOp::Remove(image.resource_id())]
        );
    }

    #[test]
    fn recovery_after_failure_advances_revision() {
        let mut registry = ResourceRegistry::with_namespace(Uuid::nil());
        let image = registry.register_image();
        registry
            .complete_image(
                image,
                ImageData::from_rgba(1, 1, vec![255, 255, 255, 255], 1.0),
            )
            .unwrap();
        registry.take_delta();
        registry.fail(image.resource_id(), "transient").unwrap();
        registry.take_delta();

        registry
            .complete_image(image, ImageData::from_rgba(1, 1, vec![0, 0, 0, 255], 1.0))
            .unwrap();

        assert_eq!(
            registry.status(image.resource_id()),
            Ok(&ResourceStatus::Ready { revision: 2 })
        );
        let delta = registry.take_delta();
        let ResourceOp::Upsert(update) = &delta.ops[0] else {
            panic!("recovery must queue an upsert");
        };
        assert_eq!(update.revision, 2);
    }

    #[test]
    fn ready_failed_ready_preserves_backend_operation_order() {
        let mut registry = ResourceRegistry::with_namespace(Uuid::nil());
        let image = registry.register_image();
        registry
            .complete_image(image, ImageData::from_rgba(1, 1, vec![255; 4], 1.0))
            .unwrap();
        registry.fail(image.resource_id(), "transient").unwrap();
        registry
            .complete_image(image, ImageData::from_rgba(1, 1, vec![0; 4], 1.0))
            .unwrap();

        let delta = registry.take_delta();
        assert!(matches!(
            delta.ops.as_slice(),
            [
                ResourceOp::Upsert(ResourceUpdate { revision: 1, .. }),
                ResourceOp::Remove(id),
                ResourceOp::Upsert(ResourceUpdate { revision: 2, .. }),
            ] if *id == image.resource_id()
        ));
    }

    #[test]
    fn ready_snapshot_is_stable_and_supersedes_pending_ops() {
        let mut registry = ResourceRegistry::with_namespace(Uuid::nil());
        let first = registry.register_image();
        let second = registry.register_image();
        registry
            .complete_image(second, ImageData::from_rgba(1, 1, vec![2; 4], 1.0))
            .unwrap();
        registry
            .complete_image(first, ImageData::from_rgba(1, 1, vec![1; 4], 1.0))
            .unwrap();

        let snapshot = registry.take_ready_snapshot();

        assert_eq!(
            snapshot
                .ready
                .iter()
                .map(|update| update.id)
                .collect::<Vec<_>>(),
            vec![first.resource_id(), second.resource_id()]
        );
        assert!(!registry.has_pending_delta());
    }
}
