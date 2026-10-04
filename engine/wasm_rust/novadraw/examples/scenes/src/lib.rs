//! 可由 Native DemoApp 和 Web Validation Host 共同使用的场景目录。

use novadraw::{FigureTree, Runtime};

pub mod advanced_figures;
pub mod border;
pub mod clip;
pub mod connection;
pub mod event;
pub mod focus;
pub mod freeform;
pub mod graphics;
pub mod layout;
pub mod ndcanvas;
pub mod scroll_pane;
pub mod shape;
pub mod style;
pub mod text;
pub mod transform;
pub mod update;
pub mod viewport;
pub mod widget;

pub type SceneEntry = (&'static str, Box<dyn FnMut() -> Runtime>);
pub type SceneFactory = Box<dyn FnMut() -> Runtime>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationKind {
    Visual,
    Interactive,
    UpdatePipeline,
    Performance,
}

pub struct SceneSpec {
    pub id: &'static str,
    pub title: &'static str,
    pub logical_size: (u32, u32),
    pub kind: ValidationKind,
    factory: SceneFactory,
}

impl SceneSpec {
    pub fn new(
        id: &'static str,
        title: &'static str,
        logical_size: (u32, u32),
        kind: ValidationKind,
        factory: impl FnMut() -> FigureTree + 'static,
    ) -> Self {
        Self {
            id,
            title,
            logical_size,
            kind,
            factory: Box::new({
                let mut factory = factory;
                move || Runtime::new(factory())
            }),
        }
    }

    pub fn runtime(
        id: &'static str,
        title: &'static str,
        logical_size: (u32, u32),
        kind: ValidationKind,
        factory: impl FnMut() -> Runtime + 'static,
    ) -> Self {
        Self {
            id,
            title,
            logical_size,
            kind,
            factory: Box::new(factory),
        }
    }

    pub fn build(&mut self) -> Runtime {
        (self.factory)()
    }

    pub fn visual(
        id: &'static str,
        title: &'static str,
        logical_size: (u32, u32),
        factory: impl FnMut() -> FigureTree + 'static,
    ) -> Self {
        Self::new(id, title, logical_size, ValidationKind::Visual, factory)
    }

    pub fn runtime_visual(
        id: &'static str,
        title: &'static str,
        logical_size: (u32, u32),
        factory: impl FnMut() -> Runtime + 'static,
    ) -> Self {
        Self::runtime(id, title, logical_size, ValidationKind::Visual, factory)
    }

    fn into_entry(self) -> SceneEntry {
        (self.title, self.factory)
    }
}

pub struct DemoSuite {
    pub id: &'static str,
    pub title: &'static str,
    pub scenes: Vec<SceneSpec>,
}

impl DemoSuite {
    pub fn new(id: &'static str, title: &'static str, scenes: Vec<SceneSpec>) -> Self {
        Self { id, title, scenes }
    }

    pub fn into_entries(self) -> Vec<SceneEntry> {
        self.scenes.into_iter().map(SceneSpec::into_entry).collect()
    }
}

pub fn catalog() -> Vec<DemoSuite> {
    vec![
        shape::suite(),
        style::suite(),
        transform::suite(),
        viewport::suite(),
        freeform::suite(),
        clip::suite(),
        connection::suite(),
        layout::suite(),
        event::suite(),
        update::suite(),
        scroll_pane::suite(),
        border::suite(),
        ndcanvas::suite(),
        text::suite(),
        widget::suite(),
        advanced_figures::suite(),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn catalog_has_unique_ids_and_buildable_scenes() {
        let mut suite_ids = BTreeSet::new();
        let mut scene_keys = BTreeSet::new();

        for mut suite in catalog() {
            assert!(
                suite_ids.insert(suite.id),
                "duplicate suite id: {}",
                suite.id
            );
            assert!(!suite.scenes.is_empty(), "empty suite: {}", suite.id);

            for scene in &mut suite.scenes {
                assert!(
                    scene_keys.insert((suite.id, scene.id)),
                    "duplicate scene id: {}/{}",
                    suite.id,
                    scene.id
                );
                assert!(
                    scene.logical_size.0 > 0 && scene.logical_size.1 > 0,
                    "invalid logical size: {}/{}",
                    suite.id,
                    scene.id
                );
                assert!(
                    scene.build().tree().contents().is_some(),
                    "scene has no contents: {}/{}",
                    suite.id,
                    scene.id
                );
            }
        }
    }
}
