#!/usr/bin/env python3
"""Validate four-mode, three-input language or script plan cost samples."""
import argparse
import json
import math
from pathlib import Path

import check_harfrust_buffer_admission as harness


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--input-kind", choices=["private-language", "general-language", "script-callbacks"],
                        required=True)
    args = parser.parse_args()
    output, kind = harness.artifact_output(args.output), args.input_kind
    raw = output / f"{kind}-cost.ndjson"
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    modes = ["ordinary", "noop", "counted", "reject_last"]
    names = ["arabic", "syriac", "hangul"] if kind == "script-callbacks" else ["inline", "spill", "long"]
    groups = {}
    if len(rows) != 12000:
        raise SystemExit("Wrong sample denominator")
    for name in names:
        for mode in modes:
            group = [row for row in rows if row["font"] == name and row["mode"] == mode]
            if sorted(row["sample"] for row in group) != list(range(1000)):
                raise SystemExit(f"Invalid sample identities: {name}/{mode}")
            times = sorted(row["ns"] / 1000 for row in group)
            costs = {(row["work"], row["allocation"], row["calls"]) for row in group}
            if len(costs) != 1:
                raise SystemExit(f"Unstable operation trace: {name}/{mode}")
            work, allocation, calls = next(iter(costs))
            groups[f"{name}/{mode}"] = {
                "samples": len(group), "median_us": times[499],
                "p95_us": times[math.ceil(len(times) * .95) - 1],
                "p99_us": times[math.ceil(len(times) * .99) - 1],
                "max_us": times[-1], "work": work, "allocation": allocation,
                "calls": calls,
            }
    for name in names:
        if groups[f"{name}/counted"]["calls"] != groups[f"{name}/reject_last"]["calls"]:
            raise SystemExit("Last rejection did not reach the full operation prefix")
    receipt = {
        "samples": len(rows), "warmup_per_input_and_mode": 100,
        "samples_per_input_and_mode": 1000,
        "input_languages": (None if kind == "script-callbacks" else
                            ["x-hbot1", "en-x-hbsclatn-hboteng", "en-x-{a*1024}-hboteng"]
                            if kind == "private-language" else
                            ["en-US", "zh-hant-mo", "und-{a*1024}-fonipa"]),
        "configuration": ("Native release; NotoSansArabic-VF.ttf/NotoSansSC-VF.ttf; modes rotate by sample"
                          if kind == "script-callbacks" else
                          "Native release; InterVariable.ttf; modes rotate by sample"),
        "timer": "fresh plan; successful drop excluded; failed candidate cleanup included",
        "data_instance_and_language_construction_in_timer": False,
        "system_load_controlled": False, "raw_sha256": harness.sha(raw.read_bytes()),
        "cost_source_sha256": harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{kind.replace('-', '_')}_cost.rs").read_bytes()),
        "language_rules_sha256": harness.sha((harness.PRODUCTION / "src/hb/tag_rules.rs").read_bytes()),
        "groups": groups,
    }
    if kind == "script-callbacks":
        receipt["input_scripts"] = names
        receipt["syriac_fixture"] = "Arabic lookup data with sorted script records relabeled arab->syrc"
        receipt["syriac_fixture_sha256"] = harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/script_callbacks_fixtures.rs").read_bytes())
    (output / f"{kind}-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
