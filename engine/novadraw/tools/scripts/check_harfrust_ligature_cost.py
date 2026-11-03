#!/usr/bin/env python3
"""Sample inline/spilled matching costs and verify the rejection prefix."""
import argparse
import json
import math
from pathlib import Path

import check_harfrust_buffer_admission as harness
import text_admission_build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output, target = harness.artifact_output(args.output), args.target_dir.resolve()
    env = text_admission_build.environment(target, args.linker)
    source = harness.REPO / "tests/consumers/harfrust-admission/src/ligature_cost.rs"
    manifest = harness.REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    harness.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                 str(manifest), "--release", "--bin", "ligature-cost"],
                output / "ligature-cost-build.log", env)
    raw = output / "ligature-cost.ndjson"
    harness.run([str(target / "release/ligature-cost")], raw, env)
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(rows) != 48000:
        raise SystemExit("Wrong sample denominator")
    groups = {}
    for case in [0, 1, 2]:
        for hot in [False, True]:
            for spare in [0, 4096]:
                for mode in ["ordinary", "noop", "count", "reject-last"]:
                    group = [r for r in rows if (r["case"], r["hot"], r["spare"], r["path"])
                             == (case, hot, spare, mode)]
                    if sorted(r["sample"] for r in group) != list(range(1000)):
                        raise SystemExit("Wrong sample identity")
                    if {r["rejected"] for r in group} != {mode == "reject-last"}:
                        raise SystemExit("Wrong rejection state")
                    costs = {(r["work"], r["allocation"], r["calls"], r["glyphs"]) for r in group}
                    if len(costs) != 1:
                        raise SystemExit("Unstable logical cost")
                    work, allocation, calls, glyphs = next(iter(costs))
                    if mode in ["count", "reject-last"]:
                        # Matching context is fresh per shape, even with a hot
                        # font lookup and spare glyph arrays.
                        if case == 1 and allocation < 9:
                            raise SystemExit("Fresh matching spill must be charged")
                        if case == 0 and hot and allocation != 0:
                            raise SystemExit("Inline matching and hot lookup must not allocate")
                    times = sorted(r["elapsed_ns"] / 1000 for r in group)
                    groups[f"{case}/{hot}/{spare}/{mode}"] = {
                        "samples": len(group), "median_us": times[499],
                        "p95_us": times[math.ceil(len(times) * .95) - 1],
                        "p99_us": times[math.ceil(len(times) * .99) - 1], "max_us": times[-1],
                        "work": work, "allocation": allocation, "calls": calls, "glyphs": glyphs}
                full, rejected = (groups[f"{case}/{hot}/{spare}/{mode}"]
                                  for mode in ["count", "reject-last"])
                if (full["calls"] != rejected["calls"] or
                        full["work"] - rejected["work"] != full["glyphs"] or
                        full["allocation"] != rejected["allocation"]):
                    raise SystemExit("Last rejection prefix differs")
    receipt = {
        "samples": len(rows), "samples_per_input_mode": 1000, "warmup_per_input_mode": 100,
        "characters": sorted({r["characters"] for r in rows}), "spare": [0, 4096],
        "cases": ["2component/coverage1/1subtable", "9component/coverage2/3subtables",
                  "ligature/multiple/ligature"],
        "configuration": "Native release, mode order rotates per sample",
        "timer": "shaping/cache/matching storage; data/plan/input/priming excluded; cleanup included",
        "system_load_controlled": False, "linker": args.linker,
        "raw_sha256": harness.sha(raw.read_bytes()), "source_sha256": harness.sha(source.read_bytes()),
        "fixture_sha256": harness.sha((source.parent / "ligature_fixtures.rs").read_bytes()),
        "production_admission_sha256": harness.sha((harness.PRODUCTION /
            "src/hb/shape_admission.rs").read_bytes()), "groups": groups}
    (output / "ligature-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
