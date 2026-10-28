//! Original GPU demo factory, inspected here without creating a GPU.
use novadraw::{
    figure::flow::FlowSnapshot,
    geometry::Rectangle,
    render::{BackendCapabilities, RenderCommandKind, RenderSubmission, SurfaceInfo},
    runtime::Runtime,
};
use std::{collections::HashMap, sync::Arc};

fn snapshot(runtime: &Runtime) -> Arc<FlowSnapshot> {
    let mut pending = vec![runtime.tree().contents().unwrap()];
    while let Some(id) = pending.pop() {
        if let Some(p) = runtime.tree().prepared_presentation(id).unwrap()
            && let Some(snapshot) = p.shared_data::<FlowSnapshot>()
        {
            return snapshot;
        }
        pending.extend(runtime.tree().child_order(id).unwrap());
    }
    panic!("missing FlowSnapshot");
}

fn backing(frame: &RenderSubmission) -> (usize, usize, usize) {
    let mut glyphs = HashMap::new();
    let mut coords = HashMap::new();
    let mut visible = 0;
    for command in &frame.commands {
        if let RenderCommandKind::DrawGlyphRun { run, .. } = &command.kind {
            let backing = run.glyphs.buffer().as_slice();
            glyphs.insert(backing.as_ptr(), std::mem::size_of_val(backing));
            coords.insert(
                run.normalized_coords.as_ptr(),
                std::mem::size_of_val(run.normalized_coords.as_ref()),
            );
            visible += run.glyphs.len();
        }
    }
    (
        visible,
        glyphs.values().sum::<usize>() + coords.values().sum::<usize>(),
        (glyphs.len() + coords.len()) * 2 * std::mem::size_of::<usize>(),
    )
}

#[test]
fn original_large_frame_clones_retain_identical_backing_and_old_resize_leases() {
    let (_, mut factory) = draw2d_official_examples::runnable_entries()
        .into_iter()
        .find(|e| e.0 == "TextFlowLargeExample")
        .unwrap();
    let mut runtime = factory();
    runtime.frames().stabilize_for_query().unwrap();
    let old = snapshot(&runtime);
    let surface = SurfaceInfo {
        logical_width: 600.0,
        logical_height: 500.0,
        pixel_width: 600,
        pixel_height: 500,
        scale_factor: 1.0,
    };
    let frame = runtime
        .frames()
        .prepare_submission(
            surface,
            BackendCapabilities::RETAINED_PARTIAL
                .with_path_clips()
                .with_custom_strokes(),
        )
        .into_ready()
        .unwrap();
    let copy = frame.clone();
    for (a, b) in frame.commands.iter().zip(&copy.commands) {
        if let (
            RenderCommandKind::DrawGlyphRun { run: a, .. },
            RenderCommandKind::DrawGlyphRun { run: b, .. },
        ) = (&a.kind, &b.kind)
        {
            assert_eq!(
                a.glyphs.buffer().as_slice().as_ptr(),
                b.glyphs.buffer().as_slice().as_ptr()
            );
            assert!(Arc::ptr_eq(&a.normalized_coords, &b.normalized_coords));
            assert!(
                old.paragraphs
                    .iter()
                    .flat_map(|p| p.layout.layout().glyph_runs())
                    .any(|source| {
                        source.glyphs.buffer().as_slice().as_ptr()
                            == a.glyphs.buffer().as_slice().as_ptr()
                    })
            );
        }
    }
    let observation = backing(&frame);
    assert_eq!(observation.0, 51232);
    assert_eq!(backing(&copy), observation);
    assert_eq!(
        runtime
            .frames()
            .complete_submission(
                frame.session_id,
                frame.frame_id,
                Ok(novadraw::render::RenderDisposition::Presented)
            )
            .unwrap(),
        novadraw::runtime::SubmissionAcknowledgement::Presented,
    );
    let root = runtime.tree().contents().unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 400.0, 500.0))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    drop(runtime);
    drop(old);
    drop(frame);
    assert_eq!(backing(&copy), observation);
    println!(
        "preappend original Large: glyphs={}, unique backing payload={} bytes, Arc counters={} bytes; layout/recorder/submission/clone add zero glyph-array copies",
        observation.0, observation.1, observation.2
    );
}
