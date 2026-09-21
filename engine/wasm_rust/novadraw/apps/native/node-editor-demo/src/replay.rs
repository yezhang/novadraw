use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use novadraw::{KeyModifiers, MouseButton, Point, Rectangle};
use novadraw_editor::HandleRole;
use serde::Serialize;

use super::FIRST_CONNECTION_ID;
use super::harness::{EditorHarness, HarnessResult};

const BLUE_NODE: u64 = 2;
const GREEN_NODE: u64 = 3;
const WIDGET_NODE: u64 = 4;

#[derive(Debug)]
pub(crate) struct ReplayError(String);

impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for ReplayError {}

impl From<String> for ReplayError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

pub(crate) struct ReplayOptions {
    selector: String,
    report: PathBuf,
}

impl ReplayOptions {
    pub(crate) fn parse() -> Result<Option<Self>, ReplayError> {
        let mut selector = None;
        let mut report = None;
        for argument in std::env::args().skip(1) {
            if let Some(value) = argument.strip_prefix("--headless-replay=") {
                selector = Some(value.to_string());
            } else if let Some(value) = argument.strip_prefix("--report=") {
                report = Some(PathBuf::from(value));
            } else {
                return Err(ReplayError(format!(
                    "unknown node-editor-demo argument `{argument}`"
                )));
            }
        }
        let Some(selector) = selector else {
            if report.is_some() {
                return Err(ReplayError(
                    "--report requires --headless-replay=<suite>".to_string(),
                ));
            }
            return Ok(None);
        };
        let report = report.unwrap_or_else(|| {
            PathBuf::from("target/verification/reports")
                .join(format!("node-editor-{selector}.json"))
        });
        Ok(Some(Self { selector, report }))
    }
}

#[derive(Serialize)]
struct ReplayReport {
    schema_version: u32,
    app: &'static str,
    selector: String,
    generated_at_unix_seconds: u64,
    passed: bool,
    cases: Vec<ReplayCaseReport>,
}

#[derive(Serialize)]
struct ReplayCaseReport {
    name: &'static str,
    passed: bool,
    checkpoints: Vec<&'static str>,
    metrics: BTreeMap<&'static str, String>,
    error: Option<String>,
}

struct ReplayEvidence {
    checkpoints: Vec<&'static str>,
    metrics: BTreeMap<&'static str, String>,
}

impl ReplayEvidence {
    fn new() -> Self {
        Self {
            checkpoints: Vec::new(),
            metrics: BTreeMap::new(),
        }
    }

    fn checkpoint(
        &mut self,
        name: &'static str,
        condition: bool,
        failure: impl FnOnce() -> String,
    ) -> Result<(), ReplayError> {
        if !condition {
            return Err(ReplayError(failure()));
        }
        self.checkpoints.push(name);
        Ok(())
    }

    fn metric(&mut self, name: &'static str, value: impl ToString) {
        self.metrics.insert(name, value.to_string());
    }
}

struct ReplayCase {
    name: &'static str,
    selector: &'static str,
    run: fn(&mut ReplayEvidence) -> Result<(), ReplayError>,
}

const CASES: &[ReplayCase] = &[
    ReplayCase {
        name: "selection-and-widget-arbitration",
        selector: "g3",
        run: replay_g3,
    },
    ReplayCase {
        name: "bounds-editing-and-history",
        selector: "g4",
        run: replay_g4,
    },
    ReplayCase {
        name: "connection-creation-and-history",
        selector: "g5.2",
        run: replay_g5_2,
    },
    ReplayCase {
        name: "connection-reconnect-and-history",
        selector: "g5.3",
        run: replay_g5_3,
    },
    ReplayCase {
        name: "connection-bendpoint-and-history",
        selector: "g5.4",
        run: replay_g5_4,
    },
    ReplayCase {
        name: "viewport-autoexpose-and-zoom",
        selector: "g5.5",
        run: replay_g5_5,
    },
];

pub(crate) fn run(options: ReplayOptions) -> Result<(), ReplayError> {
    let selected = CASES
        .iter()
        .filter(|case| options.selector == "all" || case.selector == options.selector)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(ReplayError(format!(
            "unknown replay selector `{}`; expected g3, g4, g5.2, g5.3, g5.4, g5.5, or all",
            options.selector
        )));
    }

    let mut reports = Vec::with_capacity(selected.len());
    for case in selected {
        let mut evidence = ReplayEvidence::new();
        match (case.run)(&mut evidence) {
            Ok(()) => {
                println!("PASS {}", case.name);
                reports.push(ReplayCaseReport {
                    name: case.name,
                    passed: true,
                    checkpoints: evidence.checkpoints,
                    metrics: evidence.metrics,
                    error: None,
                });
            }
            Err(error) => {
                eprintln!("FAIL {}: {error}", case.name);
                reports.push(ReplayCaseReport {
                    name: case.name,
                    passed: false,
                    checkpoints: evidence.checkpoints,
                    metrics: evidence.metrics,
                    error: Some(error.to_string()),
                });
            }
        }
    }

    let passed = reports.iter().all(|case| case.passed);
    let report = ReplayReport {
        schema_version: 1,
        app: "node-editor-demo",
        selector: options.selector,
        generated_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        passed,
        cases: reports,
    };
    if let Some(parent) = options.report.parent() {
        fs::create_dir_all(parent).map_err(|error| ReplayError(error.to_string()))?;
    }
    let json =
        serde_json::to_string_pretty(&report).map_err(|error| ReplayError(error.to_string()))?;
    fs::write(&options.report, json).map_err(|error| ReplayError(error.to_string()))?;
    println!("REPORT {}", options.report.display());

    if passed {
        Ok(())
    } else {
        Err(ReplayError(format!(
            "headless replay `{}` failed",
            report.selector
        )))
    }
}

fn replay_g3(evidence: &mut ReplayEvidence) -> Result<(), ReplayError> {
    let mut harness = EditorHarness::new()?;

    harness.click(center(&harness, BLUE_NODE)?, KeyModifiers::default())?;
    evidence.checkpoint(
        "single-selection",
        harness.selected_model_ids() == [BLUE_NODE],
        || "blue node was not the sole selection".to_string(),
    )?;

    harness.click(
        center(&harness, GREEN_NODE)?,
        KeyModifiers {
            shift: true,
            ..KeyModifiers::default()
        },
    )?;
    evidence.checkpoint(
        "ordered-multi-selection",
        harness.selected_model_ids() == [BLUE_NODE, GREEN_NODE],
        || "shift-click did not preserve ordered multi-selection".to_string(),
    )?;

    harness.click(center(&harness, WIDGET_NODE)?, KeyModifiers::default())?;
    evidence.checkpoint(
        "widget-arbitration",
        harness.selected_model_ids() == [BLUE_NODE, GREEN_NODE],
        || "widget input escaped into Editor selection fallback".to_string(),
    )?;

    harness.click(Point::new(780.0, 500.0), KeyModifiers::default())?;
    evidence.checkpoint(
        "contents-clear",
        harness.selected_model_ids().is_empty(),
        || "contents click did not clear selection".to_string(),
    )?;
    evidence.metric("model_revision", harness.model_revision());
    Ok(())
}

fn replay_g4(evidence: &mut ReplayEvidence) -> Result<(), ReplayError> {
    let mut harness = EditorHarness::new()?;
    let blue_before = bounds(&harness, BLUE_NODE)?;
    let blue_center = blue_before.center();
    let move_target = Point::new(blue_center.x() + 40.0, blue_center.y() + 30.0);

    harness.click(blue_center, KeyModifiers::default())?;
    let committed = harness.drag(blue_center, move_target, KeyModifiers::default())?;
    let blue_moved = bounds(&harness, BLUE_NODE)?;
    evidence.checkpoint(
        "move-command",
        committed
            && blue_moved
                == Rectangle::new(
                    blue_before.x + 40.0,
                    blue_before.y + 30.0,
                    blue_before.width,
                    blue_before.height,
                ),
        || "move gesture did not commit the expected bounds".to_string(),
    )?;

    evidence.checkpoint(
        "move-undo",
        harness.undo()? && bounds(&harness, BLUE_NODE)? == blue_before,
        || "move undo did not restore the original bounds".to_string(),
    )?;
    evidence.checkpoint(
        "move-redo",
        harness.redo()? && bounds(&harness, BLUE_NODE)? == blue_moved,
        || "move redo did not restore the moved bounds".to_string(),
    )?;

    let resize_start = Point::new(
        blue_moved.x + blue_moved.width,
        blue_moved.y + blue_moved.height,
    );
    let resize_end = Point::new(resize_start.x() + 24.0, resize_start.y() + 18.0);
    let resized = harness.drag(resize_start, resize_end, KeyModifiers::default())?;
    let resized_bounds = bounds(&harness, BLUE_NODE)?;
    evidence.checkpoint(
        "resize-command",
        resized
            && resized_bounds.width == blue_moved.width + 24.0
            && resized_bounds.height == blue_moved.height + 18.0,
        || "south-east resize did not commit the expected dimensions".to_string(),
    )?;

    let nodes_before_create = harness.node_count();
    harness.create_node()?;
    evidence.checkpoint(
        "create-command",
        harness.node_count() == nodes_before_create + 1,
        || "create request did not add one model node".to_string(),
    )?;
    evidence.checkpoint(
        "create-undo",
        harness.undo()? && harness.node_count() == nodes_before_create,
        || "create undo did not remove the new node".to_string(),
    )?;
    evidence.metric("model_revision", harness.model_revision());
    let (undo, redo) = harness.history_lengths();
    evidence.metric("undo_depth", undo);
    evidence.metric("redo_depth", redo);
    Ok(())
}

fn replay_g5_2(evidence: &mut ReplayEvidence) -> Result<(), ReplayError> {
    let mut harness = EditorHarness::new()?;
    create_connection(&mut harness)?;

    evidence.checkpoint(
        "connection-created",
        harness.connection_ids() == [FIRST_CONNECTION_ID]
            && harness.connection_endpoints(FIRST_CONNECTION_ID) == Some((BLUE_NODE, GREEN_NODE)),
        || "two-stage creation did not commit the expected connection".to_string(),
    )?;
    evidence.checkpoint(
        "creation-tool-finished",
        !harness.has_active_gesture() && !harness.is_connection_creation_active(),
        || "connection creation left an active Tool gesture".to_string(),
    )?;
    evidence.checkpoint(
        "connection-create-undo",
        harness.undo()? && harness.connection_ids().is_empty(),
        || "connection creation undo did not remove the connection".to_string(),
    )?;
    evidence.checkpoint(
        "connection-create-redo",
        harness.redo()?
            && harness.connection_endpoints(FIRST_CONNECTION_ID) == Some((BLUE_NODE, GREEN_NODE)),
        || "connection creation redo did not restore the connection".to_string(),
    )?;
    evidence.metric("model_revision", harness.model_revision());
    Ok(())
}

fn replay_g5_3(evidence: &mut ReplayEvidence) -> Result<(), ReplayError> {
    let mut harness = EditorHarness::new()?;
    create_connection(&mut harness)?;
    select_connection(&mut harness)?;

    let route = harness
        .connection_route(FIRST_CONNECTION_ID)
        .ok_or_else(|| ReplayError("connection has no committed route".to_string()))?;
    let target = *route
        .last()
        .ok_or_else(|| ReplayError("connection route is empty".to_string()))?;
    let reconnected = harness.drag(
        target,
        center(&harness, BLUE_NODE)?,
        KeyModifiers::default(),
    )?;
    evidence.checkpoint(
        "target-reconnect",
        reconnected
            && harness.connection_endpoints(FIRST_CONNECTION_ID) == Some((BLUE_NODE, BLUE_NODE)),
        || "target endpoint was not reconnected to the source node".to_string(),
    )?;
    evidence.checkpoint(
        "reconnect-undo",
        harness.undo()?
            && harness.connection_endpoints(FIRST_CONNECTION_ID) == Some((BLUE_NODE, GREEN_NODE)),
        || "reconnect undo did not restore the original endpoints".to_string(),
    )?;
    evidence.checkpoint(
        "reconnect-redo",
        harness.redo()?
            && harness.connection_endpoints(FIRST_CONNECTION_ID) == Some((BLUE_NODE, BLUE_NODE)),
        || "reconnect redo did not restore the self-loop".to_string(),
    )?;
    let blue_loop = harness
        .connection_route(FIRST_CONNECTION_ID)
        .ok_or_else(|| ReplayError("blue self-loop has no committed route".to_string()))?;
    let source = *blue_loop
        .first()
        .ok_or_else(|| ReplayError("blue self-loop route is empty".to_string()))?;
    evidence.checkpoint(
        "self-loop-first-endpoint-reconnect",
        harness.drag(
            source,
            center(&harness, GREEN_NODE)?,
            KeyModifiers::default(),
        )? && harness.connection_endpoints(FIRST_CONNECTION_ID) == Some((GREEN_NODE, BLUE_NODE)),
        || "first self-loop endpoint did not reconnect to green".to_string(),
    )?;
    let green_to_blue = harness
        .connection_route(FIRST_CONNECTION_ID)
        .ok_or_else(|| ReplayError("green-to-blue connection has no route".to_string()))?;
    let target = *green_to_blue
        .last()
        .ok_or_else(|| ReplayError("green-to-blue route is empty".to_string()))?;
    let green_bounds = bounds(&harness, GREEN_NODE)?;
    let green_center = center(&harness, GREEN_NODE)?;
    harness.pointer_pressed(target, MouseButton::Left, KeyModifiers::default())?;
    harness.pointer_moved(green_center)?;
    let feedback = harness.scaled_feedback_point_lists();
    evidence.checkpoint(
        "self-loop-second-endpoint-feedback",
        feedback
            == vec![vec![
                Point::new(
                    green_bounds.x + green_bounds.width,
                    green_bounds.y + green_bounds.height * super::SELF_LOOP_SOURCE_PORT_FRACTION,
                ),
                Point::new(
                    green_bounds.x + green_bounds.width + super::SELF_LOOP_EXTENT,
                    green_bounds.y + green_bounds.height * super::SELF_LOOP_SOURCE_PORT_FRACTION,
                ),
                Point::new(
                    green_bounds.x + green_bounds.width + super::SELF_LOOP_EXTENT,
                    green_bounds.y + green_bounds.height * super::SELF_LOOP_TARGET_PORT_FRACTION,
                ),
                Point::new(
                    green_bounds.x + green_bounds.width,
                    green_bounds.y + green_bounds.height * super::SELF_LOOP_TARGET_PORT_FRACTION,
                ),
            ]],
        || "second endpoint feedback did not preview the green self-loop geometry".to_string(),
    )?;
    evidence.checkpoint(
        "self-loop-second-endpoint-reconnect",
        harness.pointer_released(green_center, MouseButton::Left)?
            && harness.connection_endpoints(FIRST_CONNECTION_ID) == Some((GREEN_NODE, GREEN_NODE)),
        || "second self-loop endpoint did not reconnect to green".to_string(),
    )?;
    let green_bendpoints = harness.connection_bendpoints(FIRST_CONNECTION_ID);
    let green_loop = harness
        .connection_route(FIRST_CONNECTION_ID)
        .ok_or_else(|| ReplayError("green self-loop has no committed route".to_string()))?;
    evidence.checkpoint(
        "self-loop-owner-rebase",
        green_bendpoints.len() == 2
            && green_bendpoints
                .iter()
                .all(|point| point.x() > green_bounds.x + green_bounds.width)
            && green_loop.windows(2).all(|segment| {
                segment[0].x() == segment[1].x() || segment[0].y() == segment[1].y()
            }),
        || "green self-loop retained bendpoints from the previous owner".to_string(),
    )?;
    evidence.metric("model_revision", harness.model_revision());
    Ok(())
}

fn replay_g5_4(evidence: &mut ReplayEvidence) -> Result<(), ReplayError> {
    let mut harness = EditorHarness::new()?;
    create_connection(&mut harness)?;
    select_connection(&mut harness)?;

    let create_site = harness
        .bendpoint_handle_sites(FIRST_CONNECTION_ID)?
        .into_iter()
        .find(|site| site.role() == HandleRole::BendpointCreate(0))
        .ok_or_else(|| ReplayError("connection has no first create handle".to_string()))?;
    let created = Point::new(
        create_site.location().x(),
        create_site.location().y() + 80.0,
    );
    evidence.checkpoint(
        "bendpoint-create-command",
        harness.drag(create_site.location(), created, KeyModifiers::default())?
            && harness.connection_bendpoints(FIRST_CONNECTION_ID) == [created],
        || "create handle did not commit one bendpoint".to_string(),
    )?;
    evidence.checkpoint(
        "bendpoint-create-undo",
        harness.undo()?
            && harness
                .connection_bendpoints(FIRST_CONNECTION_ID)
                .is_empty(),
        || "bendpoint create undo did not restore the direct route".to_string(),
    )?;
    evidence.checkpoint(
        "bendpoint-create-redo",
        harness.redo()? && harness.connection_bendpoints(FIRST_CONNECTION_ID) == [created],
        || "bendpoint create redo did not restore the point".to_string(),
    )?;

    let move_site = harness
        .bendpoint_handle_sites(FIRST_CONNECTION_ID)?
        .into_iter()
        .find(|site| site.role() == HandleRole::BendpointMove(0))
        .ok_or_else(|| ReplayError("connection has no first move handle".to_string()))?;
    let moved = Point::new(
        move_site.location().x() + 24.0,
        move_site.location().y() + 18.0,
    );
    evidence.checkpoint(
        "bendpoint-move-command",
        harness.drag(move_site.location(), moved, KeyModifiers::default())?
            && harness.connection_bendpoints(FIRST_CONNECTION_ID) == [moved],
        || "move handle did not update the bendpoint".to_string(),
    )?;

    let move_site = harness
        .bendpoint_handle_sites(FIRST_CONNECTION_ID)?
        .into_iter()
        .find(|site| site.role() == HandleRole::BendpointMove(0))
        .ok_or_else(|| ReplayError("moved bendpoint has no move handle".to_string()))?;
    let collapse = midpoint(center(&harness, BLUE_NODE)?, center(&harness, GREEN_NODE)?);
    evidence.checkpoint(
        "bendpoint-collapse-command",
        harness.drag(move_site.location(), collapse, KeyModifiers::default())?
            && harness
                .connection_bendpoints(FIRST_CONNECTION_ID)
                .is_empty(),
        || "moving the bendpoint onto the direct segment did not delete it".to_string(),
    )?;
    evidence.checkpoint(
        "bendpoint-collapse-undo",
        harness.undo()? && harness.connection_bendpoints(FIRST_CONNECTION_ID) == [moved],
        || "collapse undo did not restore the moved bendpoint".to_string(),
    )?;
    evidence.metric("model_revision", harness.model_revision());
    let (undo, redo) = harness.history_lengths();
    evidence.metric("undo_depth", undo);
    evidence.metric("redo_depth", redo);
    Ok(())
}

fn replay_g5_5(evidence: &mut ReplayEvidence) -> Result<(), ReplayError> {
    let mut harness = EditorHarness::new()?;
    harness.runtime_mut().prepare_frame();
    harness.zoom_by(2.0, Point::new(0.0, 0.0))?;
    let before = bounds(&harness, BLUE_NODE)?;
    let start = harness
        .node_bounds_in_surface(BLUE_NODE)
        .ok_or_else(|| ReplayError("blue node has no surface bounds".to_string()))?
        .center();
    let edge = Point::new(815.0, 555.0);

    harness.pointer_pressed(start, novadraw::MouseButton::Left, KeyModifiers::default())?;
    harness.pointer_moved(edge)?;
    evidence.checkpoint("edge-detect", harness.autoexpose_requested(), || {
        "active drag did not request auto-expose at the viewport edge".to_string()
    })?;
    let revision = harness.model_revision();
    let origin_before = harness.viewport_origin()?;
    let tick = harness.autoexpose_tick(Duration::from_millis(30))?;
    let origin_after = harness.viewport_origin()?;
    evidence.checkpoint(
        "autoexpose-scroll",
        tick.scrolled()
            && origin_after.x() > origin_before.x()
            && origin_after.y() > origin_before.y(),
        || "corner auto-expose did not advance both viewport axes".to_string(),
    )?;
    evidence.checkpoint(
        "transient-model-stability",
        harness.model_revision() == revision,
        || "auto-expose mutated the application model before release".to_string(),
    )?;
    evidence.checkpoint(
        "single-command-release",
        harness.pointer_released(edge, novadraw::MouseButton::Left)?
            && harness.node_bounds(BLUE_NODE) != Some(before),
        || "auto-exposed drag did not commit exactly one move command".to_string(),
    )?;
    evidence.checkpoint(
        "autoexpose-undo",
        harness.undo()? && harness.node_bounds(BLUE_NODE) == Some(before),
        || "undo did not restore the pre-auto-expose node bounds".to_string(),
    )?;
    evidence.metric("zoom", harness.viewport_scale()?);
    evidence.metric("viewport_x", origin_after.x());
    evidence.metric("viewport_y", origin_after.y());
    Ok(())
}

fn create_connection(harness: &mut EditorHarness) -> HarnessResult<()> {
    harness.activate_connection_creation()?;
    harness.click(center(harness, BLUE_NODE)?, KeyModifiers::default())?;
    harness.pointer_moved(center(harness, GREEN_NODE)?)?;
    harness.click(center(harness, GREEN_NODE)?, KeyModifiers::default())
}

fn select_connection(harness: &mut EditorHarness) -> HarnessResult<()> {
    let route = harness
        .connection_route(FIRST_CONNECTION_ID)
        .ok_or_else(|| "connection has no committed route".to_string())?;
    let location = route
        .windows(2)
        .max_by(|left, right| {
            segment_length(left)
                .partial_cmp(&segment_length(right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|segment| midpoint(segment[0], segment[1]))
        .ok_or_else(|| "connection route has no selectable segment".to_string())?;
    harness.click(location, KeyModifiers::default())
}

fn center(harness: &EditorHarness, id: u64) -> HarnessResult<Point> {
    harness
        .node_bounds(id)
        .map(|bounds| bounds.center())
        .ok_or_else(|| format!("node {id} is missing"))
}

fn bounds(harness: &EditorHarness, id: u64) -> HarnessResult<Rectangle> {
    harness
        .node_bounds(id)
        .ok_or_else(|| format!("node {id} is missing"))
}

fn midpoint(left: Point, right: Point) -> Point {
    (left + right) / 2.0
}

fn segment_length(segment: &[Point]) -> f64 {
    (segment[1] - segment[0]).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g3_replay_passes() {
        replay_g3(&mut ReplayEvidence::new()).unwrap();
    }

    #[test]
    fn g4_replay_passes() {
        replay_g4(&mut ReplayEvidence::new()).unwrap();
    }

    #[test]
    fn g5_connection_replays_pass() {
        replay_g5_2(&mut ReplayEvidence::new()).unwrap();
        replay_g5_3(&mut ReplayEvidence::new()).unwrap();
        replay_g5_4(&mut ReplayEvidence::new()).unwrap();
        replay_g5_5(&mut ReplayEvidence::new()).unwrap();
    }
}
