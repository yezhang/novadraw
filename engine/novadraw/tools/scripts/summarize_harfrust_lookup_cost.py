#!/usr/bin/env python3
"""Validate cold/warm lookup samples and their logical fee denominators."""
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
    modes = ["cold_noop", "cold_counted", "warm_noop", "warm_counted", "reject_last"]
    for line in args.samples.read_text().splitlines():
        row = json.loads(line)
        if any(row[key] < 0 for key in ["ns", "work", "allocation", "calls"]):
            raise SystemExit("Negative cost or time")
        groups[row["font"], row["mode"]].append(row)
    if set(groups) != {(font, mode) for font in ["inter", "arabic", "cjk"] for mode in modes}:
        raise SystemExit("Missing sample groups")
    summary = {}
    for (font, mode), rows in sorted(groups.items()):
        if len(rows) != 1000 or sorted(row["sample"] for row in rows) != list(range(1000)):
            raise SystemExit("Incorrect sample denominator")
        fees = {(row["work"], row["allocation"], row["calls"]) for row in rows}
        if len(fees) != 1:
            raise SystemExit("Changing logical fees")
        work, allocation, calls = fees.pop()
        if mode.endswith("noop") and (work, allocation, calls) != (0, 0, 0):
            raise SystemExit("No-op callback reported costs")
        if mode == "warm_counted" and allocation != 0:
            raise SystemExit("Warm lookup allocated")
        times = sorted(row["ns"] / 1000 for row in rows)
        summary[f"{font}/{mode}"] = {
            "samples": len(rows), "median_us": times[499],
            "p95_us": times[math.ceil(len(times) * .95) - 1],
            "p99_us": times[math.ceil(len(times) * .99) - 1],
            "max_us": times[-1], "work": work, "allocation": allocation, "calls": calls,
        }
    repo = Path(__file__).resolve().parents[2]
    receipt = {
        "samples": 15000, "sample_sha256": digest(args.samples),
        "binary_sha256": digest(args.binary),
        "source_sha256": digest(repo / "tests/consumers/harfrust-admission/src/lookup_cost.rs"),
        "warmup_per_group": 100, "mode_order": "rotated per sample",
        "timer": "one GSUB lookup index 0; ShaperData creation and success drop excluded",
        "baseline": "same admitted kernel, no-op callback; not ordinary shaping",
        "scope": "native cold lookup fill, warm lookup hit and rejected tail; local counter",
        "excluded": ["AAT", "shape plan", "shaping", "Core", "GPU"],
        "allocation_unit": "whole new heap capacity slots, not bytes",
        "core_improvement_claimed": False, "groups": summary,
    }
    args.output.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
