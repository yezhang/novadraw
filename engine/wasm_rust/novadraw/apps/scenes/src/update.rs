use novadraw::{
    Color, FigureTree, GridLayout, RectangleFigure, UpdateManager, XYConstraint, XYLayout,
};

use crate::{DemoSuite, SceneSpec, ValidationKind};

pub const WINDOW_WIDTH: f64 = 800.0;
pub const WINDOW_HEIGHT: f64 = 600.0;
pub const STRESS_FIGURE_COUNT: usize = 1024;

fn gray_background() -> RectangleFigure {
    RectangleFigure::new_with_color(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT, Color::hex("#eeeeee"))
}

pub fn baseline_scene() -> FigureTree {
    let mut graph = FigureTree::new();
    let root = graph.builder().set_contents(Box::new(gray_background()));
    for (x, color) in [(100.0, "#e74c3c"), (325.0, "#2ecc71"), (550.0, "#3498db")] {
        graph.builder().add_child_to(
            root,
            Box::new(RectangleFigure::new_with_color(
                x,
                200.0,
                150.0,
                100.0,
                Color::hex(color),
            )),
        );
    }
    graph
}

fn partial_damage_scene() -> FigureTree {
    let mut graph = baseline_scene();
    let root = graph.get_contents().expect("contents");
    let target = graph.child_order(root).expect("root children")[1];
    let mut manager = UpdateManager::new();
    let old_bounds = graph.figure_bounds(target).expect("target bounds");
    graph.set_bounds_with_update(
        &mut manager,
        target,
        old_bounds.x + 40.0,
        old_bounds.y + 30.0,
        old_bounds.width,
        old_bounds.height,
    );
    let _ = graph.perform_update(&mut manager);
    graph
}

pub fn validation_scene() -> FigureTree {
    let mut graph = FigureTree::new();
    let root = graph.builder().set_contents(Box::new(gray_background()));
    graph.set_block_layout_manager(root, Box::new(XYLayout::new()));
    for (index, color) in ["#9b59b6", "#f39c12", "#1abc9c"].iter().enumerate() {
        let child = graph.builder().add_child_to(
            root,
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                140.0,
                90.0,
                Color::hex(color),
            )),
        );
        graph.set_constraint(
            child,
            XYConstraint::at_size(100.0 + index as f64 * 220.0, 220.0, 140.0, 90.0),
        );
    }
    graph.revalidate(root);
    graph
}

pub fn stress_scene() -> FigureTree {
    let mut graph = FigureTree::new();
    let root = graph.builder().set_contents(Box::new(gray_background()));
    graph.set_block_layout_manager(
        root,
        Box::new(
            GridLayout::new(32)
                .with_margins(8.0, 8.0)
                .with_spacing(2.0, 2.0),
        ),
    );
    for index in 0..STRESS_FIGURE_COUNT {
        let channel = (index % 32) as f64 / 31.0;
        graph.builder().add_child_to(
            root,
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                20.0,
                14.0,
                Color::rgba(channel, 0.55, 1.0 - channel, 1.0),
            )),
        );
    }
    graph.revalidate(root);
    graph
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "update",
        "Update pipeline",
        vec![
            SceneSpec::new(
                "baseline",
                "baseline",
                size,
                ValidationKind::UpdatePipeline,
                baseline_scene,
            ),
            SceneSpec::new(
                "partial-damage",
                "partial_damage",
                size,
                ValidationKind::UpdatePipeline,
                partial_damage_scene,
            ),
            SceneSpec::new(
                "validation",
                "validation",
                size,
                ValidationKind::UpdatePipeline,
                validation_scene,
            ),
            SceneSpec::new(
                "stress-1024",
                "stress_1024",
                size,
                ValidationKind::Performance,
                stress_scene,
            ),
        ],
    )
}
