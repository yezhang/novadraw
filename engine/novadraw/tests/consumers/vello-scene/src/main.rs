//! Standalone consumer and narrowly scoped Scene encoding cost probe.

use novadraw::{
    figure::RectangleFigure,
    graphics::Color,
    render::{BackendCapabilities, SurfaceInfo},
    runtime::{FramePreparation, Runtime},
};

fn main() {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            640.0,
            480.0,
            Color::WHITE,
        )))
        .unwrap();
    for index in 0..100 {
        runtime
            .figure(root)
            .unwrap()
            .add(Box::new(RectangleFigure::new_with_color(
                (index % 10) as f64 * 60.0,
                (index / 10) as f64 * 40.0,
                40.0,
                20.0,
                Color::RED,
            )))
            .unwrap();
    }
    let FramePreparation::Ready(submission) = runtime.frames().prepare_submission(
        SurfaceInfo {
            logical_width: 640.0,
            logical_height: 480.0,
            pixel_width: 1280,
            pixel_height: 960,
            scale_factor: 2.0,
        },
        BackendCapabilities::FULL_FRAME_ONLY
            .with_glyph_runs()
            .with_image_resources()
            .with_custom_strokes()
            .with_path_clips()
            .with_linear_gradients()
            .with_host_background()
            .with_per_operation_antialias(),
    ) else {
        panic!("expected prepared scene")
    };
    std::hint::black_box(&submission);
    #[cfg(feature = "scene")]
    {
        use novadraw::render::{RenderBackend, RenderDisposition};
        use novadraw_backend_vello::scene::{Scene, VelloSceneRenderer};
        use std::time::Instant;

        let mut renderer = VelloSceneRenderer::new();
        let mut target = Scene::new();
        let cold = Instant::now();
        assert_eq!(
            renderer.bind(&mut target).submit(&submission),
            Ok(RenderDisposition::Presented)
        );
        let cold_us = cold.elapsed().as_secs_f64() * 1_000_000.0;
        let mut samples = Vec::with_capacity(1000);
        for _ in 0..1000 {
            let started = Instant::now();
            assert_eq!(
                renderer.bind(&mut target).submit(&submission),
                Ok(RenderDisposition::Presented)
            );
            std::hint::black_box(&target);
            samples.push(started.elapsed().as_secs_f64() * 1_000_000.0);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "commands={}, cold_encode_us={cold_us:.3}, warm_p50_us={:.3}, warm_p95_us={:.3}, warm_p99_us={:.3}, paths={}",
            submission.commands.len(),
            samples[500],
            samples[950],
            samples[990],
            target.encoding().n_paths,
        );
    }
    #[cfg(not(feature = "scene"))]
    println!("commands={}, runtime_only=true", submission.commands.len());
}
