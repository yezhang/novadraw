use std::sync::Arc;

use novadraw_geometry::Rectangle;
use uuid::Uuid;

use crate::command::{ImageData, RenderCommand};
use crate::text::FontFaceResource;

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
    pub bytes: Vec<u8>,
}

impl FontData {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
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
    pub added: Vec<ResourceUpdate>,
    pub removed: Vec<ResourceId>,
}

impl ResourceDelta {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    pub fn extend(&mut self, other: Self) {
        self.added.extend(other.added);
        self.removed.extend(other.removed);
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
    pub resources: ResourceDelta,
    pub font_faces: Vec<FontFaceResource>,
    pub surface: SurfaceInfo,
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
        let mut delta = ResourceDelta {
            added: vec![image.clone()],
            removed: vec![ResourceId::new(namespace, 2)],
        };
        delta.extend(ResourceDelta {
            added: vec![font.clone()],
            removed: vec![ResourceId::new(namespace, 4)],
        });

        assert_eq!(delta.added, vec![image, font]);
        assert_eq!(
            delta.removed,
            vec![ResourceId::new(namespace, 2), ResourceId::new(namespace, 4)]
        );
    }
}
