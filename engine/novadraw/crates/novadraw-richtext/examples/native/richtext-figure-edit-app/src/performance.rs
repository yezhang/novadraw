//! CPU publication replay; the synthetic acknowledgement never submits to a GPU.
use crate::{
    Config, HEIGHT, Result, WIDTH,
    editor::{Editor, PAYLOAD},
};
use novadraw::{
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
    runtime::{FramePreparation, Runtime, SubmissionAcknowledgement},
    text::TextLayoutDiagnostics,
};
use serde_json::json;
use std::time::Instant;
const ROUNDS: usize = 3;
const STEPS: usize = 8;

pub fn run_session(config: &Config) -> Result<()> {
    use crate::embedded::Embedded;
    use novadraw_editor::{ExtendTextSelection, TextInputEvent};
    std::fs::create_dir_all(&config.output)?;
    #[cfg(feature = "profiling")]
    let capture = config.trace.then(capture::start).transpose()?;
    #[cfg(not(feature = "profiling"))]
    if config.trace {
        return Err("--trace requires --features profiling".into());
    }
    let start = Instant::now();
    let mut editor = Embedded::new(config)?;
    let cold_ns = start.elapsed().as_nanos();
    let activation = Instant::now();
    editor.start()?;
    let activation_ns = activation.elapsed().as_nanos();
    let mut samples = Vec::new();
    let record = |editor: &mut Embedded| -> Result<usize> {
        let surface = SurfaceInfo {
            logical_width: WIDTH,
            logical_height: HEIGHT,
            pixel_width: WIDTH as u32,
            pixel_height: HEIGHT as u32,
            scale_factor: 1.0,
        };
        let capabilities = BackendCapabilities {
            glyph_runs: true,
            custom_strokes: true,
            path_clips: true,
            per_operation_antialias: true,
            ..BackendCapabilities::FULL_FRAME_ONLY
        };
        let frame = match editor
            .viewer
            .drive()
            .prepare_submission(surface, capabilities)
        {
            FramePreparation::Ready(frame) => frame,
            FramePreparation::Idle => return Ok(0),
            other => return Err(format!("session CPU frame: {other:?}").into()),
        };
        let commands = frame.commands.len();
        if editor.viewer.drive().complete_submission(
            frame.session_id,
            frame.frame_id,
            Ok(RenderDisposition::Presented),
        )? != SubmissionAcknowledgement::Presented
        {
            return Err("session CPU acknowledgement failed".into());
        }
        Ok(commands)
    };
    record(&mut editor)?;
    for round in 0..ROUNDS {
        for region in 0..3 {
            for step in 0..STEPS {
                editor.locate(region)?;
                record(&mut editor)?;
                let paragraph = editor.controller()?.selection().focus.paragraph;
                let original = editor
                    .controller()?
                    .document()
                    .paragraphs()
                    .iter()
                    .find(|p| p.id() == paragraph)
                    .unwrap()
                    .text();
                for kind in ["insert", "replace", "delete"] {
                    if kind != "insert" {
                        for _ in 0..3 {
                            editor.input(TextInputEvent::Move {
                                movement: novadraw::text::TextMovement::PreviousVisual,
                                extend: ExtendTextSelection::Yes,
                            })?;
                        }
                    }
                    let start = Instant::now();
                    editor.input(TextInputEvent::InsertText(
                        match kind {
                            "insert" => PAYLOAD,
                            "replace" => "测试🚀",
                            _ => "",
                        }
                        .into(),
                    ))?;
                    let input_feedback_ns = start.elapsed().as_nanos();
                    let record_start = Instant::now();
                    let commands = record(&mut editor)?;
                    samples.push(json!({"host":"editor","kind":kind,"region":(["head","middle","tail"][region]),"round":round,"step":step,
                        "input_feedback_ns":input_feedback_ns,"record_ns":record_start.elapsed().as_nanos(),
                        "cpu_frame_ns":start.elapsed().as_nanos(),"commands":commands}));
                }
                let restored = editor
                    .controller()?
                    .document()
                    .paragraphs()
                    .iter()
                    .find(|p| p.id() == paragraph)
                    .unwrap()
                    .text();
                if restored != original {
                    return Err("session replay failed to restore paragraph content".into());
                }
            }
        }
    }
    #[cfg(feature = "profiling")]
    if let Some(session) = capture {
        capture::finish(session, config.output.join("session-edit.pftrace"))?;
    }
    let mut summary = Vec::new();
    for region in ["head", "middle", "tail"] {
        for kind in ["insert", "replace", "delete"] {
            let subset: Vec<_> = samples
                .iter()
                .filter(|s| s["region"] == region && s["kind"] == kind)
                .collect();
            let mut row =
                json!({"host":"editor","kind":kind,"region":region,"samples":subset.len()});
            for metric in ["input_feedback_ns", "record_ns", "cpu_frame_ns"] {
                let mut values: Vec<_> =
                    subset.iter().map(|s| s[metric].as_u64().unwrap()).collect();
                values.sort_unstable();
                row[metric] = json!({"p50_ms":values[values.len()/2] as f64/1e6,
                    "p95_ms":values[(values.len()*95).div_ceil(100).saturating_sub(1)] as f64/1e6});
            }
            summary.push(row);
        }
    }
    let accept = Instant::now();
    editor.input(TextInputEvent::Accept)?;
    let accept_ns = accept.elapsed().as_nanos();
    let metadata = json!({"host":"editor","load":config.load,"characters":editor.metadata["characters"],
        "cold_ns":cold_ns,"activation_ns":activation_ns,"accept_ns":accept_ns,
        "gpu_submission":false,"selection_setup_excluded":true,
        "note":"input includes candidate clone, document edit, feedback rebuild and layout; source and draft are recorded in the same scene"});
    for (name, value) in [
        ("session-samples.json", json!(samples)),
        ("session-summary.json", json!(summary)),
        ("session-metadata.json", metadata),
    ] {
        std::fs::write(config.output.join(name), serde_json::to_vec_pretty(&value)?)?;
    }
    println!(
        "{} Editor CPU samples written to {}",
        samples.len(),
        config.output.display()
    );
    Ok(())
}

fn record(runtime: &mut Runtime) -> Result<usize> {
    let _record = TextLayoutDiagnostics::default().time("richtext.edit.record");
    let surface = SurfaceInfo {
        logical_width: WIDTH,
        logical_height: HEIGHT,
        pixel_width: WIDTH as u32,
        pixel_height: HEIGHT as u32,
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
        other => return Err(format!("CPU frame: {other:?}").into()),
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
pub fn run(config: &Config) -> Result<()> {
    std::fs::create_dir_all(&config.output)?;
    #[cfg(feature = "profiling")]
    let capture = config.trace.then(capture::start).transpose()?;
    #[cfg(not(feature = "profiling"))]
    if config.trace {
        return Err("--trace requires --features profiling".into());
    }
    let start = Instant::now();
    let mut editor = Editor::new(config)?;
    let mut metadata = editor.metadata.clone();
    metadata["cold_scene_and_layout_ns"] = json!(start.elapsed().as_nanos());
    metadata["rounds"] = json!(ROUNDS);
    metadata["steps_per_region"] = json!(STEPS);
    metadata["timing_note"] =
        json!("mutation + layout + caret publication + CPU command recording; no GPU");
    record(&mut editor.runtime)?;
    let mut samples = Vec::new();
    for round in 0..ROUNDS {
        for region in 0..3 {
            for step in 0..STEPS {
                // Location and scrolling are setup, excluded from edit latency.
                editor.locate(region)?;
                record(&mut editor.runtime)?;
                let byte = editor.cursor().byte;
                let original = editor.current_text()?;
                for kind in ["insert", "replace", "delete"] {
                    let start = Instant::now();
                    let timing = match kind {
                        "insert" => editor.insert(PAYLOAD)?,
                        "replace" => editor.replace(byte..byte + PAYLOAD.len(), "测试🚀")?,
                        _ => editor.replace(byte..byte + "测试🚀".len(), "")?,
                    };
                    let record_start = Instant::now();
                    let commands = record(&mut editor.runtime)?;
                    samples.push(json!({
                        "kind":kind, "region":(["head","middle","tail"][region]),
                        "round":round,"step":step,"mutation_ns":timing.mutation_ns,
                        "layout_ns":timing.layout_ns,"record_ns":record_start.elapsed().as_nanos(),
                        "cpu_frame_ns":start.elapsed().as_nanos(),"commands":commands,
                    }));
                }
                if editor.current_text()? != original {
                    return Err("edit replay failed to restore paragraph content".into());
                }
            }
        }
    }
    let mut summary = Vec::new();
    for region in ["head", "middle", "tail"] {
        for kind in ["insert", "replace", "delete"] {
            let subset: Vec<_> = samples
                .iter()
                .filter(|s| s["kind"] == kind && s["region"] == region)
                .collect();
            let mut row = json!({"region":region,"kind":kind,"samples":subset.len()});
            for metric in ["mutation_ns", "layout_ns", "record_ns", "cpu_frame_ns"] {
                let mut values: Vec<_> =
                    subset.iter().map(|s| s[metric].as_u64().unwrap()).collect();
                values.sort_unstable();
                let p50 = values[values.len() / 2] as f64 / 1e6;
                let p95 = values[(values.len() * 95).div_ceil(100).saturating_sub(1)] as f64 / 1e6;
                row[metric] = json!({"p50_ms":p50,"p95_ms":p95});
            }
            summary.push(row);
        }
    }
    std::fs::write(
        config.output.join("samples.json"),
        serde_json::to_vec_pretty(&samples)?,
    )?;
    std::fs::write(
        config.output.join("metadata.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;
    std::fs::write(
        config.output.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    #[cfg(feature = "profiling")]
    if let Some(session) = capture {
        capture::finish(session, config.output.join("edit.pftrace"))?;
    }
    println!(
        "{} CPU edit samples written to {}. GPU was not submitted.",
        samples.len(),
        config.output.display()
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
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };
    const BUFFER_KB: u32 = 64 * 1024;
    pub fn start() -> Result<TracingSession> {
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
    pub fn finish(mut session: TracingSession, path: PathBuf) -> Result<()> {
        session.stop_blocking();
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = bytes.clone();
        session.read_trace_blocking(move |data, _| output.lock().unwrap().extend_from_slice(data));
        std::fs::write(path, &*bytes.lock().unwrap())?;
        Ok(())
    }
}
