#!/usr/bin/env python3
"""Validate user-normalization sample denominators and report construction costs."""
import argparse
import hashlib
import json
import math
from collections import defaultdict
from pathlib import Path
from check_harfrust_buffer_admission import artifact_output


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--samples", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output = artifact_output(args.output)
    groups = defaultdict(list)
    for line in args.samples.read_text().splitlines():
        row = json.loads(line)
        if any(row[key] < 0 for key in ["ns", "work", "allocation", "calls"]):
            raise SystemExit("Negative time/cost")
        groups[(row["font"], row["mode"])].append(row)
    expected = {(font, mode) for font in ["inter", "axes12", "axes64"]
                for mode in ["ordinary", "admitted", "reject_last"]}
    if set(groups) != expected:
        raise SystemExit("Missing/unexpected sample groups")
    summary = {}
    for (font, mode), rows in sorted(groups.items()):
        if len(rows) != 1000 or sorted(row["sample"] for row in rows) != list(range(1000)):
            raise SystemExit(f"Invalid sample denominator: {font}/{mode}")
        fees = {(row["work"], row["allocation"], row["calls"]) for row in rows}
        if len(fees) != 1:
            raise SystemExit("Logical fees changed between identical inputs")
        work, allocation, calls = fees.pop()
        if mode == "ordinary":
            if (work, allocation, calls) != (0, 0, 0):
                raise SystemExit("Unexpected ordinary admission fees")
        else:
            if not work or not calls:
                raise SystemExit("Missing admission fees")
            if font.startswith("axes") and allocation != int(font[4:]):
                raise SystemExit("Incorrect whole coordinate spill fee")
        times = sorted(row["ns"] / 1000 for row in rows)
        summary[f"{font}/{mode}"] = {
            "samples": len(rows), "median_us": times[499],
            "p95_us": times[math.ceil(len(times) * .95) - 1],
            "p99_us": times[math.ceil(len(times) * .99) - 1],
            "max_us": times[-1], "work": work, "allocation": allocation, "calls": calls,
        }
    repo = Path(__file__).resolve().parents[2]
    receipt = {
        "samples": 9000,
        "sample_sha256": digest(args.samples),
        "binary_sha256": digest(args.binary),
        "source_sha256": digest(repo / "tests/consumers/harfrust-admission/src/variation_cost.rs"),
        "fixtures_sha256": digest(repo / "tests/consumers/harfrust-admission/src/variation_fixtures.rs"),
        "warmup_per_font_and_mode": 100,
        "mode_order": "deterministically rotated per sample",
        "timer": "constructor only; success drop excluded; rejected candidate cleanup included",
        "scope": "fresh user-space instances; local counter; no Core, shaper cache or GPU",
        "allocation_unit": "whole new heap capacity slots, not bytes",
        "core_improvement_claimed": False,
        "groups": summary,
    }
    args.output.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
