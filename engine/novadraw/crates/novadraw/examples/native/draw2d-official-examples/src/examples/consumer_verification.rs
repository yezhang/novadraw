//! Actual Vello retained-target checks. Surface presentation is a separate acceptance step.
use novadraw::{
    figure::flow::FlowSnapshot,
    geometry::Point,
    render::{DamageMode, RenderBackend, RenderDisposition, SurfaceInfo},
    runtime::Runtime,
};
use novadraw_backend_vello::VelloRenderer;
use std::{path::Path, sync::Arc};

fn pixels(
    runtime: &mut Runtime,
    renderer: &mut VelloRenderer,
    surface: SurfaceInfo,
    path: &Path,
    full: bool,
) -> (DamageMode, image::RgbaImage) {
    if full {
        runtime.frames().request_full_redraw();
    }
    let submission = runtime
        .frames()
        .prepare_submission(surface, renderer.capabilities())
        .into_ready()
        .unwrap();
    let mode = submission.damage.mode();
    let disposition = renderer.render_for_screenshot(&submission).unwrap();
    assert_eq!(disposition, RenderDisposition::Presented);
    let _ = runtime
        .frames()
        .complete_submission(submission.session_id, submission.frame_id, Ok(disposition))
        .unwrap();
    renderer.screenshot(path).unwrap();
    (mode, image::open(path).unwrap().to_rgba8())
}

pub(crate) fn verify_native(window: Arc<winit::window::Window>) {
    let output = std::env::var_os("NOVADRAW_CONSUMER_EVIDENCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("novadraw-consumer-evidence"));
    std::fs::create_dir_all(&output).unwrap();
    let mut evidence = String::from("scene,dpi,phase,damage\n");
    for dpi in [1_u32, 2] {
        let surface = SurfaceInfo {
            logical_width: 832.0,
            logical_height: 640.0,
            pixel_width: 832 * dpi,
            pixel_height: 640 * dpi,
            scale_factor: f64::from(dpi),
        };
        for name in [
            "FlowLayoutExample",
            "GridLayoutExample",
            "ToolbarLayoutExample",
            "CaretExample",
        ] {
            let mut runtime = if name == "CaretExample" {
                super::caret_example()
            } else {
                super::panel_scene(name).unwrap().0
            };
            let mut renderer = VelloRenderer::new(window.clone(), surface).unwrap();
            let prefix = format!("{name}-{dpi}");
            let path = output.join(format!("{prefix}-actual.png"));
            pixels(&mut runtime, &mut renderer, surface, &path, true);
            let mut partial_count = 0;
            if name == "CaretExample" {
                let mut pending = vec![runtime.tree().contents().unwrap()];
                let page = loop {
                    let id = pending.pop().unwrap();
                    if runtime.tree().node(id).unwrap().figure_name()
                        == "OfficialInteractiveFlowPage"
                    {
                        break id;
                    }
                    pending.extend(runtime.tree().child_order(id).unwrap());
                };
                for stage in 0..3 {
                    let snapshot = runtime
                        .tree()
                        .prepared_presentation(page)
                        .unwrap()
                        .unwrap()
                        .shared_data::<FlowSnapshot>()
                        .unwrap();
                    let fragments: Vec<_> = snapshot
                        .fragments
                        .iter()
                        .filter(|f| f.bounds.width > 20.0)
                        .collect();
                    let fragment = fragments[stage * 2];
                    let point = Point::new(
                        fragment.bounds.x + fragment.bounds.width * 0.25,
                        fragment.bounds.y + fragment.bounds.height * 0.5,
                    );
                    let point = runtime
                        .tree()
                        .local_to_surface_transform(page)
                        .unwrap()
                        .transform_point(point);
                    runtime.input().dispatch_mouse_moved(point.x(), point.y());
                    let (mode, actual) = pixels(&mut runtime, &mut renderer, surface, &path, false);
                    partial_count += usize::from(mode == DamageMode::Partial);
                    let (_, expected) = pixels(
                        &mut runtime,
                        &mut renderer,
                        surface,
                        &output.join(format!("{prefix}-full.png")),
                        true,
                    );
                    assert_eq!(actual, expected, "{prefix}: caret hover {stage}");
                    evidence.push_str(&format!("{name},{dpi},hover-{stage},{mode:?}\n"));
                }
            } else {
                for width in [820.0, 760.0, 620.0, 440.0, 300.0, 620.0, 820.0] {
                    runtime
                        .frames()
                        .resize_logical_viewport(width, 520.0)
                        .unwrap();
                    let (mode, actual) = pixels(&mut runtime, &mut renderer, surface, &path, false);
                    partial_count += usize::from(mode == DamageMode::Partial);
                    let (_, expected) = pixels(
                        &mut runtime,
                        &mut renderer,
                        surface,
                        &output.join(format!("{prefix}-full.png")),
                        true,
                    );
                    assert_eq!(actual, expected, "{prefix}: viewport width={width}");
                    evidence.push_str(&format!("{name},{dpi},width-{width},{mode:?}\n"));
                }
                let root = runtime.tree().contents().unwrap();
                let content = runtime.tree().child_order(root).unwrap()[0];
                let child = runtime.tree().child_order(content).unwrap()[0];
                for visible in [false, true] {
                    runtime.figure(child).unwrap().set_visible(visible).unwrap();
                    let (mode, actual) = pixels(&mut runtime, &mut renderer, surface, &path, false);
                    partial_count += usize::from(mode == DamageMode::Partial);
                    let (_, expected) = pixels(
                        &mut runtime,
                        &mut renderer,
                        surface,
                        &output.join(format!("{prefix}-full.png")),
                        true,
                    );
                    assert_eq!(actual, expected, "{prefix}: visibility={visible}");
                    evidence.push_str(&format!("{name},{dpi},visible-{visible},{mode:?}\n"));
                }
            }
            assert!(
                partial_count > 0,
                "{prefix}: no actual retained-partial submission"
            );
            assert!(runtime.take_deferred_mutation_errors().is_empty());
        }
    }
    std::fs::write(output.join("frames.csv"), evidence).unwrap();
    println!(
        "Consumer retained GPU checks passed; evidence: {}",
        output.display()
    );
}
