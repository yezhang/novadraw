"""Export and summarize scroll scopes from a warm official Perfetto session."""
import argparse
import csv
import io
import json
import math
from collections import defaultdict
from pathlib import Path
import statistics
import subprocess


def distribution(values):
    ordered = sorted(values)
    if not ordered:
        return {"n": 0}
    return {
        "n": len(ordered), "mean": statistics.mean(ordered),
        "median": statistics.median(ordered),
        "p95": ordered[math.ceil(len(ordered) * .95) - 1],
        "max": ordered[-1],
    }


def analyze(args):
    """Requires an already loaded session."""
    args.output.mkdir(parents=True, exist_ok=True)
    schema = subprocess.run(
        [args.processor, "query", "--remote", args.session, "--format", "csv",
         "SELECT * FROM slice LIMIT 0; SELECT * FROM stats LIMIT 0; SELECT * FROM args LIMIT 0;"],
        check=True, capture_output=True, text=True)
    (args.output / "schema.csv").write_text(schema.stdout)
    source = Path(__file__).with_name("textflow_scroll_perfetto.sql")
    statements = [s.strip() for s in source.read_text().split(";") if s.strip()]
    tables = []
    for index, statement in enumerate(statements):
        result = subprocess.run(
            [args.processor, "query", "--remote", args.session,
             "--format", "csv", "-f", "-"],
            input=statement + ";", capture_output=True, text=True,
        )
        if result.returncode:
            raise RuntimeError(result.stderr + result.stdout)
        (args.output / f"query-{index:02}.csv").write_text(result.stdout)
        tables.append(list(csv.DictReader(io.StringIO(result.stdout))))
        if index == 2 and any(r["severity"] in ("error", "data_loss") for r in tables[-1]):
            raise ValueError("Trace has error/data_loss statistics. Rejecting full attribution.")
        if index == 2 and any(
                int(r["value"]) > 0 and (
                    "overwritten" in r["name"]
                    or r["name"] == "packet_skipped_seq_needs_incremental_state_invalid"
                ) for r in tables[-1]):
            raise ValueError("Trace buffer overwrote or skipped packets. Rejecting full attribution.")
        if index in (3, 4) and any(int(value) != 0 for row in tables[-1] for value in row.values()):
            raise ValueError("Trace has unfinished scopes or negative exclusive duration.")
    records, roots, stages = tables[-3:]
    records = [{**r, "record": json.loads(bytes.fromhex(r["record"]))} for r in records]
    (args.output / "records.json").write_text(json.dumps(records, indent=2))
    failures = [r["record"] for r in records if r["record"].get("kind") in (
        "failure", "frame_failure")]
    if failures:
        raise ValueError(f"Replay failed; rejecting performance attribution: {failures}")
    cache_by_operation = {
        r["record"]["operation_id"]: r["record"]["data"] for r in records
        if r["record"].get("kind") == "text_raster_cache"
    }
    by_parent = defaultdict(list)
    for record in records:
        if record["parent_id"].isdigit():
            by_parent[int(record["parent_id"])].append(record["record"])
    roots = {int(r["id"]): r for r in roots}
    stage_by_root = defaultdict(dict)
    for stage in stages:
        stage_by_root[int(stage["root_id"])][stage["name"]] = stage
    operations, frames, inputs = {}, {}, {}
    for root_id, root in roots.items():
        kind = {"host-operation": "operation", "host-frame": "frame",
                "host-input-dispatch": "input_dispatch"}[root["name"]]
        matching = [r for r in by_parent[root_id] if r.get("kind") == kind]
        if len(matching) != 1:
            raise ValueError(f"root {root_id} expected one {kind} record, got {len(matching)}")
        record = matching[0]
        entry = {**record, "root": root, "stages": stage_by_root[root_id]}
        target = {"operation": operations, "frame": frames, "input_dispatch": inputs}[kind]
        if kind == "frame":
            target.setdefault(record["operation_id"], []).append(entry)
        else:
            target[record["operation_id"]] = entry
    groups = defaultdict(list)
    samples = []
    first_mode = min(key for key, operation in operations.items()
                     if operation["operation"].startswith("FullDamage("))
    for operation_id, operation in operations.items():
        if operation_id < first_mode:
            continue
        name = operation["operation"].split("(")[0]
        if name not in ("Scroll", "Wheel", "Pointer", "Stable", "DragStart", "DragMove",
                        "DragEnd", "Resize", "Append"):
            continue
        attempts = frames.get(operation_id, [])
        if len(attempts) != 1:
            raise ValueError(f"operation {operation_id}: expected one frame, got {len(attempts)}")
        frame = attempts[0]
        inp = inputs.get(operation_id)
        root = inp["root"] if inp else operation["root"]
        displacement = (
            inp["data"]["viewport_after_y"] - inp["data"]["viewport_before_y"]
            if inp else operation["viewport_after_y"] - operation["viewport_before_y"]
        )
        sample = {
            "operation_id": operation_id, "mode": "full" if operation["full_redraw"] else "natural",
            "operation": name, "delta_y": displacement,
            "operation_ms": int(operation["root"]["dur"]) / 1e6,
            "input_extra_ms": (int(root["dur"]) - int(operation["root"]["dur"])) / 1e6,
            "frame_ms": int(frame["root"]["dur"]) / 1e6,
            "active_ms": (int(root["dur"]) + int(frame["root"]["dur"])) / 1e6,
            "redraw_gap_ms": (int(frame["root"]["ts"]) - int(root["ts"]) - int(root["dur"])) / 1e6,
            "outcome": frame["outcome"], "commands": frame["commands"],
            "glyphs": frame["recorded_glyphs"], "damage": frame.get("damage"),
            "snapshot": operation["snapshot"], "stages": {},
            "text_raster_cache": cache_by_operation.get(operation_id),
        }
        # Operation and frame are disjoint. Input's exclusive root time is separate.
        for entry in (operation, frame):
            for stage_name, stage in entry["stages"].items():
                destination = sample["stages"].setdefault(
                    stage_name, {"calls": 0, "inclusive_ms": 0, "exclusive_ms": 0})
                destination["calls"] += int(stage["calls"])
                destination["inclusive_ms"] += int(stage["inclusive_ns"]) / 1e6
                destination["exclusive_ms"] += int(stage["exclusive_ns"]) / 1e6
        samples.append(sample)
        groups[(sample["mode"], name)].append(sample)
    summary = {}
    for (mode, name), group in groups.items():
        valid = [r for r in group if name not in ("Scroll", "Wheel", "DragMove") or abs(r["delta_y"]) > 1e-6]
        if not valid:
            summary[f"{mode}/{name}"] = {"attempted": len(group), "successful_scrolls": 0}
            continue
        attempted = len(group)
        group = valid
        stage_names = set().union(*(r["stages"] for r in group))
        summary[f"{mode}/{name}"] = {
            "attempted": attempted, "zero_displacement": attempted - len(group),
            **{key: distribution([r[key] for r in group]) for key in (
                "active_ms", "operation_ms", "input_extra_ms", "frame_ms", "redraw_gap_ms",
                "delta_y", "commands", "glyphs")},
            "outcomes": {outcome: sum(r["outcome"] == outcome for r in group)
                         for outcome in {r["outcome"] for r in group}},
            "damage": {str(damage): sum(r["damage"] == damage for r in group)
                       for damage in {r["damage"] for r in group}},
            "stages": {
                stage: {
                    "calls": sum(r["stages"].get(stage, {}).get("calls", 0) for r in group),
                    "inclusive_mean_ms": statistics.mean(
                        r["stages"].get(stage, {}).get("inclusive_ms", 0) for r in group),
                    "exclusive_mean_ms": statistics.mean(
                        r["stages"].get(stage, {}).get("exclusive_ms", 0) for r in group),
                } for stage in stage_names
            },
        }
    result = {"summary": summary, "samples": samples}
    (args.output / "analysis.json").write_text(json.dumps(result, indent=2))
    for name, group in summary.items():
        print(name, json.dumps({k: group[k] for k in ("active_ms", "frame_ms", "outcomes") if k in group}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--processor", required=True)
    parser.add_argument("--session", required=True)
    parser.add_argument("--trace", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.trace:
        subprocess.run([args.processor, "server", "unix", "--name", args.session,
                        "--daemonize", str(args.trace)], check=True)
    try:
        analyze(args)
    finally:
        if args.trace:
            subprocess.run([args.processor, "server", "kill", args.session], check=False)


if __name__ == "__main__":
    main()
