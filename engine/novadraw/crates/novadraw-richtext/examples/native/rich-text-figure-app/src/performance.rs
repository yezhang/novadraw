//! Reproducible CPU replay. Publication is acknowledged without a GPU backend;
//! timings cover mutation, stabilization and command recording, not GPU completion.
use novadraw::{
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
    runtime::{FramePreparation, Runtime, SubmissionAcknowledgement},
    text::TextLayoutDiagnostics,
};
use serde_json::json;
use std::{error::Error, path::PathBuf, time::Instant};

const STEPS: usize = 16;
const ROUNDS: usize = 5;
const WIDTH_CHANGE: f64 = 8.0;
const HEIGHT_CHANGE: f64 = 4.0;
const DRAG_STEP: f64 = 4.0;

fn record(runtime: &mut Runtime, width: f64, height: f64) -> Result<usize, Box<dyn Error>> {
    let _scope = TextLayoutDiagnostics::default().time("richtext.record");
    let surface = SurfaceInfo {
        logical_width: width,
        logical_height: height,
        pixel_width: width as u32,
        pixel_height: height as u32,
        scale_factor: 1.0,
    };
    let capabilities = BackendCapabilities {
        glyph_runs: true,
        custom_strokes: true,
        path_clips: true,
        per_operation_antialias: true,
        ..BackendCapabilities::FULL_FRAME_ONLY
    };
    let frame = match runtime.frames().prepare_submission(surface, capabilities) {
        FramePreparation::Ready(frame) => frame,
        FramePreparation::Idle => return Ok(0),
        other => return Err(format!("CPU replay frame: {other:?}").into()),
    };
    let commands = frame.commands.len();
    if runtime.frames().complete_submission(
        frame.session_id,
        frame.frame_id,
        Ok(RenderDisposition::Presented),
    )? != SubmissionAcknowledgement::Presented
    {
        return Err("CPU recording acknowledgement failed".into());
    }
    Ok(commands)
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let output = args
        .windows(2)
        .find(|pair| pair[0] == "--output")
        .map(|pair| PathBuf::from(&pair[1]))
        .unwrap_or_else(|| PathBuf::from("/tmp/novadraw-richtext-resize"));
    std::fs::create_dir_all(&output)?;
    #[cfg(feature = "profiling")]
    let capture = args
        .iter()
        .any(|arg| arg == "--trace")
        .then(capture::start)
        .transpose()?;
    #[cfg(not(feature = "profiling"))]
    if args.iter().any(|arg| arg == "--trace") {
        return Err("--trace requires --features profiling".into());
    }
    let fixture = super::Fixture::from_args(args)?;
    let mut metadata = fixture.metadata(&fixture.document());
    let open = Instant::now();
    let (mut runtime, handle, diagram, selections) = super::scene(fixture);
    runtime
        .frames()
        .resize_logical_viewport(super::WINDOW_WIDTH, super::WINDOW_HEIGHT)?;
    runtime.frames().stabilize_for_query()?;
    metadata["cold_scene_and_layout_ns"] = json!(open.elapsed().as_nanos());
    super::click(&mut runtime, *selections.last().unwrap())?;
    let point = super::input_point(&runtime, *selections.last().unwrap())?;
    runtime.input().dispatch_mouse_double_clicked(
        point.x(),
        point.y(),
        novadraw::event::MouseButton::Left,
    );
    runtime.frames().stabilize_for_query()?;
    record(&mut runtime, super::WINDOW_WIDTH, super::WINDOW_HEIGHT)?;
    let mut samples = Vec::new();
    for round in 0..ROUNDS {
        for kind in ["width", "height", "drag"] {
            for step in 0..STEPS {
                let scope = TextLayoutDiagnostics::default().time(match kind {
                    "width" => "replay.width",
                    "height" => "replay.height",
                    _ => "replay.drag",
                });
                let start = Instant::now();
                let width = if kind == "width" {
                    super::WINDOW_WIDTH - WIDTH_CHANGE * (step + 1) as f64
                } else {
                    super::WINDOW_WIDTH
                };
                let height = if kind == "height" {
                    super::WINDOW_HEIGHT - HEIGHT_CHANGE * (step + 1) as f64
                } else {
                    super::WINDOW_HEIGHT
                };
                if kind == "drag" {
                    let before = handle.scroll_pane().viewport().view_location().y();
                    let bar = handle.scroll_pane().vertical_scroll_bar();
                    let thumb = runtime
                        .tree()
                        .child_order(bar)?
                        .into_iter()
                        .find(|id| {
                            runtime
                                .tree()
                                .layout_constraint::<novadraw::container::ScrollBarPart>(*id)
                                .is_ok_and(|role| {
                                    role == Some(&novadraw::container::ScrollBarPart::Thumb)
                                })
                        })
                        .ok_or("missing thumb")?;
                    let p = super::input_point(&runtime, thumb)?;
                    runtime.input().dispatch_mouse_pressed(
                        p.x(),
                        p.y(),
                        novadraw::event::MouseButton::Left,
                    );
                    runtime
                        .input()
                        .dispatch_mouse_moved(p.x(), p.y() + DRAG_STEP);
                    runtime.input().dispatch_mouse_released(
                        p.x(),
                        p.y() + DRAG_STEP,
                        novadraw::event::MouseButton::Left,
                    );
                    if handle.scroll_pane().viewport().view_location().y() <= before {
                        return Err("drag replay did not advance document".into());
                    }
                } else {
                    runtime.frames().resize_logical_viewport(width, height)?;
                }
                runtime.frames().stabilize_for_query()?;
                let stabilize_ns = start.elapsed().as_nanos();
                let commands = record(&mut runtime, width, height)?;
                let total_ns = start.elapsed().as_nanos();
                drop(scope);
                samples.push(json!({"kind":kind,"round":round,"step":step,
                    "stabilize_ns":stabilize_ns,"cpu_frame_ns":total_ns,"commands":commands}));
            }
            runtime
                .frames()
                .resize_logical_viewport(super::WINDOW_WIDTH, super::WINDOW_HEIGHT)?;
            runtime.viewports().set_view_location(
                handle.scroll_pane().viewport().figure_id(),
                0.0,
                0.0,
            )?;
            runtime.frames().stabilize_for_query()?;
            record(&mut runtime, super::WINDOW_WIDTH, super::WINDOW_HEIGHT)?;
        }
    }
    let bounds = runtime.tree().figure_bounds(diagram)?;
    if bounds.width <= 0.0 {
        return Err("diagram disappeared".into());
    }
    std::fs::write(
        output.join("samples.json"),
        serde_json::to_vec_pretty(&samples)?,
    )?;
    std::fs::write(
        output.join("metadata.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;
    #[cfg(feature = "profiling")]
    if let Some(capture) = capture {
        capture::finish(capture, output.join("resize.pftrace"))?;
    }
    println!(
        "CPU resize/height/drag replay: {} samples in {}",
        samples.len(),
        output.display()
    );
    Ok(())
}

#[cfg(feature = "profiling")]
mod capture {
    use super::*;
    use perfetto_sdk::{
        heap_buffer::HeapBuffer,
        pb_msg::{PbMsg, PbMsgWriter},
        producer::{Backends, Producer, ProducerInitArgsBuilder},
        protos::config::{
            data_source_config::DataSourceConfig,
            trace_config::{TraceConfig, TraceConfigBufferConfig, TraceConfigDataSource},
            track_event::track_event_config::TrackEventConfig,
        },
        tracing_session::TracingSession,
        track_event::TrackEvent,
    };
    use std::sync::{Arc, Mutex};
    const BUFFER_KB: u32 = 64 * 1024;
    pub fn start() -> Result<TracingSession, Box<dyn Error>> {
        Producer::init(
            ProducerInitArgsBuilder::new()
                .backends(Backends::IN_PROCESS)
                .build(),
        );
        TrackEvent::init();
        novadraw::text::register_perfetto_categories()?;
        let writer = PbMsgWriter::new();
        let buffer = HeapBuffer::new(writer.stream_writer());
        let mut msg = PbMsg::new(&writer)?;
        let mut config = TraceConfig { msg: &mut msg };
        config.set_buffers(|b: &mut TraceConfigBufferConfig| {
            b.set_size_kb(BUFFER_KB);
        });
        config.set_data_sources(|d: &mut TraceConfigDataSource| {
            d.set_config(|c: &mut DataSourceConfig| {
                c.set_name("track_event");
                c.set_track_event_config(|t: &mut TrackEventConfig| {
                    t.set_disabled_categories("*");
                    t.set_enabled_categories("novadraw.phase");
                });
            });
        });
        msg.finalize();
        let mut bytes = vec![0; writer.stream_writer().get_written_size()];
        buffer.copy_into(&mut bytes);
        let mut session = TracingSession::in_process()?;
        session.setup(&bytes);
        session.start_blocking();
        Ok(session)
    }
    pub fn finish(mut session: TracingSession, path: PathBuf) -> Result<(), Box<dyn Error>> {
        session.stop_blocking();
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = bytes.clone();
        session.read_trace_blocking(move |data, _| output.lock().unwrap().extend_from_slice(data));
        std::fs::write(path, &*bytes.lock().unwrap())?;
        Ok(())
    }
}
