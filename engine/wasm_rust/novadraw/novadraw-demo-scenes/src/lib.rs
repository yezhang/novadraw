//! 可由 Native DemoApp 和 Web Validation Host 共同使用的场景工厂。

use novadraw::FigureTree;

pub mod shape;
pub mod viewport;

pub type SceneEntry = (&'static str, Box<dyn FnMut() -> FigureTree>);

pub struct DemoTheme {
    pub id: &'static str,
    pub title: &'static str,
    pub scenes: Vec<SceneEntry>,
}

pub fn web_themes() -> Vec<DemoTheme> {
    vec![
        DemoTheme {
            id: "shape",
            title: "Shapes",
            scenes: shape::scenes(),
        },
        DemoTheme {
            id: "viewport",
            title: "Viewport",
            scenes: viewport::scenes(),
        },
    ]
}
