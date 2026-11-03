#!/usr/bin/env python3
"""Sample PairPos work, whole-capacity allocation and final rejection."""
import argparse
import json
import math
from pathlib import Path

import check_harfrust_buffer_admission as h
import text_admission_build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output, target = h.artifact_output(args.output), args.target_dir.resolve()
    verified = json.loads((output / "gpos-pair-verification.json").read_text())
    for relative, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / relative).read_bytes()) != checksum:
            raise SystemExit(f"Production changed: {relative}")
    source = h.REPO / "tests/consumers/harfrust-admission/src/gpos_pair_cost.rs"
    library = h.REPO / "target/text-harfrust-gpos-pair-build/lib-production"
    package = h.REPO / "target/text-harfrust-gpos-pair-cost"
    h.SOURCE = source
    h.manifest(package, library, h.PRODUCTION / "Cargo.toml", binary=True)
    env = text_admission_build.environment(target, args.linker)
    h.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
        str(package / "Cargo.toml"), "--release", "--bin", "oracle"], output / "gpos-pair-cost-build.log", env)
    raw = output / "gpos-pair-cost.ndjson"
    h.run([str(target / "release/oracle")], raw, env)
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(rows) != 64000:
        raise SystemExit("Wrong sample denominator")
    groups = {}
    for format_ in [1, 2]:
        for substitutions in [False, True]:
            for hot in [False, True]:
                for spare in [0, 16384]:
                    for mode in ["ordinary", "noop", "count", "reject-last"]:
                        group = [r for r in rows if (r["format"], r["substitutions"], r["hot"], r["spare"], r["path"])
                            == (format_, substitutions, hot, spare, mode)]
                        if sorted(r["sample"] for r in group) != list(range(1000)):
                            raise SystemExit("Wrong sample identities")
                        if {r["rejected"] for r in group} != {mode == "reject-last"}:
                            raise SystemExit("Wrong rejection state")
                        costs = {(r["work"], r["allocation"], r["calls"], r["glyphs"]) for r in group}
                        if len(costs) != 1:
                            raise SystemExit("Unstable costs")
                        work, allocation, calls, glyphs = next(iter(costs))
                        if glyphs != 3072 * (2 if substitutions else 1):
                            raise SystemExit("Wrong GSUB output")
                        if mode in ["count", "reject-last"] and hot and (not substitutions or spare > 0) and allocation != 0:
                            raise SystemExit("PairPos cannot allocate")
                        times = sorted(r["elapsed_ns"] / 1000 for r in group)
                        groups[f"{format_}/{substitutions}/{hot}/{spare}/{mode}"] = {
                            "samples": len(group), "median_us": times[499],
                            "p95_us": times[math.ceil(len(times) * .95) - 1],
                            "p99_us": times[math.ceil(len(times) * .99) - 1], "max_us": times[-1],
                            "work": work, "allocation": allocation, "calls": calls, "glyphs": glyphs}
                    full, rejected = (groups[f"{format_}/{substitutions}/{hot}/{spare}/{mode}"]
                        for mode in ["count", "reject-last"])
                    if (full["calls"] != rejected["calls"] or full["work"] - rejected["work"] != full["glyphs"]
                            or full["allocation"] != rejected["allocation"]):
                        raise SystemExit("Last rejection prefix differs")
    receipt = {
        "samples": len(rows), "samples_per_input_mode": 1000, "warmup_per_input_mode": 100,
        "characters": 3072, "spare": [0, 16384],
        "cases": "PairPos1/2, coverage2, 3 subtables/2 lookups, scalar1/2=15; optional prior MultipleSubst",
        "configuration": "Native release; mode order rotates per sample; unsafe concat enabled",
        "timer": "shaping/cache/reservation plus result or rejected cleanup; data/plan/input/priming excluded",
        "system_load_controlled": False, "linker": args.linker,
        "raw_sha256": h.sha(raw.read_bytes()), "source_sha256": h.sha(source.read_bytes()),
        "fixture_sha256": h.sha((source.parent / "gpos_pair_fixtures.rs").read_bytes()),
        "production_sources_sha256": verified["production_sources_sha256"], "groups": groups,
    }
    (output / "gpos-pair-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
