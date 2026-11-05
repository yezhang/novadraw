"""Alternate frozen release executables through identical native input replay."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

from analyze_textflow_interactions import analyze, distribution


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--cycles", type=int, default=2)
    parser.add_argument("--gpu-wait", action="store_true")
    parser.add_argument("--no-flow-assembly-reuse", action="store_true",
                        help="Compare the default full-assembly path")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    binaries = {"before": args.before.resolve(), "after": args.after.resolve()}
    receipts, groups = [], {}
    for run in range(args.runs):
        for label in (("before", "after") if run % 2 == 0 else ("after", "before")):
            binary = binaries[label]
            directory = (args.output / f"{label}-{run}").resolve()
            command = [str(binary), "--interaction-replay", "--cycles", str(args.cycles),
                       "--timings", "--natural-damage",
                       "--offscreen-gpu", "--exit-after-replay", "--output", str(directory)]
            if not args.no_flow_assembly_reuse:
                command.append("--flow-assembly-reuse")
            if args.gpu_wait:
                command.append("--gpu-wait")
            started = time.time()
            result = subprocess.run(command, capture_output=True, text=True, timeout=180)
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "stdout.txt").write_text(result.stdout)
            (directory / "stderr.txt").write_text(result.stderr)
            result.check_returncode()
            timing_files = list(directory.glob("timings-*.json"))
            if len(timing_files) != 1:
                raise ValueError(f"Expected one timing file in {directory}")
            config, samples = analyze(timing_files[0])
            receipt = {
                "label": label, "run": run, "started": started, "command": command,
                "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "config": config, "samples": samples,
            }
            receipts.append(receipt)
            for sample in samples:
                groups.setdefault((label, sample["operation"]), []).append(sample)
            (args.output / "receipts.json").write_text(json.dumps(receipts, indent=2))
            print(f"{label}-{run}: {len(samples)} operations", flush=True)
    summary = {
        f"{label}/{name}": {
            metric: distribution([s[metric] for s in samples]) for metric in (
                "active_ms", "operation_ms", "input_extra_ms", "frame_ms", "gpu_wait_ms",
                "latency_ms", "glyphs", "commands")
        } for (label, name), samples in groups.items()
    }
    geometry_fields = ("bytes", "lines", "runs", "fragments", "effective_width", "first_source_text")
    baseline = {receipt["run"]: receipt for receipt in receipts if receipt["label"] == "before"}
    compared = 0
    for receipt in receipts:
        if receipt["label"] != "after":
            continue
        before = baseline[receipt["run"]]["samples"]
        after = receipt["samples"]
        if len(before) != len(after):
            raise ValueError("Before/after operation counts differ")
        for old, new in zip(before, after):
            if old["operation"] != new["operation"] or abs(old["delta_y"] - new["delta_y"]) > 1e-6:
                raise ValueError("Before/after native input results differ")
            if any(old["snapshot"][field] != new["snapshot"][field] for field in geometry_fields):
                raise ValueError("Before/after text geometry differs")
            compared += 1
    (args.output / "geometry-check.json").write_text(json.dumps({
        "matched_operations": compared, "fields": geometry_fields,
        "scope": "text summary and viewport displacement; pixel equivalence requires separate capture"
    }, indent=2))
    (args.output / "comparison.json").write_text(json.dumps(summary, indent=2))
    for name in sorted({name for _, name in groups}):
        print(name, json.dumps({
            label: {metric: summary[f"{label}/{name}"][metric]["p50"] for metric in (
                "active_ms", "frame_ms", "latency_ms")}
            for label in binaries
        }), flush=True)


if __name__ == "__main__":
    main()
