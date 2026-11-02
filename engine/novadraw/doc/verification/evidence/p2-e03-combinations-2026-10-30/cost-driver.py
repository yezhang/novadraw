"""Measure E03 phase boundaries in a disposable, fingerprinted source copy."""

import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


PHASES = [
    "model_clone",
    "command_build",
    "command_execute",
    "snapshot_capture",
    "projection_update",
    "feedback_attach",
    "feedback_cleanup",
    "stabilization",
]

INSTRUMENTATION = """
use std::cell::Cell;
use std::time::Instant;
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static TIMES: Cell<[u128; 8]> = const { Cell::new([0; 8]) };
}
/// One measured phase, enabled only during a sample.
pub struct Span { phase: usize, began: Option<Instant> }
impl Span {
    /// Starts an enabled phase timer without allocating.
    pub fn new(phase: usize) -> Self {
        Self { phase, began: ENABLED.with(|enabled| enabled.get().then(Instant::now)) }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(began) = self.began {
            let elapsed = began.elapsed().as_nanos();
            TIMES.with(|times| {
                let mut values = times.get();
                values[self.phase] += elapsed;
                times.set(values);
            });
        }
    }
}
/// Clears and enables this thread's phase counters.
pub fn reset() {
    TIMES.with(|times| times.set([0; 8]));
    ENABLED.with(|enabled| enabled.set(true));
}
/// Disables recording and returns this thread's counters.
pub fn take() -> [u128; 8] {
    ENABLED.with(|enabled| enabled.set(false));
    TIMES.with(Cell::get)
}
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    isolated = Path(tempfile.mkdtemp(prefix="novadraw-e03-phase-cost-"))
    for name in ("crates/novadraw", "crates/novadraw-editor", "assets/fonts"):
        shutil.copytree(root / name, isolated / name)
    manifest = (root / "Cargo.toml").read_text()
    (isolated / "Cargo.toml").write_text(
        '[workspace]\nresolver = "3"\n'
        'members = ["crates/novadraw", "crates/novadraw-editor"]\n\n'
        + manifest[manifest.index("[workspace.package]"):]
    )
    shutil.copy2(root / "Cargo.lock", isolated / "Cargo.lock")
    patch = []
    fingerprints = {}

    def edit(name, transform):
        original = (root / name).read_text()
        changed = transform(original)
        if changed == original:
            raise RuntimeError(f"No instrumentation added: {name}")
        (isolated / name).write_text(changed)
        fingerprints[name] = hashlib.sha256(original.encode()).hexdigest()
        patch.extend(difflib.unified_diff(
            original.splitlines(True), changed.splitlines(True),
            fromfile="a/" + name, tofile="b/" + name,
        ))

    def replace_once(text, old, new):
        if text.count(old) != 1:
            raise RuntimeError(f"Expected one instrumentation boundary: {old[:100]}")
        return text.replace(old, new, 1)

    edit("crates/novadraw-editor/src/lib.rs",
         lambda text: text + "\n/// Isolated verification instrumentation.\n"
         "pub mod e03_phase_cost;\n")
    (isolated / "crates/novadraw-editor/src/e03_phase_cost.rs").write_text(INSTRUMENTATION)
    patch.extend(difflib.unified_diff(
        [], INSTRUMENTATION.splitlines(True),
        fromfile="/dev/null", tofile="b/crates/novadraw-editor/src/e03_phase_cost.rs",
    ))

    def projection(text):
        text = replace_once(
            text, "let snapshot = match ModelSnapshot::capture(&self.model) {",
            "let snapshot_started = crate::e03_phase_cost::Span::new(3);\n"
            "        let snapshot = match ModelSnapshot::capture(&self.model) {",
        )
        text = replace_once(
            text, "        let result = self.parts_to_retire(&snapshot).and_then(|retiring| {",
            "        drop(snapshot_started);\n"
            "        let projection_started = crate::e03_phase_cost::Span::new(4);\n"
            "        let result = self.parts_to_retire(&snapshot).and_then(|retiring| {",
        )
        return replace_once(
            text, "        match result {\n            Ok(()) => {",
            "        drop(projection_started);\n"
            "        match result {\n            Ok(()) => {",
        )

    edit("crates/novadraw-editor/src/viewer/projection.rs", projection)

    def viewer(text):
        for signature, phase in [
            ("pub fn stabilize_for_query(", 7),
            ("pub fn remove_overlay_visuals(", 6),
            ("fn add_feedback_batch(", 5),
        ]:
            start = text.index(signature)
            body = text.index("{", start)
            text = text[:body + 1] + (
                f"\n        let _cost = crate::e03_phase_cost::Span::new({phase});"
            ) + text[body + 1:]
        return text

    edit("crates/novadraw-editor/src/viewer/mod.rs", viewer)

    def fixture(text):
        # The fixture's model commands deliberately clone complete snapshots.
        # Measure the actual clones; do not replace the application algorithm.
        text = text.replace(
            "        let before = model.nodes.clone();\n        let mut after = before.clone();",
            "        let model_clone_started = novadraw_editor::e03_phase_cost::Span::new(0);\n"
            "        let before = model.nodes.clone();\n        let mut after = before.clone();\n"
            "        drop(model_clone_started);",
        )
        text = text.replace(
            "        model.nodes = self.after.clone();",
            "        {\n"
            "            let _clone = novadraw_editor::e03_phase_cost::Span::new(0);\n"
            "            model.nodes = self.after.clone();\n"
            "        }",
        )
        start = text.index("impl MovePlan<Model> for Plan")
        pos = text.index("        self.trace.borrow_mut().build_count += 1;", start)
        text = text[:pos] + (
            "        let _build = novadraw_editor::e03_phase_cost::Span::new(1);\n"
        ) + text[pos:]
        start = text.index("impl Command<Model> for AtomicMove")
        pos = text.index(
            "    fn execute(&mut self, model: &mut Model) -> Result<(), CommandError> {", start
        )
        pos = text.index("{", pos) + 1
        text = text[:pos] + (
            "\n        let _execute = novadraw_editor::e03_phase_cost::Span::new(2);"
        ) + text[pos:]
        start = text.index("fn container_scaling_samples_include_multiple_parents")
        end = text.index("#[cfg(test)]\nfn print_scaling", start)
        section = text[start:end]
        section = replace_once(
            section, "        for _ in 0..SAMPLES {\n            let parts = sources",
            "        for sample in 0..SAMPLES {\n            let parts = sources",
        )
        section = replace_once(
            section, "            let (_, observation) = measure(|| {\n                domain",
            "            novadraw_editor::e03_phase_cost::reset();\n"
            "            let preview_started = std::time::Instant::now();\n"
            "            domain.pointer_moved(&mut viewer, Point::new(400.0, 120.0)).unwrap();\n"
            "            let preview_ns = preview_started.elapsed().as_nanos();\n"
            "            let preview_phases = novadraw_editor::e03_phase_cost::take();\n"
            "            println!(\"e03_preview scenario={label} sample={sample} "
            "total_ns={preview_ns} values={preview_phases:?}\");\n"
            "            novadraw_editor::e03_phase_cost::reset();\n"
            "            let (_, observation) = measure(|| {\n                domain",
        )
        section = replace_once(
            section, "            commits.push(observation);",
            "            let release_phases = novadraw_editor::e03_phase_cost::take();\n"
            "            println!(\"e03_phases scenario={label} sample={sample} "
            "nodes={nodes} release_ns={} values={release_phases:?}\", observation.0);\n"
            "            novadraw_editor::e03_phase_cost::reset();\n"
            "            let stable_started = std::time::Instant::now();\n"
            "            viewer.stabilize_for_query().unwrap();\n"
            "            let stable_ns = stable_started.elapsed().as_nanos();\n"
            "            let stable_phases = novadraw_editor::e03_phase_cost::take();\n"
            "            println!(\"e03_stable scenario={label} sample={sample} "
            "total_ns={stable_ns} values={stable_phases:?}\");\n"
            "            commits.push(observation);",
        )
        return text[:start] + section + text[end:]

    edit("crates/novadraw-editor/tests/p2_e03_container_editing_contract.rs", fixture)
    (output / "instrumentation.patch").write_text("".join(patch))
    (output / "source-fingerprints.json").write_text(json.dumps(fingerprints, indent=2) + "\n")
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    command = [
        "cargo", "+1.96.0", "test", "--manifest-path", str(isolated / "Cargo.toml"),
        "-p", "novadraw-editor", "--release", "--test", "p2_e03_container_editing_contract",
        "container_scaling_samples_include_multiple_parents_and_large_source_subtrees",
        "--", "--exact", "--ignored", "--nocapture", "--test-threads=1",
    ]
    with (output / "cost.log").open("w") as log:
        result = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        raise SystemExit(f"Cost probe failed; see {output / 'cost.log'}")
    scenarios = {}
    previews = {}
    for line in (output / "cost.log").read_text().splitlines():
        match = re.search(
            r"e03_phases scenario=(\w+) sample=(\d+) nodes=(\d+) release_ns=(\d+) values=(\[.*\])",
            line,
        )
        if match:
            name, sample, nodes, release, values = match.groups()
            scenarios.setdefault(name, []).append({
                "sample": int(sample), "nodes": int(nodes), "release_ns": int(release),
                **dict(zip(PHASES, json.loads(values))),
                **previews.pop((name, int(sample))),
            })
        match = re.search(r"e03_preview scenario=(\w+) sample=(\d+) total_ns=(\d+) values=(\[.*\])", line)
        if match:
            name, sample, total, values = match.groups()
            phases = json.loads(values)
            previews[(name, int(sample))] = {
                "preview_ns": int(total),
                "preview_feedback_attach": phases[5],
                "preview_feedback_cleanup": phases[6],
                "preview_stabilization": phases[7],
            }
        match = re.search(r"e03_stable scenario=(\w+) sample=(\d+) total_ns=(\d+) values=(\[.*\])", line)
        if match:
            name, sample, total, values = match.groups()
            entry = scenarios[name][-1]
            assert entry["sample"] == int(sample)
            entry["post_release_stabilization"] = int(total)
            entry["stabilization"] += json.loads(values)[7]
    summary = {}
    for name, samples in scenarios.items():
        assert len(samples) == 32
        for entry in samples:
            assert entry["model_clone"] <= entry["command_build"] + entry["command_execute"]
        fields = [field for field in samples[0] if field not in ("sample", "nodes")]
        summary[name] = {
            "samples": len(samples), "nodes": samples[0]["nodes"],
            "timings_ns": {
                field: {
                    "p50": sorted(s[field] for s in samples)[len(samples) // 2],
                    "p95": sorted(s[field] for s in samples)[len(samples) * 95 // 100],
                }
                for field in fields
            },
        }
    assert len(summary) == 4
    (output / "samples.json").write_text(json.dumps(scenarios, indent=2) + "\n")
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    print(f"Isolated copy: {isolated}")


if __name__ == "__main__":
    main()
