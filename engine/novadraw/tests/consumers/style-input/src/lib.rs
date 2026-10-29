use novadraw::{
    figure::{FigureStyle, RectangleFigure},
    graphics::Color,
    runtime::Runtime,
    text::FontDescriptor,
};

/// A mounted style update followed by inherited font and opacity queries.
#[unsafe(no_mangle)]
pub extern "C" fn style_workload(size: f32, alpha: f64, updates: usize) -> f64 {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0).expect("valid figure bounds")))
        .unwrap();
    let child = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 80.0).expect("valid figure bounds")))
        .unwrap();
    let mut result = 0.0;
    for index in 0..updates {
        let font = format!("{}px Inter Variable", size + (index % 2) as f32);
        runtime
            .figure(root)
            .unwrap()
            .set_style(FigureStyle {
                foreground: Some(if index % 2 == 0 {
                    Color::BLACK
                } else {
                    Color::RED
                }),
                alpha: Some(novadraw::graphics::Opacity::try_new(alpha).unwrap()),
                font: Some(FontDescriptor::parse(&font).unwrap()),
                ..Default::default()
            })
            .unwrap();
        let style = runtime.tree().resolved_style(child).unwrap();
        let font = style.font;
        result += f64::from(font.size()) + style.alpha.get();
    }
    result
}

/// Construction, nested metrics, frozen recording and resized border painting.
#[unsafe(no_mangle)]
pub extern "C" fn border_workload(mode: u32, updates: usize) -> f64 {
    use novadraw::{
        figure::border::{Border, CompoundBorder, LineBorder, MarginBorder},
        geometry::{Insets, Rectangle},
        render::CommandRecorder,
    };
    let mut result = 0.0;
    let make = || {
        CompoundBorder::new(
            LineBorder::new(Color::BLACK, 2.0)
                .unwrap()
                .with_insets(Insets::new(3.0, 4.0, 5.0, 6.0))
                .unwrap(),
            CompoundBorder::new(
                MarginBorder::new(Insets::new(1.0, 2.0, 3.0, 4.0)).unwrap(),
                LineBorder::new(Color::RED, 1.0).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    if mode == 0 {
        for _ in 0..updates {
            let border = std::hint::black_box(make());
            result += border.get_insets().width() + border.preferred_size().height;
        }
        return result;
    }
    let border = make();
    let drawing = border.snapshot_bounds_drawing(None).unwrap();
    for index in 0..updates {
        if mode == 1 {
            result += border.get_insets().width() + border.preferred_size().height;
        } else {
            let width = if mode == 3 {
                100.0 + index as f64
            } else {
                100.0
            };
            let mut recorder = CommandRecorder::new();
            border
                .paint_with_insets(
                    Rectangle::new(0.0, 0.0, width, 80.0),
                    Insets::ZERO,
                    &mut recorder.paint(),
                )
                .unwrap();
            std::hint::black_box(recorder.finish().unwrap());
            result += width;
        }
    }
    std::hint::black_box(drawing);
    result
}

#[cfg(test)]
mod contracts {
    include!("../../../../crates/novadraw/tests/p2_s08_style_input_contract.rs");
}

#[cfg(test)]
mod border_contracts {
    include!("../../../../crates/novadraw/tests/p2_s08_border_input_contract.rs");
}

#[cfg(test)]
mod admission_contracts {
    include!("../../../../crates/novadraw/tests/p2_s08_admission_contract.rs");
}

#[cfg(test)]
mod detached_geometry_contracts {
    include!("../../../../crates/novadraw/tests/p2_s08_detached_geometry_contract.rs");
}

/// Actual Runtime frame preparation and acknowledgement, with shared nested borders.
#[unsafe(no_mangle)]
pub extern "C" fn frame_workload(mode: u32, updates: usize) -> u64 {
    use novadraw::{
        figure::border::{CompoundBorder, LineBorder, MarginBorder},
        geometry::{Insets, Rectangle},
        render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
        runtime::{FramePreparation, ReplaceFigureBorder},
    };
    use std::sync::Arc;
    const NODES: usize = 64;
    let border = Arc::new(
        CompoundBorder::new(
            LineBorder::new(Color::BLACK, 2.0).unwrap(),
            CompoundBorder::new(
                MarginBorder::new(Insets::uniform(1.0)).unwrap(),
                LineBorder::new(Color::RED, 1.0).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0).expect("valid figure bounds")))
        .unwrap();
    let mut nodes = Vec::with_capacity(NODES);
    for index in 0..NODES {
        let id = runtime
            .figure(root)
            .unwrap()
            .add(Box::new(RectangleFigure::new(
                (index % 8) as f64 * 90.0,
                (index / 8) as f64 * 65.0,
                80.0,
                55.0,
            ).expect("valid figure bounds")))
            .unwrap();
        runtime
            .figure(id)
            .unwrap()
            .update_capability(ReplaceFigureBorder::new(Some(border.clone())))
            .unwrap();
        nodes.push(id);
    }
    let surface = SurfaceInfo {
        logical_width: 800.0,
        logical_height: 600.0,
        pixel_width: 800,
        pixel_height: 600,
        scale_factor: 1.0,
    };
    let mut checksum = 0;
    for index in 0..=updates {
        if mode == 1 {
            for id in &nodes {
                let bounds = runtime.tree().figure_bounds(*id).unwrap();
                runtime
                    .figure(*id)
                    .unwrap()
                    .set_bounds(Rectangle::new(
                        bounds.x,
                        bounds.y,
                        80.0 + (index % 2) as f64,
                        55.0,
                    ))
                    .unwrap();
            }
        } else if mode == 2 {
            runtime
                .figure(root)
                .unwrap()
                .set_style(FigureStyle {
                    foreground: Some(if index % 2 == 0 {
                        Color::BLACK
                    } else {
                        Color::RED
                    }),
                    ..Default::default()
                })
                .unwrap();
        }
        match runtime
            .frames()
            .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
        {
            FramePreparation::Ready(frame) => {
                checksum += 1;
                assert_eq!(
                    runtime
                        .frames()
                        .complete_submission(
                            frame.session_id,
                            frame.frame_id,
                            Ok(RenderDisposition::Presented)
                        )
                        .unwrap(),
                    novadraw::runtime::SubmissionAcknowledgement::Presented
                );
            }
            FramePreparation::Idle => {}
            other => panic!("frame probe failed: {other:?}"),
        }
    }
    checksum
}
