#!/usr/bin/env python3
"""Sample property queries, whole capacity and final rejection with GDEF."""
import argparse
import itertools
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
    verified = json.loads((output / "gdef-verification.json").read_text())
    for relative, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / relative).read_bytes()) != checksum:
            raise SystemExit(f"Production changed: {relative}")
    source = h.REPO / "tests/consumers/harfrust-admission/src/gdef_cost.rs"
    h.SOURCE = source
    package = h.REPO / "target/text-harfrust-gdef-cost"
    h.manifest(package, h.REPO / "target/text-harfrust-gdef-build/lib-production",
        h.PRODUCTION / "Cargo.toml", binary=True)
    env = text_admission_build.environment(target, args.linker)
    h.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
        str(package / "Cargo.toml"), "--release", "--bin", "oracle"], output / "gdef-cost-build.log", env)
    raw = output / "gdef-cost.ndjson"
    h.run([str(target / "release/oracle")], raw, env)
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(rows) != 192000:
        raise SystemExit("Wrong sample denominator")
    groups = {}
    modes = ["ordinary", "noop", "count", "reject-last"]
    for kind, case, direction, hot, spare in itertools.product([0, 3, 6], [4, 8], range(2), [False, True], [0, 16384]):
        for mode in modes:
            group = [r for r in rows if (r["kind"], r["case"], r["direction"], r["hot"], r["spare"], r["path"])
                == (kind, case, direction, hot, spare, mode)]
            if sorted(r["sample"] for r in group) != list(range(1000)):
                raise SystemExit("Wrong sample identities")
            if {r["rejected"] for r in group} != {mode == "reject-last"}:
                raise SystemExit("Wrong rejection state")
            costs = {(r["work"], r["allocation"], r["calls"], r["glyphs"], r["property_work"]) for r in group}
            if len(costs) != 1:
                raise SystemExit("Unstable costs")
            work, allocation, calls, glyphs, property_work = next(iter(costs))
            if glyphs != (1536 if kind == 6 else 3072):
                raise SystemExit("Wrong substitution output")
            if mode in ["count", "reject-last"] and property_work < 1536 * 16:
                raise SystemExit("Missing property query work")
            times = sorted(r["elapsed_ns"] / 1000 for r in group)
            groups[f"{kind}/{case}/{direction}/{hot}/{spare}/{mode}"] = {
                "samples": len(group), "median_us": times[499],
                "p95_us": times[math.ceil(len(times) * .95) - 1],
                "p99_us": times[math.ceil(len(times) * .99) - 1], "max_us": times[-1],
                "work": work, "allocation": allocation, "calls": calls, "glyphs": glyphs,
                "property_work": property_work}
        full, rejected = (groups[f"{kind}/{case}/{direction}/{hot}/{spare}/{mode}"] for mode in ["count", "reject-last"])
        if (full["calls"] != rejected["calls"] or full["work"] - rejected["work"] != full["glyphs"]
                or full["allocation"] != rejected["allocation"] or full["property_work"] != rejected["property_work"]):
            raise SystemExit("Last rejection prefix differs")
    receipt = {
        "samples": len(rows), "samples_per_input_mode": 1000, "warmup_per_input_mode": 100,
        "characters": 1536, "input": "ABC repeated 512; all GDEF class 3",
        "cases": "ClassDef1/2 with attachment classes; no GPOS/PairPos2/Context3; both horizontal directions; absent script; Graphemes",
        "configuration": "Native release; mode order rotates per sample; unsafe concat enabled",
        "timer": "shaping/cache/reservation plus result or rejected cleanup; data/plan/input/priming excluded",
        "system_load_controlled": False, "linker": args.linker,
        "raw_sha256": h.sha(raw.read_bytes()), "source_sha256": h.sha(source.read_bytes()),
        "fixture_sha256": h.sha((source.parent / "gdef_fixtures.rs").read_bytes()),
        "production_sources_sha256": verified["production_sources_sha256"], "groups": groups,
    }
    (output / "gdef-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
