#!/usr/bin/env python3
"""Validate instance-cost sample denominators and summarize repeated construction."""
import argparse
import hashlib
import json
import math
from collections import defaultdict
from pathlib import Path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--samples", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    groups = defaultdict(list)
    for line in args.samples.read_text().splitlines():
        sample = json.loads(line)
        if sample["ns"] < 0 or any(sample[key] < 0 for key in ["work", "allocation", "calls"]):
            raise SystemExit("Negative sample or cost")
        groups[(sample["font"], sample["mode"])].append(sample)
    expected = {(font, mode) for font in ["inter", "axes12", "axes64"]
                for mode in ["ordinary", "admitted", "reject_last"]}
    if set(groups) != expected:
        raise SystemExit("Missing or unexpected cost groups")
    summary = {}
    for (font, mode), samples in sorted(groups.items()):
        if len(samples) != 1000 or sorted(s["sample"] for s in samples) != list(range(1000)):
            raise SystemExit(f"Invalid sample denominator: {font}/{mode}")
        fees = {(s["work"], s["allocation"], s["calls"]) for s in samples}
        if len(fees) != 1:
            raise SystemExit(f"Unstable logical fees: {font}/{mode}")
        work, allocation, calls = fees.pop()
        if mode == "ordinary" and (work, allocation, calls) != (0, 0, 0):
            raise SystemExit("Ordinary mode unexpectedly records admission fees")
        if mode != "ordinary" and (not work or not calls):
            raise SystemExit("Admitted mode does not record fees")
        if font == "axes12" and mode != "ordinary" and allocation != 12:
            raise SystemExit("Incorrect 12-axis spill fee")
        if font == "axes64" and mode != "ordinary" and allocation != 64:
            raise SystemExit("Incorrect 64-axis spill fee")
        times = sorted(s["ns"] / 1000 for s in samples)
        summary[f"{font}/{mode}"] = {
            "samples": len(samples),
            "median_us": times[math.ceil(len(times) * .5) - 1],
            "p95_us": times[math.ceil(len(times) * .95) - 1],
            "p99_us": times[math.ceil(len(times) * .99) - 1],
            "max_us": times[-1],
            "work": work,
            "allocation": allocation,
            "calls": calls,
        }
    repo = Path(__file__).resolve().parents[2]
    receipt = {
        "samples": sum(len(samples) for samples in groups.values()),
        "sample_sha256": digest(args.samples),
        "binary_sha256": digest(args.binary),
        "benchmark_source_sha256": digest(
            repo / "tests/consumers/harfrust-admission/src/instance_cost.rs"),
        "fixtures_source_sha256": digest(
            repo / "tests/consumers/harfrust-admission/src/instance_fixtures.rs"),
        "mode_order": "deterministically rotated for each sample",
        "warmup_per_mode_and_font": 100,
        "timer": "constructor only; success destruction excluded; rejection cleanup included",
        "allocation_unit": "whole new heap capacity slots, not bytes",
        "scope": "repeated fresh normalized instances, no shaper data, Core or GPU",
        "environment_qualified_latency_or_core_improvement_claimed": False,
        "groups": summary,
    }
    args.output.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
