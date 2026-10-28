//! Tree-only endpoint construction; scheduling, geometry and input ownership remain in Core.
use super::tree_layout::{ContentsLayout, TreeModel, with_tree_connectors};
use novadraw::{
    animation::{
        AnimationPlan, AnimationStart, GeometryBinding, GeometryPatch, GeometryProvider,
        GeometryProviderKey, LayoutBounds, Motion, PresentedVisibility, Tween, ViewportOrigin,
    },
    container::RangeModelSnapshot,
    geometry::{Affine2D, Point, Rectangle},
    runtime::Runtime,
    tree::FigureId,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

const DURATION: Duration = Duration::from_millis(210);
type Failure = Box<dyn std::error::Error>;

fn clamp(value: Point, horizontal: RangeModelSnapshot, vertical: RangeModelSnapshot) -> Point {
    Point::new(
        value.x().clamp(
            horizontal.minimum,
            (horizontal.maximum - horizontal.extent).max(horizontal.minimum),
        ),
        value.y().clamp(
            vertical.minimum,
            (vertical.maximum - vertical.extent).max(vertical.minimum),
        ),
    )
}

/// Consumes a queued operation after input dispatch and before the host prepares a frame.
pub(super) fn process(runtime: &mut Runtime, model: &Arc<Mutex<TreeModel>>) {
    let request = model.lock().expect("tree model").pending_toggle.take();
    if let Some(index) = request {
        let result = toggle(runtime, model, index);
        model.lock().expect("tree model").animation_error =
            result.err().map(|error| error.to_string());
    }
}

pub(super) fn toggle(
    runtime: &mut Runtime,
    model: &Arc<Mutex<TreeModel>>,
    index: usize,
) -> Result<(), Failure> {
    runtime.frames().stabilize_for_query()?;
    let (root, viewport, marked, expanded, enabled) = {
        let tree = model.lock().expect("tree model");
        (
            tree.branches[0].figure,
            tree.viewport.expect("Tree viewport"),
            tree.branches[index].title,
            tree.branches[index].expanded,
            tree.use_animation,
        )
    };
    let enabled = enabled
        && runtime.animation().timeline().mode() == novadraw::animation::AnimationMode::Enabled;
    let before = if enabled {
        Some(runtime.animation().capture_geometry([viewport])?)
    } else {
        None
    };
    {
        let mut tree = model.lock().expect("tree model");
        tree.branches[index].expanded = !expanded;
        let contents = tree.branches[0].contents;
        runtime
            .figure(contents)?
            .set_layout_manager(Box::new(ContentsLayout {
                model: model.clone(),
                index: 0,
            }))?;
    }
    runtime.frames().stabilize_for_query()?;
    if !enabled {
        return Ok(());
    }
    let before = before.expect("enabled capture");
    let tree = model.lock().expect("tree model");
    let mut order = vec![root];
    let mut cursor = 0;
    while cursor < order.len() {
        order.extend(runtime.tree().child_order(order[cursor])?);
        cursor += 1;
    }
    let final_bounds = order
        .iter()
        .map(|id| Ok((*id, runtime.tree().figure_bounds(*id)?)))
        .collect::<Result<HashMap<_, _>, novadraw::tree::TreeQueryError>>()?;
    let mut terminal = final_bounds.clone();
    let mut initial = order
        .iter()
        .map(|id| Ok((*id, before.geometry().figure(*id)?.bounds)))
        .collect::<Result<HashMap<_, _>, novadraw::animation::AnimationError>>()?;
    let mut descendants = tree.branches[index].children.clone();
    let mut visible_descendants = Vec::new();
    while let Some(child) = descendants.pop() {
        let branch = &tree.branches[child];
        let visible = if expanded {
            before.geometry().figure(branch.title)?.effective_visible
        } else {
            runtime.tree().is_effectively_visible(branch.title)?
        };
        if !visible {
            continue;
        }
        visible_descendants.push(child);
        if branch.expanded {
            descendants.extend(branch.children.iter().copied());
        }
    }
    if expanded {
        collapse(&tree, index, &visible_descendants, &mut terminal, runtime)?;
    } else {
        // Newly visible nodes start at the explicit nonzero collapsed endpoint. A reversing
        // node already visible in the captured E keeps that exact captured geometry.
        let mut collapsed = final_bounds.clone();
        collapse(&tree, index, &visible_descendants, &mut collapsed, runtime)?;
        for id in &order {
            if !before.geometry().figure(*id)?.effective_visible {
                initial.insert(*id, collapsed[id]);
            }
        }
        let contents = tree.branches[index].contents;
        if !before.geometry().figure(contents)?.effective_visible {
            initial.insert(contents, collapsed[&contents]);
        }
    }
    let handle = runtime.viewports().handle(viewport)?;
    let old_viewport = before.geometry().figure(viewport)?;
    let offset = old_viewport.child_transform.transform_point(Point::ZERO);
    let origin = Point::new(
        old_viewport.client_box.x - offset.x(),
        old_viewport.client_box.y - offset.y(),
    );
    let anchor = before
        .geometry()
        .to_content_transform(marked, viewport)?
        .ok_or(novadraw::animation::AnimationError::InvalidValue)?
        .transform_point(Point::ZERO);
    // Endpoint normalization preserves the operated title's committed world placement.
    // Use the engine edge transforms, with the viewport scroll/client mapping removed.
    let to_content = runtime
        .tree()
        .child_content_to_surface_transform(viewport)?
        .inverse()
        .ok_or(novadraw::animation::AnimationError::InvalidValue)?;
    let position = (to_content * runtime.tree().local_to_surface_transform(marked)?)
        .transform_point(Point::ZERO);
    let horizontal = handle.horizontal_range();
    let vertical = handle.vertical_range();
    let final_origin = clamp(
        Point::new(
            origin.x() + position.x() - anchor.x(),
            origin.y() + position.y() - anchor.y(),
        ),
        horizontal,
        vertical,
    );
    // This is the one source write for the final RangeModel value. An active old plan may be
    // replaced after this source boundary; its captured owned E remains the initial authority.
    let mut tracks = Vec::new();
    for id in &order {
        if initial[id] != terminal[id] {
            let channel = runtime
                .animation()
                .timeline()
                .bind_geometry(*id, GeometryBinding::layout_bounds())?;
            runtime
                .animation()
                .timeline()
                .refresh_geometry(*id, GeometryBinding::layout_bounds())?;
            tracks.push(AnimationPlan::geometry_track(
                channel,
                Motion::Tween(Tween::between(
                    LayoutBounds::try_new(initial[id])?,
                    LayoutBounds::try_new(terminal[id])?,
                    DURATION,
                )?),
            )?);
        }
        // Retain only nodes captured visible or legitimately revealed by this expansion.
        let visible = before.geometry().figure(*id)?.effective_visible
            || runtime.tree().is_effectively_visible(*id)?;
        if visible && !runtime.tree().is_visible(*id)? {
            let channel = runtime
                .animation()
                .timeline()
                .bind_geometry(*id, GeometryBinding::visibility())?;
            runtime
                .animation()
                .timeline()
                .refresh_geometry(*id, GeometryBinding::visibility())?;
            tracks.push(AnimationPlan::geometry_track(
                channel,
                Motion::Tween(Tween::between(
                    PresentedVisibility(true),
                    PresentedVisibility(true),
                    DURATION,
                )?),
            )?);
        }
    }
    // A stable bounds track keeps the group finite even when no rectangle changed.
    if tracks.is_empty() {
        let channel = runtime
            .animation()
            .timeline()
            .bind_geometry(root, GeometryBinding::layout_bounds())?;
        tracks.push(AnimationPlan::geometry_track(
            channel,
            Motion::Tween(Tween::between(
                LayoutBounds::try_new(initial[&root])?,
                LayoutBounds::try_new(terminal[&root])?,
                DURATION,
            )?),
        )?);
    }
    let plan = with_tree_connectors(AnimationPlan::parallel(tracks)?, &tree)
        .with_geometry_provider(GeometryProvider::new(
            GeometryProviderKey::<ViewportOrigin>::new(viewport),
            [viewport, marked],
            vec![],
            move |context| {
                let marked_position = context
                    .to_content_transform(marked, viewport)?
                    .ok_or(novadraw::animation::AnimationError::InvalidValue)?
                    .transform_point(Point::ZERO);
                Ok(ViewportOrigin(clamp(
                    Point::new(
                        origin.x() + marked_position.x() - anchor.x(),
                        origin.y() + marked_position.y() - anchor.y(),
                    ),
                    horizontal,
                    vertical,
                )))
            },
            |value| {
                Ok(GeometryPatch {
                    child_transform: Some(Affine2D::from_translation(-value.0.x(), -value.0.y())),
                    ..Default::default()
                })
            },
        ));
    drop(tree);
    let start = runtime
        .animation()
        .start_viewport_plan(viewport, final_origin, plan)?;
    model.lock().expect("tree model").animation = match start {
        AnimationStart::Running(id) | AnimationStart::Existing(id) => Some(id),
        AnimationStart::Suppressed(_) => None,
        _ => return Err(novadraw::animation::AnimationError::InvalidValue.into()),
    };
    Ok(())
}

fn collapse(
    tree: &TreeModel,
    index: usize,
    descendants: &[usize],
    rectangles: &mut HashMap<FigureId, Rectangle>,
    runtime: &Runtime,
) -> Result<(), Failure> {
    // Inside-out nonzero envelopes; every child title shares its enclosing branch's center.
    let mut inside_out = descendants.to_vec();
    inside_out.sort_unstable();
    let descendants: HashSet<_> = descendants.iter().copied().collect();
    for child in inside_out.iter().rev() {
        let branch = &tree.branches[*child];
        let title = rectangles[&branch.title];
        let mut width = title.width;
        let mut height = title.height;
        for child in &branch.children {
            if descendants.contains(child) {
                let rect = rectangles[&tree.branches[*child].figure];
                width = width.max(rect.width);
                height = height.max(rect.height);
            }
        }
        rectangles.insert(
            branch.title,
            Rectangle::new(
                (width - title.width) / 2.0,
                (height - title.height) / 2.0,
                title.width,
                title.height,
            ),
        );
        rectangles.insert(branch.contents, Rectangle::new(0.0, 0.0, width, height));
        rectangles.insert(branch.figure, Rectangle::new(0.0, 0.0, width, height));
        for child in &branch.children {
            if descendants.contains(child) {
                let id = tree.branches[*child].figure;
                let rect = rectangles[&id];
                rectangles.insert(
                    id,
                    Rectangle::new(
                        (width - rect.width) / 2.0,
                        (height - rect.height) / 2.0,
                        rect.width,
                        rect.height,
                    ),
                );
            }
        }
    }
    let branch = &tree.branches[index];
    let title = rectangles[&branch.title];
    let width = branch
        .children
        .iter()
        .filter(|id| descendants.contains(id))
        .map(|id| rectangles[&tree.branches[*id].figure].width)
        .fold(title.width, f64::max);
    let height = branch
        .children
        .iter()
        .filter(|id| descendants.contains(id))
        .map(|id| rectangles[&tree.branches[*id].figure].height)
        .fold(title.height, f64::max);
    rectangles.insert(
        branch.contents,
        Rectangle::new(
            title.x + (title.width - width) / 2.0,
            title.y + (title.height - height) / 2.0,
            width,
            height,
        ),
    );
    for child in &branch.children {
        if descendants.contains(child) {
            let id = tree.branches[*child].figure;
            let rect = rectangles[&id];
            rectangles.insert(
                id,
                Rectangle::new(
                    (width - rect.width) / 2.0,
                    (height - rect.height) / 2.0,
                    rect.width,
                    rect.height,
                ),
            );
        }
    }
    // Expand ancestors without changing existing world placements or clipping strategy.
    let mut current = branch.figure;
    loop {
        let children = runtime.tree().child_order(current)?;
        let insets = runtime.tree().insets(current)?;
        let old = rectangles[&current];
        let mut envelope = Rectangle::new(0.0, 0.0, old.width, old.height);
        for child in &children {
            if let Some(rect) = rectangles.get(child) {
                envelope = envelope.union(Rectangle::new(
                    rect.x + insets.left,
                    rect.y + insets.top,
                    rect.width + insets.right,
                    rect.height + insets.bottom,
                ));
            }
        }
        for child in children {
            if let Some(rect) = rectangles.get_mut(&child) {
                rect.x -= envelope.x;
                rect.y -= envelope.y;
            }
        }
        rectangles.insert(
            current,
            Rectangle::new(
                old.x + envelope.x,
                old.y + envelope.y,
                envelope.width,
                envelope.height,
            ),
        );
        if current == tree.branches[0].figure {
            break;
        }
        current = runtime.tree().parent_id(current)?.expect("branch ancestor");
    }
    Ok(())
}

pub(crate) fn verify_native(window: Arc<winit::window::Window>) {
    use novadraw::{
        event::MonotonicTime,
        render::{RenderBackend, RenderDisposition, SurfaceInfo},
    };
    use novadraw_backend_vello::VelloRenderer;
    let output = std::path::Path::new("target/verification/tree-animation");
    std::fs::create_dir_all(output).unwrap();
    let mut work_evidence =
        String::from("orientation,dpi,phase,nodes,edges,matrices,clips,envelopes\n");
    for horizontal in [true, false] {
        for dpi in [1_u32, 2] {
            let surface = SurfaceInfo {
                logical_width: 1200.0,
                logical_height: 800.0,
                pixel_width: 1200 * dpi,
                pixel_height: 800 * dpi,
                scale_factor: f64::from(dpi),
            };
            let mut fixture = super::tree_example::canvas_fixture(false);
            fixture.model.lock().unwrap().horizontal = horizontal;
            fixture.model.lock().unwrap().use_animation = true;
            let mut renderer = VelloRenderer::new(window.clone(), surface).unwrap();
            let prefix = format!(
                "{}-{dpi}",
                if horizontal { "horizontal" } else { "vertical" }
            );
            let mut frames = Vec::new();
            for (phase, micros) in [
                ("baseline", 0),
                ("start", 0),
                ("middle", 105_000),
                ("terminal", 209_999),
                ("final", 210_000),
            ] {
                if phase == "start" {
                    toggle(&mut fixture.runtime, &fixture.model, 1).unwrap();
                }
                fixture
                    .runtime
                    .animation()
                    .advance_time(MonotonicTime::from_micros(micros))
                    .unwrap();
                let prepared = fixture
                    .runtime
                    .frames()
                    .prepare_submission(surface, renderer.capabilities());
                let owner = fixture.model.lock().unwrap().animation;
                let state = owner.map(|id| fixture.runtime.animation().timeline().state(id));
                let failure = owner.map(|id| {
                    fixture
                        .runtime
                        .animation()
                        .timeline()
                        .failure(id)
                        .map(|error| error.cloned())
                });
                let viewport = fixture.model.lock().unwrap().viewport.unwrap();
                let geometry = fixture.runtime.animation().geometry().map(|view| {
                    (
                        view.version(),
                        view.figure(viewport).map(|node| {
                            (node.bounds, node.effective_visible, node.visual_envelope)
                        }),
                    )
                });
                let submission = prepared.into_ready().unwrap_or_else(|error| {
                    panic!("{prefix}/{phase}: {error:?}, {state:?}, {failure:?}, {geometry:?}")
                });
                if phase == "middle" || phase == "final" {
                    assert_eq!(
                        submission.damage.mode(),
                        novadraw::render::DamageMode::Partial
                    );
                }
                if let Ok(view) = fixture.runtime.animation().geometry() {
                    let work = view.materialization_work();
                    work_evidence.push_str(&format!(
                        "{horizontal},{dpi},{phase},{},{},{},{},{}\n",
                        work.nodes, work.edges, work.matrices, work.clips, work.envelopes
                    ));
                }
                let disposition = renderer.render_for_screenshot(&submission).unwrap();
                assert!(
                    disposition == RenderDisposition::Presented
                        || phase == "start" && disposition == RenderDisposition::Skipped,
                    "{prefix}/{phase}: {disposition:?}"
                );
                let _ = fixture
                    .runtime
                    .frames()
                    .complete_submission(
                        submission.session_id,
                        submission.frame_id,
                        Ok(disposition),
                    )
                    .unwrap();
                let path = output.join(format!("{prefix}-{phase}.png"));
                renderer.screenshot(&path).unwrap();
                frames.push(image::open(path).unwrap().to_rgba8());
                if phase == "middle" {
                    // Retained partial rendering must equal an independently recorded full frame.
                    fixture.runtime.frames().request_full_redraw();
                    let full = fixture
                        .runtime
                        .frames()
                        .prepare_submission(surface, renderer.capabilities())
                        .into_ready()
                        .unwrap();
                    assert_eq!(
                        renderer.render_for_screenshot(&full),
                        Ok(RenderDisposition::Presented)
                    );
                    let _ = fixture
                        .runtime
                        .frames()
                        .complete_submission(
                            full.session_id,
                            full.frame_id,
                            Ok(RenderDisposition::Presented),
                        )
                        .unwrap();
                    let full_path = output.join(format!("{prefix}-middle-full.png"));
                    renderer.screenshot(&full_path).unwrap();
                    assert!(
                        *frames.last().unwrap() == image::open(full_path).unwrap().to_rgba8(),
                        "middle partial/full pixels differ"
                    );
                }
            }
            assert!(frames[0] == frames[1], "first frame changed");
            assert!(frames[1] != frames[2], "missing intermediate geometry");
            assert!(frames[2] != frames[4], "missing static completion");
            fixture.runtime.frames().request_full_redraw();
            let full = fixture
                .runtime
                .frames()
                .prepare_submission(surface, renderer.capabilities())
                .into_ready()
                .unwrap();
            assert_eq!(
                renderer.render_for_screenshot(&full),
                Ok(RenderDisposition::Presented)
            );
            let _ = fixture
                .runtime
                .frames()
                .complete_submission(
                    full.session_id,
                    full.frame_id,
                    Ok(RenderDisposition::Presented),
                )
                .unwrap();
            let path = output.join(format!("{prefix}-final-full.png"));
            renderer.screenshot(&path).unwrap();
            assert!(
                frames[4] == image::open(path).unwrap().to_rgba8(),
                "partial cleanup left old pixels"
            );
            drop(renderer);
            for (name, mode) in [
                ("static", novadraw::animation::AnimationMode::Enabled),
                ("disabled", novadraw::animation::AnimationMode::Disabled),
                ("reduced", novadraw::animation::AnimationMode::ReducedMotion),
            ] {
                let mut static_tree = super::tree_example::canvas_fixture(false);
                static_tree.model.lock().unwrap().horizontal = horizontal;
                static_tree.model.lock().unwrap().use_animation = name != "static";
                static_tree.runtime.animation().timeline().set_mode(mode);
                static_tree.runtime.frames().stabilize_for_query().unwrap();
                toggle(&mut static_tree.runtime, &static_tree.model, 1).unwrap();
                let mut static_renderer = VelloRenderer::new(window.clone(), surface).unwrap();
                let static_frame = static_tree
                    .runtime
                    .frames()
                    .prepare_submission(surface, static_renderer.capabilities())
                    .into_ready()
                    .unwrap();
                assert_eq!(
                    static_renderer.render_for_screenshot(&static_frame),
                    Ok(RenderDisposition::Presented)
                );
                let path = output.join(format!("{prefix}-{name}.png"));
                static_renderer.screenshot(&path).unwrap();
                assert!(
                    frames[4] == image::open(path).unwrap().to_rgba8(),
                    "final static pixels differ"
                );
            }
        }
    }
    std::fs::write(output.join("geometry-work.csv"), work_evidence).unwrap();
    println!(
        "Tree Native GPU evidence passed: both orientations, DPI 1/2, first/intermediate/final frames and partial cleanup."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        animation::{AnimationMode, AnimationState},
        event::MonotonicTime,
    };

    fn center(
        geometry: &novadraw::animation::PresentationGeometry,
        title: FigureId,
        viewport: FigureId,
    ) -> Point {
        let bounds = geometry.figure(title).unwrap().bounds;
        geometry
            .to_content_transform(title, viewport)
            .unwrap()
            .unwrap()
            .transform_point(Point::new(bounds.width / 2.0, bounds.height / 2.0))
    }

    #[test]
    fn native_press_release_pairs_toggle_tree_with_animation_on_and_off() {
        use novadraw_platform_winit::WinitPointerAdapter;
        use winit::event::{ElementState, MouseButton};
        for animated in [false, true] {
            let mut fixture = super::super::tree_example::canvas_fixture(false);
            fixture.runtime.frames().stabilize_for_query().unwrap();
            fixture.model.lock().unwrap().use_animation = animated;
            let (title, child) = {
                let tree = fixture.model.lock().unwrap();
                (tree.branches[1].title, tree.branches[6].title)
            };
            let mut adapter = WinitPointerAdapter::new();
            for (base_time, expected_expanded) in [(0, false), (600, true)] {
                let bounds = fixture.runtime.tree().figure_bounds(title).unwrap();
                let position = fixture
                    .runtime
                    .tree()
                    .local_to_surface_transform(title)
                    .unwrap()
                    .transform_point(Point::new(bounds.width / 2.0, bounds.height / 2.0));
                for (state, offset) in [
                    (ElementState::Pressed, 0),
                    (ElementState::Released, 10),
                    (ElementState::Pressed, 100),
                    (ElementState::Released, 110),
                ] {
                    adapter
                        .dispatch_button_input(
                            &mut fixture.runtime,
                            state,
                            MouseButton::Left,
                            position,
                            Duration::from_millis(base_time + offset),
                        )
                        .unwrap();
                }
                process(&mut fixture.runtime, &fixture.model);
                fixture.runtime.frames().stabilize_for_query().unwrap();
                let tree = fixture.model.lock().unwrap();
                assert_eq!(tree.branches[1].expanded, expected_expanded);
                assert_eq!(
                    fixture
                        .runtime
                        .tree()
                        .is_effectively_visible(child)
                        .unwrap(),
                    expected_expanded
                );
                assert_eq!(tree.animation.is_some(), animated);
            }
        }
    }

    #[test]
    fn tree_collapse_preserves_first_frame_and_converges_all_descendant_centers() {
        for horizontal in [true, false] {
            let mut fixture = super::super::tree_example::canvas_fixture(false);
            fixture.model.lock().unwrap().horizontal = horizontal;
            fixture.model.lock().unwrap().use_animation = true;
            let viewport = fixture.model.lock().unwrap().viewport.unwrap();
            let before = fixture
                .runtime
                .animation()
                .capture_geometry([viewport])
                .unwrap();
            let (marked, child) = {
                let tree = fixture.model.lock().unwrap();
                (tree.branches[1].title, tree.branches[6].title)
            };
            let old_center = center(before.geometry(), child, viewport);
            toggle(&mut fixture.runtime, &fixture.model, 1).unwrap();
            fixture
                .runtime
                .animation()
                .advance_time(MonotonicTime::from_micros(0))
                .unwrap();
            let first = fixture
                .runtime
                .animation()
                .capture_geometry([viewport])
                .unwrap();
            assert_eq!(old_center, center(first.geometry(), child, viewport));
            assert!(first.geometry().figure(child).unwrap().effective_visible);
            assert!(
                !fixture
                    .runtime
                    .tree()
                    .is_effectively_visible(child)
                    .unwrap()
            );
            fixture
                .runtime
                .animation()
                .advance_time(MonotonicTime::from_micros(209_999))
                .unwrap();
            let last = fixture
                .runtime
                .animation()
                .capture_geometry([viewport])
                .unwrap();
            let target = center(last.geometry(), marked, viewport);
            let tree = fixture.model.lock().unwrap();
            for index in [3, 4, 5, 6, 7, 8] {
                let actual = center(last.geometry(), tree.branches[index].title, viewport);
                assert!(
                    (actual.x() - target.x()).abs() < 0.01,
                    "{index}: {actual:?}, {target:?}"
                );
                assert!(
                    (actual.y() - target.y()).abs() < 0.01,
                    "{index}: {actual:?}, {target:?}"
                );
            }
            drop(tree);
            fixture
                .runtime
                .animation()
                .advance_time(MonotonicTime::from_micros(210_000))
                .unwrap();
            let final_geometry = fixture
                .runtime
                .animation()
                .capture_geometry([viewport])
                .unwrap();
            assert!(
                !final_geometry
                    .geometry()
                    .figure(child)
                    .unwrap()
                    .effective_visible
            );
        }
    }

    #[test]
    fn tree_reverse_uses_latest_unrecorded_sample_and_scroll_cancels_whole_group() {
        let mut fixture = super::super::tree_example::canvas_fixture(false);
        fixture.model.lock().unwrap().use_animation = true;
        let viewport = fixture.model.lock().unwrap().viewport.unwrap();
        let child = fixture.model.lock().unwrap().branches[6].title;
        toggle(&mut fixture.runtime, &fixture.model, 1).unwrap();
        fixture
            .runtime
            .animation()
            .advance_time(MonotonicTime::from_micros(0))
            .unwrap();
        fixture
            .runtime
            .animation()
            .advance_time(MonotonicTime::from_micros(50_000))
            .unwrap();
        let current = fixture
            .runtime
            .animation()
            .capture_geometry([viewport])
            .unwrap();
        let sample = fixture.runtime.animation().timeline().sample_version().time;
        let current_center = center(current.geometry(), child, viewport);
        toggle(&mut fixture.runtime, &fixture.model, 1).unwrap();
        assert_eq!(
            fixture.runtime.animation().timeline().sample_version().time,
            sample
        );
        let reverse = fixture
            .runtime
            .animation()
            .capture_geometry([viewport])
            .unwrap();
        assert_eq!(current_center, center(reverse.geometry(), child, viewport));
        fixture
            .runtime
            .animation()
            .advance_time(MonotonicTime::from_micros(75_000))
            .unwrap();
        let captured = fixture
            .runtime
            .animation()
            .capture_geometry([viewport])
            .unwrap();
        let origin = captured
            .geometry()
            .value::<ViewportOrigin>(viewport)
            .unwrap()
            .0;
        let handle = fixture.runtime.viewports().handle(viewport).unwrap();
        let expected = clamp(
            Point::new(origin.x() + 10.0, origin.y() + 20.0),
            handle.horizontal_range(),
            handle.vertical_range(),
        );
        let owner = fixture.model.lock().unwrap().animation.unwrap();
        fixture
            .runtime
            .viewports()
            .scroll_by(viewport, 10.0, 20.0)
            .unwrap();
        assert_eq!(handle.view_location(), expected);
        assert_eq!(
            fixture.runtime.animation().timeline().state(owner).unwrap(),
            AnimationState::Cancelled
        );
        assert_eq!(
            fixture
                .runtime
                .animation()
                .timeline()
                .active_animation_count(),
            0
        );
    }

    #[test]
    fn first_expand_preserves_nested_hidden_branch_and_repeated_reversal_anchor() {
        let mut fixture = super::super::tree_example::canvas_fixture(false);
        // Collapse a nested branch statically, then its parent. Expanding the parent must
        // reveal the nested title while keeping that branch's descendants hidden.
        toggle(&mut fixture.runtime, &fixture.model, 5).unwrap();
        toggle(&mut fixture.runtime, &fixture.model, 1).unwrap();
        fixture.model.lock().unwrap().use_animation = true;
        let (viewport, nested, hidden) = {
            let tree = fixture.model.lock().unwrap();
            (
                tree.viewport.unwrap(),
                tree.branches[5].title,
                tree.branches[6].title,
            )
        };
        toggle(&mut fixture.runtime, &fixture.model, 1).unwrap();
        fixture
            .runtime
            .animation()
            .advance_time(MonotonicTime::from_micros(0))
            .unwrap();
        let first = fixture
            .runtime
            .animation()
            .capture_geometry([viewport])
            .unwrap();
        assert!(first.geometry().figure(nested).unwrap().effective_visible);
        assert!(!first.geometry().figure(hidden).unwrap().effective_visible);
        for micros in [25_000, 50_000, 75_000] {
            fixture
                .runtime
                .animation()
                .advance_time(MonotonicTime::from_micros(micros))
                .unwrap();
            let before = fixture
                .runtime
                .animation()
                .capture_geometry([viewport])
                .unwrap();
            let position = center(before.geometry(), nested, viewport);
            let version = fixture.runtime.animation().timeline().sample_version().time;
            toggle(&mut fixture.runtime, &fixture.model, 1).unwrap();
            let after = fixture
                .runtime
                .animation()
                .capture_geometry([viewport])
                .unwrap();
            assert_eq!(position, center(after.geometry(), nested, viewport));
            assert!(!after.geometry().figure(hidden).unwrap().effective_visible);
            assert_eq!(
                fixture.runtime.animation().timeline().sample_version().time,
                version
            );
        }
    }

    #[test]
    fn disposing_dependency_cancels_tree_and_releases_active_reservations() {
        let mut fixture = super::super::tree_example::canvas_fixture(false);
        fixture.model.lock().unwrap().use_animation = true;
        toggle(&mut fixture.runtime, &fixture.model, 1).unwrap();
        fixture
            .runtime
            .animation()
            .advance_time(MonotonicTime::from_micros(0))
            .unwrap();
        let (viewport, dependency, owner) = {
            let tree = fixture.model.lock().unwrap();
            (
                tree.viewport.unwrap(),
                tree.branches[6].figure,
                tree.animation.unwrap(),
            )
        };
        let held = fixture
            .runtime
            .animation()
            .capture_geometry([viewport])
            .unwrap();
        {
            let mut tree = fixture.model.lock().unwrap();
            tree.branches[5].children.retain(|index| *index != 6);
            tree.branches[6].active = false;
        }
        fixture.runtime.dispose_subtree(dependency).unwrap();
        fixture.runtime.frames().stabilize_for_query().unwrap();
        let current = fixture
            .runtime
            .animation()
            .capture_geometry([viewport])
            .unwrap();
        assert!(current.geometry().figure(dependency).is_err());
        assert!(held.geometry().figure(dependency).is_ok());
        assert_eq!(
            fixture.runtime.animation().timeline().state(owner).unwrap(),
            AnimationState::Cancelled
        );
        assert_eq!(
            fixture
                .runtime
                .animation()
                .timeline()
                .active_animation_count(),
            0
        );
    }

    #[test]
    fn disabled_tree_matches_static_source_geometry() {
        for mode in [AnimationMode::Disabled, AnimationMode::ReducedMotion] {
            let mut animated = super::super::tree_example::canvas_fixture(false);
            let mut static_tree = super::super::tree_example::canvas_fixture(false);
            static_tree.runtime.frames().stabilize_for_query().unwrap();
            animated.model.lock().unwrap().use_animation = true;
            animated.runtime.animation().timeline().set_mode(mode);
            toggle(&mut animated.runtime, &animated.model, 1).unwrap();
            toggle(&mut static_tree.runtime, &static_tree.model, 1).unwrap();
            for index in 0..19 {
                let a = animated.model.lock().unwrap().branches[index].title;
                let b = static_tree.model.lock().unwrap().branches[index].title;
                assert_eq!(
                    animated.runtime.tree().figure_bounds(a).unwrap(),
                    static_tree.runtime.tree().figure_bounds(b).unwrap()
                );
                assert_eq!(
                    animated.runtime.tree().is_effectively_visible(a).unwrap(),
                    static_tree
                        .runtime
                        .tree()
                        .is_effectively_visible(b)
                        .unwrap()
                );
            }
            assert_eq!(
                animated
                    .runtime
                    .animation()
                    .timeline()
                    .active_animation_count(),
                0
            );
        }
    }
}
