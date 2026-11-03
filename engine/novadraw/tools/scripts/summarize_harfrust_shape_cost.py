#!/usr/bin/env python3
"""Validate actual shaping cost samples, mode identities and rejection prefix."""
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
    raw = output / "shape-cost.ndjson"
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(rows) != 16000:
        raise SystemExit("Wrong sample denominator")
    groups = {}
    for font in [0, 1]:
        for spare in [False, True]:
            for mode in ["ordinary", "noop", "count", "reject-last"]:
                group = [row for row in rows if (row["font"], row["spare"], row["path"])
                         == (font, spare, mode)]
                if sorted(row["sample"] for row in group) != list(range(1000)):
                    raise SystemExit("Wrong sample identity")
                if {row["rejected"] for row in group} != {mode == "reject-last"}:
                    raise SystemExit("Wrong rejection state")
                if {row["characters"] for row in group} != {960}:
                    raise SystemExit("Wrong character denominator")
                costs = {(row["work"], row["allocation"], row["calls"]) for row in group}
                if len(costs) != 1:
                    raise SystemExit("Unstable work or allocation")
                work, allocation, calls = next(iter(costs))
                if allocation != 0:
                    raise SystemExit("Nonexpanding shape unexpectedly allocated")
                times = sorted(row["elapsed_ns"] / 1000 for row in group)
                groups[f"{font}/{spare}/{mode}"] = {
                    "samples": len(group), "median_us": times[499],
                    "p95_us": times[math.ceil(len(times) * .95) - 1],
                    "p99_us": times[math.ceil(len(times) * .99) - 1],
                    "max_us": times[-1], "work": work, "allocation": allocation, "calls": calls,
                }
            a, b = groups[f"{font}/{spare}/count"], groups[f"{font}/{spare}/reject-last"]
            if a["calls"] != b["calls"] or a["work"] - b["work"] != 960:
                raise SystemExit("Last rejection prefix differs")
    receipt = {
        "samples": len(rows), "samples_per_input_mode": 1000, "warmup_per_input_mode": 100,
        "characters": 960, "fonts": ["layout-free InterVariable", "layout-free NotoSansSC-VF"],
        "configuration": "Native release, modes rotate by sample",
        "timer": "actual shaping; input and font/plan preparation and successful drop excluded; "
                 "failed buffer cleanup included",
        "system_load_controlled": False,
        "raw_sha256": harness.sha(raw.read_bytes()),
        "cost_source_sha256": harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/shape_cost.rs").read_bytes()),
        "fixture_sha256": harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/shape_fixtures.rs").read_bytes()),
        "groups": groups,
    }
    (output / "shape-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
