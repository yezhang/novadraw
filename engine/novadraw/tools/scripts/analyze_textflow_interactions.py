"""Summarize matched native interaction timings without requiring tracing."""
import argparse
from collections import defaultdict
import json
import math
from pathlib import Path
import statistics


def distribution(values):
    values = sorted(values)
    if not values:
        return {"n": 0}
    return {
        "n": len(values), "p50": statistics.median(values),
        "p95": values[math.ceil(len(values) * .95) - 1],
        "p99": values[math.ceil(len(values) * .99) - 1],
        "mean": statistics.mean(values), "max": values[-1],
    }


def analyze(path):
    document = json.loads(path.read_text())
    operations, frames, inputs, timings, latency = {}, defaultdict(list), {}, defaultdict(dict), {}
    current = None
    for record in document["records"]:
        kind = record["kind"]
        if kind in ("failure", "frame_failure"):
            raise ValueError(f"{path}: failed replay: {record}")
        if kind == "operation":
            if not record["succeeded"]:
                raise ValueError(f"{path}: failed operation: {record}")
            current = record["operation_id"]
            operations[current] = record
        elif kind == "frame":
            current = record["operation_id"]
            frames[current].append(record)
        elif kind == "input_dispatch":
            inputs[record["operation_id"]] = record["data"]
        elif kind == "interaction_latency":
            latency[record["operation_id"]] = record["data"]["elapsed_ns"] / 1e6
        elif kind == "timing" and current is not None:
            # Outer scope drops after its identifying record. Inner diagnostic
            # scopes are intentionally excluded from operation/frame totals.
            if record["phase"] in ("host-operation", "host-frame", "host-input-dispatch",
                                   "host-gpu-queue-wait"):
                phase = record["phase"]
                timings[current][phase] = timings[current].get(phase, 0) + record["elapsed_ns"] / 1e6
    samples = []
    for key, op in operations.items():
        name = op["operation"].split("(")[0]
        if name in ("Reset", "FullDamage") or key not in latency:
            continue
        inp = inputs.get(key)
        delta = (inp["viewport_after_y"] - inp["viewport_before_y"]) if inp else (
            op["viewport_after_y"] - op["viewport_before_y"])
        timing = timings[key]
        frame_ms = timing.get("host-frame", 0)
        operation_ms = timing.get("host-operation", 0)
        input_ms = timing.get("host-input-dispatch", operation_ms)
        gpu_wait_ms = timing.get("host-gpu-queue-wait", 0)
        attempts = frames[key]
        if not attempts or any(frame["outcome"] not in (
                "native_backend_presented", "gpu_offscreen_published", "no_damage", "not_presented")
                for frame in attempts):
            raise ValueError(f"{path}: invalid frame outcome at {key}")
        samples.append({
            "operation_id": key, "operation": name, "delta_y": delta,
            "operation_ms": operation_ms, "input_extra_ms": input_ms - operation_ms,
            "frame_ms": frame_ms, "gpu_wait_ms": gpu_wait_ms,
            "active_ms": input_ms + frame_ms + gpu_wait_ms,
            "latency_ms": latency[key], "attempts": len(attempts),
            "glyphs": sum(frame["recorded_glyphs"] for frame in attempts),
            "commands": sum(frame["commands"] for frame in attempts),
            "snapshot": op["snapshot"],
        })
    for name in ("DragMove", "Wheel"):
        if document["config"]["interaction_replay"] and not any(
                s["operation"] == name and abs(s["delta_y"]) > 1e-6 for s in samples):
            raise ValueError(f"{path}: {name} did not move the viewport")
    return document["config"], samples


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    groups = defaultdict(list)
    runs = []
    for path in args.inputs:
        config, samples = analyze(path)
        runs.append({"path": str(path), "config": config, "samples": samples})
        for sample in samples:
            groups[sample["operation"]].append(sample)
    summary = {
        name: {
            **{metric: distribution([s[metric] for s in samples]) for metric in (
                "active_ms", "operation_ms", "input_extra_ms", "frame_ms", "gpu_wait_ms", "latency_ms",
                "glyphs", "commands")},
            "zero_displacement": sum(abs(s["delta_y"]) <= 1e-6 for s in samples),
            "attempts": distribution([s["attempts"] for s in samples]),
        } for name, samples in groups.items()
    }
    args.output.write_text(json.dumps({"summary": summary, "runs": runs}, indent=2))
    for name, result in summary.items():
        print(name, json.dumps({key: result[key] for key in (
            "active_ms", "frame_ms", "latency_ms", "zero_displacement")}))


if __name__ == "__main__":
    main()
