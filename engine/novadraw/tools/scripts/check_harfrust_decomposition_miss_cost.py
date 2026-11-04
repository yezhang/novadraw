#!/usr/bin/env python3
"""Sample terminal b-miss and exact final-rejection cost prefixes."""
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
    verified = json.loads((output / "decomposition-miss-verification.json").read_text())
    for relative, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / relative).read_bytes()) != checksum:
            raise SystemExit(f"Production changed: {relative}")
    source = h.REPO / "tests/consumers/harfrust-admission/src/decomposition_miss_cost.rs"
    h.SOURCE = source
    package = h.REPO / "target/text-harfrust-decomposition-miss-cost"
    h.manifest(package, h.REPO / "target/text-harfrust-decomposition-miss-build/lib-production",
               h.PRODUCTION / "Cargo.toml", binary=True)
    env = text_admission_build.environment(target, args.linker)
    h.run(["cargo", "build", "--offline", "--manifest-path",
           str(package / "Cargo.toml"), "--release", "--bin", "oracle"],
          output / "decomposition-miss-cost-build.log", env)
    raw = output / "decomposition-miss-cost.ndjson"
    h.run([str(target / "release/oracle")], raw, env)
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(rows) != 192000:
        raise SystemExit("Wrong sample denominator")
    groups = {}
    for kind, case, direction, hot, spare in itertools.product(
            [0, 3, 6], [4, 8], range(2), [False, True], [0, 16384]):
        for mode in ["ordinary", "noop", "count", "reject-last"]:
            selected = [r for r in rows if
                (r["kind"], r["case"], r["direction"], r["hot"], r["spare"], r["path"])
                == (kind, case, direction, hot, spare, mode)]
            if sorted(r["sample"] for r in selected) != list(range(1000)):
                raise SystemExit("Wrong sample identities")
            if {r["rejected"] for r in selected} != {mode == "reject-last"}:
                raise SystemExit("Wrong rejection state")
            fields = ["work", "allocation", "calls", "glyphs", "mapping_work", "unicode_work"]
            costs = {tuple(r[f] for f in fields) for r in selected}
            if len(costs) != 1:
                raise SystemExit("Unstable costs")
            cost = dict(zip(fields, next(iter(costs))))
            if cost["glyphs"] == 0 or (mode in ["count", "reject-last"] and
                    (cost["mapping_work"] == 0 or cost["unicode_work"] == 0)):
                raise SystemExit("Missing output or terminal query cost")
            times = sorted(r["elapsed_ns"] / 1000 for r in selected)
            groups[f"{kind}/{case}/{direction}/{hot}/{spare}/{mode}"] = {
                "samples": len(selected), "median_us": times[499],
                "p95_us": times[math.ceil(len(times) * .95) - 1],
                "p99_us": times[math.ceil(len(times) * .99) - 1], "max_us": times[-1], **cost}
        full, rejected = (groups[f"{kind}/{case}/{direction}/{hot}/{spare}/{mode}"]
                          for mode in ["count", "reject-last"])
        if (full["calls"] != rejected["calls"] or
                full["work"] - rejected["work"] != full["glyphs"] or
                any(full[f] != rejected[f] for f in ["allocation", "mapping_work", "unicode_work"])):
            raise SystemExit("Last rejection prefix differs")
    receipt = {
        "samples": len(rows), "samples_per_input_mode": 1000, "warmup_per_input_mode": 100,
        "characters": 1152, "input": "ABéÅÅǻCDE repeated 128",
        "cases": "ClassDef1/2; no GPOS/PairPos2/Context3; LTR/RTL; absent script; Graphemes; spare0/16384; map0",
        "configuration": "Native release; mode order rotates per sample; unsafe concat enabled",
        "timer": "shaping plus successful drop or rejected cleanup; preparation/priming and retained buffer drop excluded",
        "system_load_controlled": False, "linker": args.linker,
        "raw_sha256": h.sha(raw.read_bytes()), "source_sha256": h.sha(source.read_bytes()),
        "fixture_sha256": h.sha((source.parent / "decomposition_miss_fixtures.rs").read_bytes()),
        "production_sources_sha256": verified["production_sources_sha256"], "groups": groups,
    }
    (output / "decomposition-miss-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
