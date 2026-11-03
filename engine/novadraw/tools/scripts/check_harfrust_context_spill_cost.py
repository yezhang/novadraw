#!/usr/bin/env python3
"""Sample heap-backed contexts with storage and rejection assertions."""
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
    verified = json.loads((output / "context-spill-verification.json").read_text())
    for relative, checksum in verified["production_sources_sha256"].items():
        if harness.sha((harness.PRODUCTION / relative).read_bytes()) != checksum:
            raise SystemExit(f"Production changed: {relative}")
    source = harness.REPO / "tests/consumers/harfrust-admission/src/context_spill_cost.rs"
    original = harness.REPO / "target/text-harfrust-context-spill-build/lib-production/Cargo.toml"
    package = harness.REPO / "target/text-harfrust-context-spill-cost"
    harness.SOURCE = source
    harness.manifest(package, original.parent, harness.PRODUCTION / "Cargo.toml", binary=True)
    harness.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                 str(package / "Cargo.toml"), "--release", "--bin", "oracle"],
                output / "context-spill-cost-build.log", env)
    raw = output / "context-spill-cost.ndjson"
    harness.run([str(target / "release/oracle")], raw, env)
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(rows) != 32000:
        raise SystemExit("Wrong sample denominator")
    groups = {}
    for count in [9, 64]:
        for hot in [False, True]:
            for spare in [0, 4096]:
                for mode in ["ordinary", "noop", "count", "reject-last"]:
                    group = [r for r in rows if (r["count"], r["hot"], r["spare"], r["path"])
                             == (count, hot, spare, mode)]
                    if sorted(r["sample"] for r in group) != list(range(1000)):
                        raise SystemExit("Wrong sample identity")
                    if {r["rejected"] for r in group} != {mode == "reject-last"}:
                        raise SystemExit("Wrong rejection state")
                    costs = {(r["work"], r["allocation"], r["calls"], r["glyphs"]) for r in group}
                    if len(costs) != 1:
                        raise SystemExit("Unstable logical costs")
                    work, allocation, calls, glyphs = next(iter(costs))
                    if mode in ["count", "reject-last"] and hot and allocation != count * 2:
                        raise SystemExit("Matching/backup must allocate exactly two full arrays")
                    times = sorted(r["elapsed_ns"] / 1000 for r in group)
                    groups[f"{count}/{hot}/{spare}/{mode}"] = {
                        "samples": len(group), "median_us": times[499],
                        "p95_us": times[math.ceil(len(times) * .95) - 1],
                        "p99_us": times[math.ceil(len(times) * .99) - 1], "max_us": times[-1],
                        "work": work, "allocation": allocation, "calls": calls, "glyphs": glyphs}
                full, rejected = (groups[f"{count}/{hot}/{spare}/{mode}"]
                                  for mode in ["count", "reject-last"])
                if (full["calls"] != rejected["calls"] or
                        full["work"] - rejected["work"] != full["glyphs"] or
                        full["allocation"] != rejected["allocation"]):
                    raise SystemExit("Last rejection prefix differs")
    receipt = {
        "samples": len(rows), "samples_per_input_mode": 1000, "warmup_per_input_mode": 100,
        "characters": sorted({r["characters"] for r in rows}), "spare": [0, 4096],
        "cases": ["9input/5records/coverage2/3subtables", "64input/5records/coverage2/3subtables"],
        "configuration": "Native release, mode order rotates per sample",
        "timer": "shaping/cache/matching storage; data/plan/input/priming excluded; cleanup included",
        "system_load_controlled": False, "linker": args.linker,
        "raw_sha256": harness.sha(raw.read_bytes()), "source_sha256": harness.sha(source.read_bytes()),
        "fixture_sha256": harness.sha((source.parent / "context_fixtures.rs").read_bytes()),
        "production_sources_sha256": verified["production_sources_sha256"], "groups": groups}
    (output / "context-spill-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
