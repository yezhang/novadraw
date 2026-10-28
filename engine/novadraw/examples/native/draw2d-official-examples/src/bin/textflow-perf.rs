//! CPU-only original-fixture driver. Every append starts from the same source text.
use std::time::Instant;

#[path = "textflow-perf/workloads.rs"]
mod workloads;

use novadraw::{
    event::{Key, KeyModifiers},
    figure::flow::FlowSnapshot,
    runtime::Runtime,
};

fn snapshot(runtime: &Runtime) -> &FlowSnapshot {
    let mut pending = vec![runtime.tree().contents().expect("fixture root")];
    while let Some(id) = pending.pop() {
        if let Some(presentation) = runtime.tree().prepared_presentation(id).unwrap()
            && let Some(snapshot) = presentation.data::<FlowSnapshot>()
        {
            return snapshot;
        }
        pending.extend(runtime.tree().child_order(id).unwrap());
    }
    panic!("original fixture did not publish flow data");
}

fn main() {
    let extended = std::env::args().nth(2).is_some_and(|arg| arg == "extended");
    let workload = std::env::args().nth(3).unwrap_or_else(|| "original".into());
    let samples: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "1".into())
        .parse()
        .expect("sample count");
    let (_, mut factory) = draw2d_official_examples::runnable_entries()
        .into_iter()
        .find(|entry| entry.0 == "TextFlowLargeExample")
        .expect("original fixture");
    for sample in 0..samples {
        let start = Instant::now();
        let (mut runtime, source) = if workload == "original" {
            (factory(), None)
        } else {
            let (runtime, source, text) = workloads::make(&workload);
            (runtime, Some((source, text)))
        };
        runtime.frames().set_timing_enabled(true);
        let construct_ns = start.elapsed().as_nanos();
        let start = Instant::now();
        if let Err(error) = runtime.frames().stabilize_for_query() {
            eprintln!("sample {sample}: {error:?}");
            println!("{{\"sample\":{sample},\"status\":\"initial_failure\"}}");
            continue;
        }
        let initial_ns = start.elapsed().as_nanos();
        let before = snapshot(&runtime);
        let paragraphs = before.paragraphs.len();
        let bytes: usize = before
            .paragraphs
            .iter()
            .map(|p| p.layout.request().text().len())
            .sum();
        let lines: usize = before
            .paragraphs
            .iter()
            .map(|p| p.layout.layout().line_metrics().len())
            .sum();
        let runs: usize = before
            .paragraphs
            .iter()
            .map(|p| p.layout.runs().len())
            .sum();
        let fragments = before.fragments.len();
        let width = before.paragraphs[0]
            .layout
            .request()
            .constraints()
            .max_width
            .unwrap();
        let revision = before.revision;
        let initial_work = before.statistics;
        let original_source = (workload == "original").then(|| {
            let range = &before.paragraphs[0].sources[0];
            assert_eq!(
                &before.paragraphs[0].layout.request().text()[range.range.clone()],
                "Normal text."
            );
            (
                range.source,
                runtime.tree().component_revision(range.source).unwrap(),
            )
        });
        for timing in runtime.frames().take_timings() {
            println!(
                "{{\"sample\":{sample},\"status\":\"work\",\"phase\":\"runtime-initial/{}\",\"calls\":{},\"inclusive_ns\":{},\"exclusive_ns\":{}}}",
                timing.stage, timing.calls, timing.inclusive_ns, timing.exclusive_ns
            );
        }
        println!(
            "{{\"sample\":{sample},\"status\":\"work\",\"phase\":\"initial-cache\",\"paragraph_hits\":{},\"source_geometry_calls\":{}}}",
            initial_work.paragraph_hits, initial_work.source_geometry_calls
        );
        let start = Instant::now();
        let receipt = if let Some((source, text)) = &source {
            Some(
                runtime
                    .figure(*source)
                    .unwrap()
                    .update_component(novadraw::figure::flow::SetStructuredFlowText(format!(
                        "{text}Z"
                    )))
                    .unwrap(),
            )
        } else {
            runtime
                .input()
                .dispatch_key_pressed(Key::Character('Z'), KeyModifiers::default());
            None
        };
        let dispatch_ns = start.elapsed().as_nanos();
        let original_dispatch_revision = original_source.map(|(source, previous_revision)| {
            let revision = runtime.tree().component_revision(source).unwrap();
            assert_eq!(
                Some(revision),
                previous_revision.checked_add(1),
                "key dispatch must commit the queued source mutation once"
            );
            revision
        });
        if let Some(receipt) = receipt {
            assert!(receipt.changed);
            assert_eq!(receipt.previous_revision + 1, receipt.revision);
            println!(
                "{{\"sample\":{sample},\"status\":\"work\",\"phase\":\"source-commit\",\"previous_revision\":{},\"revision\":{}}}",
                receipt.previous_revision, receipt.revision
            );
        }
        let start = Instant::now();
        if let Err(error) = runtime.frames().stabilize_for_query() {
            eprintln!("sample {sample}: {error:?}");
            println!("{{\"sample\":{sample},\"status\":\"append_failure\"}}");
            continue;
        }
        let append_update_ns = start.elapsed().as_nanos();
        if let Some((source, previous_revision)) = original_source {
            let revision = runtime.tree().component_revision(source).unwrap();
            let dispatch_revision = original_dispatch_revision.unwrap();
            assert_eq!(previous_revision.checked_add(1), Some(revision));
            assert_eq!(revision, dispatch_revision);
            println!(
                "{{\"sample\":{sample},\"status\":\"work\",\"phase\":\"source-commit\",\"evidence\":\"component-revision\",\"previous_revision\":{previous_revision},\"dispatch_revision\":{dispatch_revision},\"revision\":{revision}}}"
            );
        }
        let after = snapshot(&runtime);
        assert_ne!(revision, after.revision);
        if let Some((_, text)) = &source {
            assert!(
                after.paragraphs[0]
                    .layout
                    .request()
                    .text()
                    .starts_with(&format!("{text}Z"))
            );
        } else {
            assert!(
                after.paragraphs[0]
                    .layout
                    .request()
                    .text()
                    .starts_with("Normal text.Z")
            );
        }
        assert_eq!(
            after
                .paragraphs
                .iter()
                .map(|p| p.layout.request().text().len())
                .sum::<usize>(),
            bytes + 1
        );
        let append_work = after.statistics;
        for timing in runtime.frames().take_timings() {
            println!(
                "{{\"sample\":{sample},\"status\":\"work\",\"phase\":\"runtime-append/{}\",\"calls\":{},\"inclusive_ns\":{},\"exclusive_ns\":{}}}",
                timing.stage, timing.calls, timing.inclusive_ns, timing.exclusive_ns
            );
        }
        println!(
            "{{\"sample\":{sample},\"status\":\"work\",\"phase\":\"append-cache\",\"paragraph_hits\":{},\"source_geometry_calls\":{}}}",
            append_work.paragraph_hits - initial_work.paragraph_hits,
            append_work.source_geometry_calls - initial_work.source_geometry_calls
        );
        assert!(runtime.take_deferred_mutation_errors().is_empty());
        let start = Instant::now();
        runtime
            .frames()
            .stabilize_for_query()
            .expect("unchanged stable query");
        let stable_ns = start.elapsed().as_nanos();
        println!(
            "{{\"sample\":{sample},\"status\":\"work\",\"phase\":\"initial\",\"builds\":{},\"layout_calls\":{},\"contribution_ns\":{},\"styled_layout_ns\":{},\"source_geometry_ns\":{},\"build_ns\":{}}}",
            initial_work.builds,
            initial_work.styled_layout_calls,
            initial_work.contribution_ns,
            initial_work.styled_layout_ns,
            initial_work.source_geometry_ns,
            initial_work.build_ns
        );
        println!(
            "{{\"sample\":{sample},\"status\":\"work\",\"phase\":\"append\",\"builds\":{},\"layout_calls\":{},\"contribution_ns\":{},\"styled_layout_ns\":{},\"source_geometry_ns\":{},\"build_ns\":{}}}",
            append_work.builds - initial_work.builds,
            append_work.styled_layout_calls - initial_work.styled_layout_calls,
            append_work.contribution_ns - initial_work.contribution_ns,
            append_work.styled_layout_ns - initial_work.styled_layout_ns,
            append_work.source_geometry_ns - initial_work.source_geometry_ns,
            append_work.build_ns - initial_work.build_ns
        );
        println!(
            "{{\"sample\":{sample},\"status\":\"ok\",\"fixture\":\"{workload}\",\"construct_ns\":{construct_ns},\"initial_ns\":{initial_ns},\"dispatch_ns\":{dispatch_ns},\"append_update_ns\":{append_update_ns},\"append_e2e_ns\":{},\"stable_ns\":{stable_ns},\"paragraphs\":{paragraphs},\"bytes\":{bytes},\"lines\":{lines},\"runs\":{runs},\"fragments\":{fragments},\"effective_width\":{width},\"present\":\"unavailable\"}}",
            dispatch_ns + append_update_ns
        );
        if extended {
            let root = runtime.tree().contents().unwrap();
            let pane = runtime.tree().child_order(root).unwrap()[0];
            let viewport = runtime
                .tree()
                .child_order(pane)
                .unwrap()
                .into_iter()
                .find(|id| runtime.tree().node(*id).unwrap().figure_name() == "ViewportFigure")
                .unwrap();
            let work_before = snapshot(&runtime).statistics;
            let start = Instant::now();
            runtime.viewports().scroll_by(viewport, 0.0, 100.0).unwrap();
            runtime.frames().stabilize_for_query().unwrap();
            let scroll_ns = start.elapsed().as_nanos();
            let scroll_calls =
                snapshot(&runtime).statistics.styled_layout_calls - work_before.styled_layout_calls;
            runtime.frames().take_timings();
            let published_revision = snapshot(&runtime).revision;
            let start = Instant::now();
            runtime
                .figure(root)
                .unwrap()
                .set_bounds(novadraw::geometry::Rectangle::new(0.0, 0.0, 400.0, 500.0))
                .unwrap();
            if let Err(error) = runtime.frames().stabilize_for_query() {
                let resize_ns = start.elapsed().as_nanos();
                eprintln!("sample {sample}: resize {error:?}");
                for timing in runtime.frames().take_timings() {
                    println!(
                        "{{\"sample\":{sample},\"status\":\"work\",\"phase\":\"runtime-resize-failure/{}\",\"calls\":{},\"inclusive_ns\":{},\"exclusive_ns\":{}}}",
                        timing.stage, timing.calls, timing.inclusive_ns, timing.exclusive_ns
                    );
                }
                println!(
                    "{{\"sample\":{sample},\"status\":\"resize_failure\",\"resize_ns\":{resize_ns},\"published_revision_unchanged\":{}}}",
                    snapshot(&runtime).revision == published_revision
                );
                continue;
            }
            let resize_ns = start.elapsed().as_nanos();
            let width = snapshot(&runtime).paragraphs[0]
                .layout
                .request()
                .constraints()
                .max_width
                .unwrap();
            let resize_calls = snapshot(&runtime).statistics.styled_layout_calls
                - work_before.styled_layout_calls
                - scroll_calls;
            assert_ne!(width, 586.0);
            let start = Instant::now();
            let frame = runtime
                .frames()
                .prepare_submission(
                    novadraw::render::SurfaceInfo {
                        logical_width: 400.0,
                        logical_height: 500.0,
                        pixel_width: 400,
                        pixel_height: 500,
                        scale_factor: 1.0,
                    },
                    novadraw::render::BackendCapabilities::RETAINED_PARTIAL
                        .with_path_clips()
                        .with_custom_strokes(),
                )
                .into_ready()
                .expect("CPU IR preparation");
            let record_ns = start.elapsed().as_nanos();
            let glyphs: usize = frame
                .commands
                .iter()
                .map(|command| match &command.kind {
                    novadraw::render::RenderCommandKind::DrawGlyphRun { run, .. } => {
                        run.glyphs.len()
                    }
                    _ => 0,
                })
                .sum();
            println!(
                "{{\"sample\":{sample},\"status\":\"extended\",\"scroll_ns\":{scroll_ns},\"resize_ns\":{resize_ns},\"record_ns\":{record_ns},\"scroll_layout_calls\":{scroll_calls},\"resize_layout_calls\":{resize_calls},\"resize_width\":{width},\"commands\":{},\"native_glyphs\":{glyphs},\"present\":\"unavailable\"}}",
                frame.commands.len()
            );
        }
    }
}
