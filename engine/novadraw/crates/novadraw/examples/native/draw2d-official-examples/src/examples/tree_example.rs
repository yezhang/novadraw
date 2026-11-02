//! TreeExample.java fixture and root/style controls (IBM and contributors, EPL-2.0).
#[cfg(test)]
use novadraw::{event::EventContext, figure::LabelFigure};
#[cfg(test)]
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use novadraw::{
    figure::RectangleFigure, geometry::Rectangle, graphics::Color, runtime::Runtime, tree::FigureId,
};

use super::{
    runtime_with_fonts,
    simple_tree_example::{EnablePageSelection, add_branch_with_root, branch_subtree},
    tree_layout::{Branch, BranchLayout, BranchStyle, ContentsLayout, Lines, TreeModel},
};
#[cfg(test)]
use crate::controls::{ScaleFigure, add_button, add_checkbox, replace_text_value_later};
use novadraw_example_support::panel::{Panel, PanelScene, egui};

pub fn panel_scene() -> PanelScene {
    panel_scene_from_fixture(canvas_fixture(false))
}

fn panel_scene_from_fixture(fixture: Fixture) -> PanelScene {
    let model = fixture.model;
    let selection = fixture.selection;
    (
        fixture.runtime,
        Some(Panel::new([PANEL_X as f32, 20.0], move |ui, runtime| {
            super::tree_animation::process(runtime, &model);
            ui.heading("Tree");
            let mut changed = false;
            {
                let mut tree = model.lock().expect("tree model");
                if ui
                    .checkbox(&mut tree.use_animation, "Use Animation")
                    .changed()
                    && !tree.use_animation
                    && let Some(id) = tree.animation.take()
                {
                    runtime
                        .animation()
                        .timeline()
                        .cancel(id)
                        .expect("tree animation cancellation");
                }
                if let Some(error) = &tree.animation_error {
                    ui.label(error);
                }
                changed |= ui
                    .checkbox(&mut tree.horizontal, "Horizontal Orientation")
                    .changed();
                changed |= ui.checkbox(&mut tree.compressed, "Compress Tree").changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut tree.major, SPACING_MIN..=SPACING_MAX)
                            .integer()
                            .text("Major Spacing"),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut tree.minor, SPACING_MIN..=SPACING_MAX)
                            .integer()
                            .text("Minor Spacing"),
                    )
                    .changed();
            }
            ui.separator();
            let selected = *selection.lock().expect("tree selection");
            ui.label("Selected Node");
            ui.add_enabled_ui(selected.is_some(), |ui| {
                let Some(index) = selected else {
                    return;
                };
                {
                    let mut tree = model.lock().expect("tree model");
                    let branch = &mut tree.branches[index];
                    changed |= ui.checkbox(&mut branch.centered, "Align Center").changed();
                    changed |= ui
                        .radio_value(&mut branch.style, BranchStyle::Normal, "Normal")
                        .changed();
                    changed |= ui
                        .radio_value(&mut branch.style, BranchStyle::Hanging, "Hanging")
                        .changed();
                    if !branch.children.is_empty() {
                        let mut expanded = branch.expanded;
                        if ui.checkbox(&mut expanded, "Expanded").changed() {
                            tree.pending_toggle = Some(index);
                        }
                    }
                }
                if ui.button("More Children").clicked() {
                    let (next, parent, style) = {
                        let tree = model.lock().expect("tree model");
                        (
                            tree.branches.len(),
                            tree.branches[index].contents,
                            tree.branches[index].style,
                        )
                    };
                    let (subtree, contents, title, _) =
                        branch_subtree("child", next, selection.clone(), model.clone());
                    let receipt = runtime
                        .figure(parent)
                        .expect("tree parent")
                        .add_subtree(subtree)
                        .expect("tree insertion");
                    let mut tree = model.lock().expect("tree model");
                    tree.branches.push(Branch {
                        figure: receipt.root(),
                        contents: receipt.figure(contents).expect("branch contents"),
                        title: receipt.figure(title).expect("branch title"),
                        children: Vec::new(),
                        style,
                        centered: true,
                        expanded: true,
                        active: true,
                    });
                    tree.branches[index].children.push(next);
                    changed = true;
                }
                if ui.button("Fewer Children").clicked() {
                    let mut tree = model.lock().expect("tree model");
                    if let Some(child) = tree.branches[index].children.last().copied() {
                        runtime
                            .figure(tree.branches[index].contents)
                            .expect("tree parent")
                            .remove(tree.branches[child].figure)
                            .expect("tree removal");
                        tree.branches[index].children.pop();
                        let mut descendants = vec![child];
                        while let Some(index) = descendants.pop() {
                            tree.branches[index].active = false;
                            descendants.extend(tree.branches[index].children.iter().copied());
                        }
                        changed = true;
                    }
                }
            });
            if changed {
                let mut tree = model.lock().expect("tree model");
                if let Some(id) = tree.animation.take() {
                    runtime
                        .animation()
                        .timeline()
                        .cancel(id)
                        .expect("tree animation cancellation");
                }
                let root = tree.branches[0].contents;
                drop(tree);
                runtime
                    .figure(root)
                    .expect("tree contents")
                    .set_layout_manager(Box::new(ContentsLayout {
                        model: model.clone(),
                        index: 0,
                    }))
                    .expect("tree layout");
            }
            super::tree_animation::process(runtime, &model);
        })),
    )
}

const CANVAS_WIDTH: f64 = 850.0;
const PANEL_X: f64 = 860.0;
#[cfg(test)]
const PANEL_WIDTH: f64 = 310.0;
#[cfg(test)]
const CONTROL_HEIGHT: f64 = 24.0;
const SPACING_MIN: f64 = 5.0;
const SPACING_MAX: f64 = 50.0;
#[cfg(test)]
const SPACING_DEFAULT: f64 = 10.0;
#[cfg(test)]
const SPACING_INCREMENT: f64 = 5.0;

#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct Fixture {
    pub(super) runtime: Runtime,
    pub(super) model: Arc<Mutex<TreeModel>>,
    selection: Arc<Mutex<Option<usize>>>,
    controls: Vec<FigureId>,
}

pub fn tree_example() -> Runtime {
    fixture().runtime
}

#[cfg(test)]
fn invalidate_tree(model: &Arc<Mutex<TreeModel>>, context: &mut EventContext<'_>) {
    let root = model.lock().expect("tree model").branches[0].contents;
    context.effects().set_layout_manager_later(
        root,
        Box::new(ContentsLayout {
            model: model.clone(),
            index: 0,
        }),
    );
}

fn fixture() -> Fixture {
    canvas_fixture(cfg!(test))
}

pub(super) fn canvas_fixture(_legacy_controls: bool) -> Fixture {
    let mut runtime = runtime_with_fonts();
    let host = runtime
        .set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, 1200.0, 800.0, Color::WHITE)
                .expect("valid figure bounds"),
        ))
        .expect("white FigureCanvas");
    let viewport = runtime
        .viewports()
        .add(host, Rectangle::new(0.0, 0.0, CANVAS_WIDTH, 780.0))
        .expect("Tree viewport")
        .figure_id();
    // Parent indices preserve TreeExample.createContents insertion order,
    // including the two standalone Child siblings between Normal Style branches.
    let source = [
        (None, "Graph Root", BranchStyle::Normal),
        (Some(0), "Normal Style", BranchStyle::Normal),
        (Some(0), "Child", BranchStyle::Normal),
        (Some(1), "Child 1", BranchStyle::Normal),
        (Some(1), "Child 2", BranchStyle::Normal),
        (Some(1), "Child 3", BranchStyle::Normal),
        (Some(5), "child", BranchStyle::Normal),
        (Some(5), "child", BranchStyle::Normal),
        (Some(5), "child", BranchStyle::Normal),
        (Some(0), "Normal Style", BranchStyle::Normal),
        (Some(0), "Child", BranchStyle::Normal),
        (Some(9), "Child 1", BranchStyle::Hanging),
        (Some(9), "Child 2", BranchStyle::Hanging),
        (Some(12), "child", BranchStyle::Normal),
        (Some(12), "child", BranchStyle::Normal),
        (Some(0), "Normal Style", BranchStyle::Normal),
        (Some(15), "Child 1", BranchStyle::Normal),
        (Some(15), "Child 2", BranchStyle::Normal),
        (Some(15), "Child 3", BranchStyle::Normal),
    ];
    let mut branches: Vec<Branch> = Vec::new();
    let mut lines: Vec<Lines> = Vec::new();
    for (index, (parent, text, style)) in source.into_iter().enumerate() {
        let parent_figure = parent.map(|p| branches[p].contents).unwrap_or(viewport);
        let (figure, contents, title, branch_lines) =
            add_branch_with_root(&mut runtime, Some(parent_figure), text, parent.is_none());
        branches.push(Branch {
            figure,
            contents,
            title,
            children: Vec::new(),
            style,
            centered: true,
            expanded: true,
            active: true,
        });
        lines.push(branch_lines);
        if let Some(parent) = parent {
            branches[parent].children.push(index);
        }
    }
    runtime
        .figure(branches[0].figure)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, CANVAS_WIDTH, 780.0))
        .unwrap();
    let mut tree = TreeModel::new(branches);
    tree.viewport = Some(viewport);
    let model = Arc::new(Mutex::new(tree));
    let selection = Arc::new(Mutex::new(None));
    {
        let locked = model.lock().unwrap();
        for (index, (branch, lines)) in locked.branches.iter().zip(lines).enumerate() {
            runtime
                .figure(branch.figure)
                .unwrap()
                .set_layout_manager(Box::new(BranchLayout {
                    model: model.clone(),
                    index,
                    lines,
                }))
                .unwrap();
            runtime
                .figure(branch.contents)
                .unwrap()
                .set_layout_manager(Box::new(ContentsLayout {
                    model: model.clone(),
                    index,
                }))
                .unwrap();
            runtime
                .figure(branch.title)
                .unwrap()
                .update_component(EnablePageSelection {
                    index,
                    selection: selection.clone(),
                    model: model.clone(),
                })
                .unwrap();
        }
    }
    #[cfg(not(test))]
    let controls = Vec::new();
    #[cfg(test)]
    let controls = if _legacy_controls {
        let mut controls = Vec::new();
        caption(&mut runtime, host, "Root Properties", 20.0);
        for (text, y, initial, setter) in [
            (
                "Horizontal Orientation",
                50.0,
                true,
                (|m: &mut TreeModel, b| m.horizontal = b) as fn(&mut TreeModel, bool),
            ),
            (
                "Compress Tree",
                110.0,
                false,
                (|m: &mut TreeModel, b| m.compressed = b) as fn(&mut TreeModel, bool),
            ),
        ] {
            let model = model.clone();
            controls.push(add_checkbox(
                &mut runtime,
                host,
                Rectangle::new(PANEL_X, y, PANEL_WIDTH, CONTROL_HEIGHT),
                text,
                Arc::new(AtomicBool::new(initial)),
                move |value, context| {
                    setter(&mut model.lock().unwrap(), value);
                    invalidate_tree(&model, context);
                },
            ));
        }
        // Animation remains a separate presentation/viewport contract.
        for (text, label_y, scale_y, setter) in [
            (
                "Major Spacing: 10",
                150.0,
                180.0,
                (|m: &mut TreeModel, v| m.major = v) as fn(&mut TreeModel, f64),
            ),
            (
                "Minor Spacing: 10",
                220.0,
                250.0,
                (|m: &mut TreeModel, v| m.minor = v) as fn(&mut TreeModel, f64),
            ),
        ] {
            let caption_group = runtime
                .figure(host)
                .unwrap()
                .add(Box::new(
                    RectangleFigure::new_with_color(
                        PANEL_X,
                        label_y,
                        PANEL_WIDTH,
                        CONTROL_HEIGHT,
                        Color::WHITE,
                    )
                    .expect("valid figure bounds"),
                ))
                .unwrap();
            runtime
                .figure(caption_group)
                .unwrap()
                .add(Box::new(
                    LabelFigure::new(text)
                        .with_bounds(Rectangle::new(2.0, 0.0, PANEL_WIDTH, CONTROL_HEIGHT))
                        .expect("valid detached figure input"),
                ))
                .unwrap();
            let model = model.clone();
            controls.push(
                runtime
                    .figure(host)
                    .unwrap()
                    .add(Box::new(
                        ScaleFigure::new(
                            Rectangle::new(PANEL_X, scale_y, PANEL_WIDTH, CONTROL_HEIGHT),
                            SPACING_MIN,
                            SPACING_MAX,
                            Arc::new(Mutex::new(SPACING_DEFAULT)),
                            move |value, context| {
                                let value = value.round();
                                setter(&mut model.lock().unwrap(), value);
                                let prefix = text.split(':').next().unwrap();
                                replace_text_value_later(
                                    context,
                                    caption_group,
                                    format!("{prefix}: {value:.0}"),
                                    PANEL_WIDTH,
                                );
                                invalidate_tree(&model, context);
                            },
                        )
                        .with_increment(SPACING_INCREMENT),
                    ))
                    .unwrap(),
            );
        }
        caption(&mut runtime, host, "Selected Node:", 300.0);
        for (text, x, y, action) in [
            (
                "Align Center",
                PANEL_X,
                375.0,
                (|b: &mut Branch| b.centered = true) as fn(&mut Branch),
            ),
            (
                "Align Top/Left",
                PANEL_X + 155.0,
                375.0,
                (|b: &mut Branch| b.centered = false) as fn(&mut Branch),
            ),
            (
                "Normal",
                PANEL_X,
                415.0,
                (|b: &mut Branch| b.style = BranchStyle::Normal) as fn(&mut Branch),
            ),
            (
                "Hanging",
                PANEL_X + 155.0,
                415.0,
                (|b: &mut Branch| b.style = BranchStyle::Hanging) as fn(&mut Branch),
            ),
            (
                "expand/collapse",
                PANEL_X,
                455.0,
                (|b: &mut Branch| {
                    if !b.children.is_empty() {
                        b.expanded = !b.expanded;
                    }
                }) as fn(&mut Branch),
            ),
        ] {
            let model = model.clone();
            let selection = selection.clone();
            controls.push(add_button(
                &mut runtime,
                host,
                Rectangle::new(x, y, 150.0, CONTROL_HEIGHT),
                text,
                move |context| {
                    if let Some(index) = *selection.lock().unwrap() {
                        action(&mut model.lock().unwrap().branches[index]);
                        invalidate_tree(&model, context);
                    }
                },
            ));
        }
        let delete_model = model.clone();
        let delete_selection = selection.clone();
        controls.push(add_button(
            &mut runtime,
            host,
            Rectangle::new(PANEL_X + 155.0, 335.0, 150.0, CONTROL_HEIGHT),
            "Fewer Children",
            move |context| {
                let mut locked = delete_model.lock().unwrap();
                if let Some(parent) = *delete_selection.lock().unwrap()
                    && let Some(child) = locked.branches[parent].children.pop()
                {
                    let mut descendants = vec![child];
                    while let Some(index) = descendants.pop() {
                        locked.branches[index].active = false;
                        descendants.extend(locked.branches[index].children.iter().copied());
                    }
                    context.effects().remove_child_later(
                        locked.branches[parent].contents,
                        locked.branches[child].figure,
                    );
                    drop(locked);
                    invalidate_tree(&delete_model, context);
                }
            },
        ));
        let add_model = model.clone();
        let add_selection = selection.clone();
        controls.push(add_button(
            &mut runtime,
            host,
            Rectangle::new(PANEL_X, 335.0, 150.0, CONTROL_HEIGHT),
            "More Children",
            move |context| {
                let Some(parent) = *add_selection.lock().unwrap() else {
                    return;
                };
                let locked = add_model.lock().unwrap();
                let index = locked.branches.len();
                let parent_figure = locked.branches[parent].contents;
                let style = locked.branches[parent].style;
                drop(locked);
                let (subtree, contents, title, _) =
                    branch_subtree("child", index, add_selection.clone(), add_model.clone());
                let committed_model = add_model.clone();
                context
                    .effects()
                    .add_subtree_later(parent_figure, subtree, move |receipt| {
                        let Ok(receipt) = receipt else {
                            return;
                        };
                        let mut model = committed_model.lock().unwrap();
                        model.branches.push(Branch {
                            figure: receipt.root(),
                            contents: receipt.figure(contents).unwrap(),
                            title: receipt.figure(title).unwrap(),
                            children: Vec::new(),
                            style,
                            centered: true,
                            expanded: true,
                            active: true,
                        });
                        model.branches[parent].children.push(index);
                    });
            },
        ));
        controls
    } else {
        Vec::new()
    };
    Fixture {
        runtime,
        model,
        selection,
        controls,
    }
}

#[cfg(test)]
fn caption(runtime: &mut Runtime, host: FigureId, text: &str, y: f64) {
    runtime
        .figure(host)
        .unwrap()
        .add(Box::new(
            LabelFigure::new(text)
                .with_bounds(Rectangle::new(PANEL_X, y, PANEL_WIDTH, CONTROL_HEIGHT))
                .expect("valid detached figure input"),
        ))
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::render::{BackendCapabilities, SurfaceInfo};
    use novadraw::{event::MouseButton, geometry::Point};

    #[test]
    fn floating_panel_routes_canvas_selection_and_native_double_clicks() {
        use novadraw_platform_winit::WinitPointerAdapter;
        use std::time::Duration;
        use winit::event::{ElementState, MouseButton as NativeButton};

        fn panel_frame(
            panel: &mut Panel,
            context: &egui::Context,
            runtime: &mut Runtime,
            position: Point,
            pressed: Option<bool>,
        ) {
            let position = egui::pos2(position.x() as f32, position.y() as f32);
            let mut events = vec![egui::Event::PointerMoved(position)];
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                });
            }
            let _ = panel.prepare_ui(
                context,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                runtime,
            );
        }

        for animated in [false, true] {
            let fixture = canvas_fixture(false);
            let model = fixture.model.clone();
            let selection = fixture.selection.clone();
            model.lock().unwrap().use_animation = animated;
            let (mut runtime, panel) = panel_scene_from_fixture(fixture);
            let mut panel = panel.unwrap();
            let context = egui::Context::default();
            let mut adapter = WinitPointerAdapter::new();
            prepare(&mut runtime);
            let (title, child) = {
                let tree = model.lock().unwrap();
                (tree.branches[1].title, tree.branches[6].title)
            };
            for (base_time, expected_expanded) in [(0, false), (600, true)] {
                let bounds = runtime.tree().figure_bounds(title).unwrap();
                let position = runtime
                    .tree()
                    .local_to_surface_transform(title)
                    .unwrap()
                    .transform_point(Point::new(bounds.width / 2.0, bounds.height / 2.0));
                for _ in 0..4 {
                    panel_frame(&mut panel, &context, &mut runtime, position, None);
                }
                for (pressed, offset) in [(true, 0), (false, 10), (true, 100), (false, 110)] {
                    // egui-winit uses this ownership query before processing a button frame.
                    let consumed = context.egui_wants_pointer_input();
                    assert!(!consumed, "Tree title input must reach Canvas");
                    if !consumed {
                        adapter
                            .dispatch_button_input(
                                &mut runtime,
                                if pressed {
                                    ElementState::Pressed
                                } else {
                                    ElementState::Released
                                },
                                NativeButton::Left,
                                position,
                                Duration::from_millis(base_time + offset),
                            )
                            .unwrap();
                    }
                    panel_frame(&mut panel, &context, &mut runtime, position, Some(pressed));
                    if offset == 10 {
                        assert_eq!(*selection.lock().unwrap(), Some(1), "single click selects");
                        assert_eq!(
                            model.lock().unwrap().branches[1].expanded,
                            !expected_expanded,
                            "single click does not fold"
                        );
                        let label = runtime.tree().child_order(title).unwrap()[0];
                        assert_eq!(
                            runtime.tree().figure_style(label).unwrap().foreground,
                            Some(Color::WHITE)
                        );
                    }
                }
                prepare(&mut runtime);
                let tree = model.lock().unwrap();
                assert_eq!(tree.branches[1].expanded, expected_expanded);
                assert_eq!(
                    runtime.tree().is_effectively_visible(child).unwrap(),
                    expected_expanded
                );
                assert_eq!(tree.animation.is_some(), animated);
                assert!(tree.animation_error.is_none());
            }
            assert!(runtime.take_deferred_mutation_errors().is_empty());
        }
    }

    #[test]
    fn shared_connector_providers_replay_static_normal_hanging_and_nested_endpoints() {
        use novadraw::animation::{AnimationPlan, GeometryBinding, LayoutBounds, Motion, Tween};
        use novadraw::event::MonotonicTime;
        use novadraw::render::RenderCommandKind;
        use std::time::Duration;

        fn lines(runtime: &mut Runtime) -> Vec<[f64; 4]> {
            runtime.frames().request_full_redraw();
            let submission = runtime
                .frames()
                .prepare_submission(
                    SurfaceInfo {
                        logical_width: 1200.0,
                        logical_height: 800.0,
                        pixel_width: 1200,
                        pixel_height: 800,
                        scale_factor: 1.0,
                    },
                    BackendCapabilities::RETAINED_PARTIAL.with_linear_gradients(),
                )
                .into_ready()
                .unwrap();
            let mut result = submission
                .commands
                .iter()
                .filter_map(|command| {
                    if let RenderCommandKind::Line { p1, p2, .. } = command.kind {
                        Some([p1.x(), p1.y(), p2.x(), p2.y()])
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            result.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let _ = runtime
                .frames()
                .complete_submission(
                    submission.session_id,
                    submission.frame_id,
                    Ok(novadraw::render::RenderDisposition::Presented),
                )
                .unwrap();
            result
        }
        for horizontal in [true, false] {
            let mut fixture = canvas_fixture(false);
            fixture.model.lock().unwrap().horizontal = horizontal;
            let before = lines(&mut fixture.runtime);
            let root = fixture.model.lock().unwrap().branches[0].figure;
            let bounds = fixture.runtime.tree().figure_bounds(root).unwrap();
            let channel = fixture
                .runtime
                .animation()
                .timeline()
                .bind_geometry(root, GeometryBinding::layout_bounds())
                .unwrap();
            let plan = AnimationPlan::track(
                channel,
                Motion::Tween(
                    Tween::between(
                        LayoutBounds::try_new(bounds).unwrap(),
                        LayoutBounds::try_new(bounds).unwrap(),
                        Duration::from_millis(210),
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
            let plan = super::super::tree_layout::with_tree_connectors(
                plan,
                &fixture.model.lock().unwrap(),
            );
            fixture.runtime.animation().timeline().start(plan).unwrap();
            fixture
                .runtime
                .animation()
                .advance_time(MonotonicTime::from_micros(0))
                .unwrap();
            let animated = lines(&mut fixture.runtime);
            assert_eq!(before, animated, "orientation {horizontal}");
            fixture
                .runtime
                .animation()
                .advance_time(MonotonicTime::from_micros(210_000))
                .unwrap();
            assert_eq!(before, lines(&mut fixture.runtime));
        }
    }

    #[test]
    fn full_official_tree_has_nineteen_nodes_and_five_root_branches() {
        let mut fixture = fixture();
        {
            let model = fixture.model.lock().unwrap();
            assert_eq!(model.branches.len(), 19);
            assert_eq!(model.branches[0].children, [1, 2, 9, 10, 15]);
            assert_eq!(model.branches[5].children, [6, 7, 8]);
            assert_eq!(model.branches[12].children, [13, 14]);
            assert_eq!(model.branches[11].style, BranchStyle::Hanging);
            assert_eq!(model.branches[12].style, BranchStyle::Hanging);
            for branch in &model.branches {
                assert_eq!(
                    fixture.runtime.tree().child_order(branch.figure).unwrap(),
                    &[branch.contents, branch.title]
                );
            }
        }
        let result = fixture.runtime.frames().prepare_submission(
            SurfaceInfo {
                logical_width: 1200.0,
                logical_height: 800.0,
                pixel_width: 1200,
                pixel_height: 800,
                scale_factor: 1.0,
            },
            BackendCapabilities::RETAINED_PARTIAL.with_linear_gradients(),
        );
        assert!(result.into_ready().is_ok());
    }

    fn prepare(runtime: &mut Runtime) {
        let preparation = runtime.frames().prepare_submission(
            SurfaceInfo {
                logical_width: 1200.0,
                logical_height: 800.0,
                pixel_width: 1200,
                pixel_height: 800,
                scale_factor: 1.0,
            },
            BackendCapabilities::RETAINED_PARTIAL.with_linear_gradients(),
        );
        assert!(
            !matches!(preparation, novadraw::runtime::FramePreparation::Error(_)),
            "{preparation:?}"
        );
        if let novadraw::runtime::FramePreparation::Ready(submission) = preparation {
            let _ = runtime
                .frames()
                .complete_submission(
                    submission.session_id,
                    submission.frame_id,
                    Ok(novadraw::render::RenderDisposition::Presented),
                )
                .unwrap();
        }
    }

    fn click(runtime: &mut Runtime, figure: FigureId) {
        let bounds = runtime.tree().figure_bounds(figure).unwrap();
        let point = runtime
            .tree()
            .local_to_surface_transform(figure)
            .unwrap()
            .transform_point(Point::new(bounds.width / 2.0, bounds.height / 2.0));
        runtime
            .input()
            .dispatch_mouse_pressed(point.x(), point.y(), MouseButton::Left);
        runtime
            .input()
            .dispatch_mouse_released(point.x(), point.y(), MouseButton::Left);
    }

    #[test]
    fn selecting_another_page_restores_gradient_and_inherited_text() {
        let mut fixture = fixture();
        prepare(&mut fixture.runtime);
        let titles = {
            let model = fixture.model.lock().unwrap();
            [model.branches[1].title, model.branches[2].title]
        };
        for (index, title) in titles.into_iter().enumerate() {
            click(&mut fixture.runtime, title);
            prepare(&mut fixture.runtime);
            for (other, other_title) in titles.into_iter().enumerate() {
                let label = fixture.runtime.tree().child_order(other_title).unwrap()[0];
                assert_eq!(
                    fixture
                        .runtime
                        .tree()
                        .figure_style(label)
                        .unwrap()
                        .foreground,
                    if other == index {
                        Some(Color::WHITE)
                    } else {
                        None
                    }
                );
            }
        }
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn selection_style_collapse_and_delete_follow_official_actions() {
        let mut fixture = fixture();
        prepare(&mut fixture.runtime);
        let title = fixture.model.lock().unwrap().branches[1].title;
        click(&mut fixture.runtime, title);
        assert_eq!(*fixture.selection.lock().unwrap(), Some(1));
        // Controls: orientation, compression, major, minor, center, left,
        // normal, hanging, expand/collapse, fewer children.
        click(&mut fixture.runtime, fixture.controls[7]);
        prepare(&mut fixture.runtime);
        assert_eq!(
            fixture.model.lock().unwrap().branches[1].style,
            BranchStyle::Hanging
        );
        click(&mut fixture.runtime, fixture.controls[8]);
        prepare(&mut fixture.runtime);
        let contents = fixture.model.lock().unwrap().branches[1].contents;
        assert!(
            !fixture
                .runtime
                .tree()
                .is_effectively_visible(contents)
                .unwrap()
        );
        click(&mut fixture.runtime, fixture.controls[8]);
        prepare(&mut fixture.runtime);
        assert!(
            fixture
                .runtime
                .tree()
                .is_effectively_visible(contents)
                .unwrap()
        );
        click(&mut fixture.runtime, fixture.controls[9]);
        prepare(&mut fixture.runtime);
        let model = fixture.model.lock().unwrap();
        assert_eq!(model.branches[1].children, [3, 4]);
        assert!(!model.branches[5].active);
        assert!(!model.branches[6].active);
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn spacing_scales_use_official_five_pixel_keyboard_increment_and_update_caption() {
        use novadraw::event::{Key, KeyModifiers};
        let mut fixture = fixture();
        prepare(&mut fixture.runtime);
        let host = fixture.runtime.tree().contents().unwrap();
        let before = fixture.runtime.tree().descendant_ids(host).unwrap();
        fixture
            .runtime
            .focus()
            .request(fixture.controls[2])
            .unwrap();
        fixture
            .runtime
            .input()
            .dispatch_key_pressed(Key::ArrowRight, KeyModifiers::default());
        prepare(&mut fixture.runtime);
        assert_eq!(fixture.model.lock().unwrap().major, 15.0);
        let after = fixture.runtime.tree().descendant_ids(host).unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(after.iter().filter(|id| !before.contains(id)).count(), 1);
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }

    #[test]
    fn more_children_mounts_full_official_branch_and_inherits_parent_style() {
        let mut fixture = fixture();
        prepare(&mut fixture.runtime);
        let parent_title = fixture.model.lock().unwrap().branches[11].title;
        click(&mut fixture.runtime, parent_title);
        click(&mut fixture.runtime, fixture.controls[10]);
        prepare(&mut fixture.runtime);
        let child_title = {
            let model = fixture.model.lock().unwrap();
            assert_eq!(model.branches.len(), 20);
            assert_eq!(model.branches[11].children, [19]);
            let child = &model.branches[19];
            assert_eq!(child.style, BranchStyle::Hanging);
            assert_eq!(
                fixture.runtime.tree().child_order(child.figure).unwrap(),
                [child.contents, child.title]
            );
            assert_eq!(
                fixture
                    .runtime
                    .tree()
                    .child_order(child.title)
                    .unwrap()
                    .len(),
                1
            );
            child.title
        };
        click(&mut fixture.runtime, child_title);
        assert_eq!(*fixture.selection.lock().unwrap(), Some(19));
        let previous_label = fixture.runtime.tree().child_order(parent_title).unwrap()[0];
        let selected_label = fixture.runtime.tree().child_order(child_title).unwrap()[0];
        assert_eq!(
            fixture
                .runtime
                .tree()
                .figure_style(previous_label)
                .unwrap()
                .foreground,
            None
        );
        assert_eq!(
            fixture
                .runtime
                .tree()
                .figure_style(selected_label)
                .unwrap()
                .foreground,
            Some(Color::WHITE)
        );
        click(&mut fixture.runtime, fixture.controls[10]);
        prepare(&mut fixture.runtime);
        assert_eq!(fixture.model.lock().unwrap().branches[19].children, [20]);
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }
}
