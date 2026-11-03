#!/usr/bin/env python3
"""Validate single-substitution cold/hot cost and rejection prefixes."""
import argparse
import json
import math
from pathlib import Path

import check_harfrust_buffer_admission as harness


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    raw = output / "single-cost.ndjson"
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(rows) != 16000:
        raise SystemExit("Wrong sample denominator")
    groups = {}
    for case in [0, 1]:
        for hot in [False, True]:
            for mode in ["ordinary", "noop", "count", "reject-last"]:
                group = [r for r in rows if (r["case"], r["hot"], r["path"]) == (case, hot, mode)]
                if sorted(r["sample"] for r in group) != list(range(1000)):
                    raise SystemExit("Wrong sample identity")
                if {r["rejected"] for r in group} != {mode == "reject-last"}:
                    raise SystemExit("Wrong rejection state")
                if {r["characters"] for r in group} != {960}:
                    raise SystemExit("Wrong character denominator")
                costs = {(r["work"], r["allocation"], r["calls"]) for r in group}
                if len(costs) != 1:
                    raise SystemExit("Unstable logical cost")
                work, allocation, calls = next(iter(costs))
                if hot and allocation != 0:
                    raise SystemExit("Hot lookup unexpectedly allocates")
                if not hot and mode in ["count", "reject-last"] and allocation == 0:
                    raise SystemExit("Cold lookup storage is not charged")
                times = sorted(r["elapsed_ns"] / 1000 for r in group)
                groups[f"{case}/{hot}/{mode}"] = {
                    "samples": len(group), "median_us": times[499],
                    "p95_us": times[math.ceil(len(times) * .95) - 1],
                    "p99_us": times[math.ceil(len(times) * .99) - 1], "max_us": times[-1],
                    "work": work, "allocation": allocation, "calls": calls,
                }
            full, rejected = groups[f"{case}/{hot}/count"], groups[f"{case}/{hot}/reject-last"]
            if full["calls"] != rejected["calls"] or full["work"] - rejected["work"] != 960:
                raise SystemExit("Last rejection prefix differs")
            if full["allocation"] != rejected["allocation"]:
                raise SystemExit("Rejected storage prefix differs")
    receipt = {
        "samples": len(rows), "samples_per_input_mode": 1000, "warmup_per_input_mode": 100,
        "characters": 960, "cases": ["format1/coverage1/1subtable/1lookup", "format2/coverage2/3subtables/2lookups"],
        "configuration": "Native release, mode order rotates per sample",
        "timer": "actual shaping and lookup cache; data/plan/input and hot-cache priming excluded; failed cleanup included",
        "system_load_controlled": False, "raw_sha256": harness.sha(raw.read_bytes()),
        "source_sha256": harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/single_cost.rs").read_bytes()),
        "fixture_sha256": harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/single_fixtures.rs").read_bytes()),
        "groups": groups,
    }
    (output / "single-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
