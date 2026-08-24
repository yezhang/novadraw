use novadraw_geometry::Rectangle;

use crate::command::RenderCommand;

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
}
