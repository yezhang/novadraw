use novadraw_geometry::Rectangle;

use crate::command::RenderCommand;

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

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ResourceDelta {
    pub added: Vec<u64>,
    pub removed: Vec<u64>,
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
    pub surface: SurfaceInfo,
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
    }
}
