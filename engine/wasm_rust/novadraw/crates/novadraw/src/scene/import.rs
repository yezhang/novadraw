//! Destination identities and CPU leases for immutable, multi-version resources.
use super::SceneError;
use crate::{
    advanced::RuntimeNamespace,
    render::{
        FontFaceRef, ImageResourceRef, RecordedDrawing, RenderCommandKind, ResourceId,
        ResourcePayload, ResourceUpdate,
    },
    runtime::ResourceRegistry,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
};

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
struct ImportKey {
    producer: RuntimeNamespace,
    resource: ResourceId,
    revision: u64,
    image: bool,
}

#[derive(Debug)]
pub(crate) struct ImportLease {
    pub(crate) update: ResourceUpdate,
}

#[derive(Clone, Default)]
pub(crate) struct ResourceImports {
    entries: HashMap<ImportKey, Weak<ImportLease>>,
    // The table owns identity metadata, not a permanent payload reference.
    ids: HashMap<ImportKey, ResourceId>,
    pub(crate) recovery: Vec<Arc<ImportLease>>,
}

pub(crate) struct ImportCandidate {
    pub(crate) registry: ResourceRegistry,
    pub(crate) imports: ResourceImports,
    pub(crate) drawing: RecordedDrawing,
    pub(crate) leases: Vec<Arc<ImportLease>>,
}

impl ResourceImports {
    pub(crate) fn prepare(
        &self,
        registry: &ResourceRegistry,
        producer: RuntimeNamespace,
        drawing: &RecordedDrawing,
    ) -> Result<ImportCandidate, SceneError> {
        // All validation precedes allocation. Candidate storage never mutates the destination.
        crate::render::validate_graphics_input(drawing.commands(), 1.0)
            .map_err(SceneError::InvalidGraphicsInput)?;
        let mut resources = HashMap::new();
        for update in &drawing.resources().ready {
            if update.revision == 0 || resources.insert(update.id, update).is_some() {
                return Err(SceneError::ResourceMismatch);
            }
        }
        let mut referenced = HashSet::new();
        let mut depth = 0_usize;
        for command in drawing.commands() {
            match &command.kind {
                RenderCommandKind::PushState => depth += 1,
                RenderCommandKind::PopState => {
                    depth = depth.checked_sub(1).ok_or(SceneError::Graphics(
                        crate::graphics::GraphicsError::UnbalancedState,
                    ))?;
                }
                RenderCommandKind::RestoreState if depth == 0 => {
                    return Err(SceneError::Graphics(
                        crate::graphics::GraphicsError::UnbalancedState,
                    ));
                }
                RenderCommandKind::ResetClip => {
                    return Err(SceneError::Policy(
                        "replay cannot reset its enclosing clip".into(),
                    ));
                }
                RenderCommandKind::Image { image, .. } => {
                    let update = resources
                        .get(&image.resource_id())
                        .ok_or(SceneError::ResourceMismatch)?;
                    match &update.payload {
                        ResourcePayload::Image(data)
                            if update.revision == image.revision()
                                && data.width == image.width()
                                && data.height == image.height()
                                && data.scale == image.scale() => {}
                        _ => return Err(SceneError::ResourceMismatch),
                    }
                    referenced.insert(update.id);
                }
                RenderCommandKind::DrawGlyphRun { run, .. } => {
                    let update = resources
                        .get(&run.font.resource_id())
                        .ok_or(SceneError::ResourceMismatch)?;
                    crate::text::outline::FontInstanceRef::new(
                        &run.font,
                        update,
                        &run.normalized_coords,
                    )
                    .map_err(|error| {
                        SceneError::Graphics(crate::graphics::GraphicsError::Font(error))
                    })?;
                    referenced.insert(update.id);
                }
                _ => {}
            }
        }
        if depth != 0 || referenced.len() != resources.len() {
            return Err(SceneError::ResourceMismatch);
        }
        let mut candidate = ImportCandidate {
            registry: registry.clone(),
            imports: self.clone(),
            drawing: drawing.clone(),
            leases: Vec::new(),
        };
        candidate.imports.collect(&mut candidate.registry);
        let mut mapping = HashMap::new();
        for update in &drawing.resources().ready {
            let key = ImportKey {
                producer,
                resource: update.id,
                revision: update.revision,
                image: matches!(update.payload, ResourcePayload::Image(_)),
            };
            let lease = if let Some(lease) = self.entries.get(&key).and_then(Weak::upgrade) {
                if lease.update.payload != update.payload {
                    return Err(SceneError::SnapshotConflict);
                }
                lease
            } else {
                let mapped = candidate.registry.import_payload(update.payload.clone());
                let lease = Arc::new(ImportLease { update: mapped });
                candidate.imports.ids.insert(key, lease.update.id);
                candidate
                    .imports
                    .entries
                    .insert(key, Arc::downgrade(&lease));
                lease
            };
            mapping.insert(update.id, lease.update.clone());
            candidate.leases.push(lease);
        }
        for command in &mut candidate.drawing.commands {
            match &mut command.kind {
                RenderCommandKind::Image { image, .. } => {
                    let mapped = &mapping[&image.resource_id()];
                    *image = ImageResourceRef::new(
                        mapped.id,
                        mapped.revision,
                        image.width(),
                        image.height(),
                        image.scale(),
                    );
                }
                RenderCommandKind::DrawGlyphRun { run, .. } => {
                    let mapped = &mapping[&run.font.resource_id()];
                    run.font =
                        FontFaceRef::new(mapped.id, mapped.revision, run.font.collection_index());
                }
                _ => {}
            }
        }
        candidate.drawing.resources.ready = candidate
            .leases
            .iter()
            .map(|lease| lease.update.clone())
            .collect();
        Ok(candidate)
    }

    pub(crate) fn retain_commands(
        &self,
        commands: &[crate::render::RenderCommand],
    ) -> Vec<Arc<ImportLease>> {
        let ids: HashSet<_> = commands
            .iter()
            .filter_map(|command| match &command.kind {
                RenderCommandKind::Image { image, .. } => Some(image.resource_id()),
                RenderCommandKind::DrawGlyphRun { run, .. } => Some(run.font.resource_id()),
                _ => None,
            })
            .collect();
        self.entries
            .values()
            .filter_map(Weak::upgrade)
            .filter(|lease| ids.contains(&lease.update.id))
            .collect()
    }

    pub(crate) fn collect(&mut self, registry: &mut ResourceRegistry) {
        let retired: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(key, lease)| (lease.strong_count() == 0).then_some(*key))
            .collect();
        for key in retired {
            self.entries.remove(&key);
            if let Some(id) = self.ids.remove(&key) {
                // Imported IDs are never handed out as mutable ImageId/FontId.
                registry
                    .remove_import(id)
                    .expect("live import metadata owns its registry identity");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn drawing(revision: u64) -> RecordedDrawing {
        let image =
            ImageResourceRef::new(ResourceId::new(uuid::Uuid::nil(), 1), revision, 1, 1, 1.0);
        let mut recorder = crate::render::CommandRecorder::new();
        recorder
            .paint()
            .draw_image(image, crate::geometry::Rectangle::new(0.0, 0.0, 10.0, 10.0))
            .unwrap();
        recorder.leases.push(ResourceUpdate {
            id: image.resource_id(),
            revision,
            payload: ResourcePayload::Image(Arc::new(crate::render::ImageData::from_rgba(
                1,
                1,
                vec![255; 4],
                1.0,
            ))),
        });
        recorder.finish().unwrap()
    }
    #[test]
    fn import_failure_does_not_allocate_registry_or_modify_current_table() {
        let table = ResourceImports::default();
        let registry = ResourceRegistry::new();
        let mut broken = drawing(1);
        broken.resources.ready.clear();
        assert!(matches!(
            table.prepare(&registry, RuntimeNamespace::new(), &broken),
            Err(SceneError::ResourceMismatch)
        ));
        assert!(!registry.has_pending_delta());
        assert!(table.entries.is_empty());
        let mut duplicate = drawing(1);
        duplicate
            .resources
            .ready
            .push(duplicate.resources.ready[0].clone());
        assert!(matches!(
            table.prepare(&registry, RuntimeNamespace::new(), &duplicate),
            Err(SceneError::ResourceMismatch)
        ));
        assert!(table.entries.is_empty());
    }
    #[test]
    fn import_key_deduplicates_exact_payload_and_reclaims_after_last_cpu_lease() {
        let producer = RuntimeNamespace::new();
        let first = ResourceImports::default()
            .prepare(&ResourceRegistry::new(), producer, &drawing(1))
            .unwrap();
        let id = first.drawing.resources.ready[0].id;
        let mut registry = first.registry;
        let mut table = first.imports;
        let leases = first.leases;
        let duplicate = table.prepare(&registry, producer, &drawing(1)).unwrap();
        assert_eq!(duplicate.drawing.resources.ready[0].id, id);
        let newer = table.prepare(&registry, producer, &drawing(2)).unwrap();
        assert_ne!(newer.drawing.resources.ready[0].id, id);
        let mut conflict = drawing(1);
        conflict.resources.ready[0].payload = ResourcePayload::Image(Arc::new(
            crate::render::ImageData::from_rgba(1, 1, vec![0; 4], 1.0),
        ));
        assert!(matches!(
            table.prepare(&registry, producer, &conflict),
            Err(SceneError::SnapshotConflict)
        ));
        table.collect(&mut registry);
        assert!(registry.status(id).is_ok());
        drop(leases);
        table.collect(&mut registry);
        assert!(
            registry.status(id).is_ok(),
            "duplicate candidate also retains the import"
        );
        drop(duplicate);
        table.collect(&mut registry);
        assert_eq!(
            registry.status(id),
            Err(crate::runtime::ResourceError::UnknownResource)
        );
        assert!(
            registry.take_delta().ops.iter().any(
                |op| matches!(op, crate::render::ResourceOp::Remove(removed) if *removed == id)
            )
        );
    }
}
