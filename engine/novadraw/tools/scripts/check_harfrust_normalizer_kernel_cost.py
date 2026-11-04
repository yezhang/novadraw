#!/usr/bin/env python3
"""Sample shared normalization using immutable preceding cost inputs."""
import argparse
import itertools
import json
import math
import shutil
from pathlib import Path

import check_harfrust_buffer_admission as h
import text_admission_build


def times(rows, field):
    values = sorted(r[field] / 1000 for r in rows)
    return {
        "samples": len(values), "median_us": values[(len(values) - 1) // 2],
        "p95_us": values[math.ceil(len(values) * .95) - 1],
        "p99_us": values[math.ceil(len(values) * .99) - 1], "max_us": values[-1],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output, target = h.artifact_output(args.output), args.target_dir.resolve()
    verified = json.loads((output / "normalizer-kernel-verification.json").read_text())
    prior = json.loads((output / "decomposition-miss-cost-summary.json").read_text())
    prior_ordinary = json.loads((output / "decomposition-miss-ordinary-cost-summary.json").read_text())
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    for name, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / name).read_bytes()) != checksum:
            raise SystemExit(f"Source changed: {name}")
    for filename, checksum in [
        ("decomposition_miss_cost.rs", prior["source_sha256"]),
        ("decomposition_miss_ordinary_cost.rs", prior_ordinary["source_sha256"]),
        ("decomposition_miss_fixtures.rs", prior["fixture_sha256"]),
    ]:
        if h.sha((src / filename).read_bytes()) != checksum:
            raise SystemExit(f"Frozen cost input changed: {filename}")
    env = text_admission_build.environment(target, args.linker)
    build = h.REPO / "target/text-harfrust-normalizer-kernel-build"
    base = h.REPO / "target/text-harfrust-normalizer-kernel-cost"
    base.mkdir(exist_ok=True)
    packages, binaries, lint = {}, {}, {}
    for mode, owner, source in [
        ("admission", "production", "decomposition_miss_cost.rs"),
        ("ordinary", "original", "decomposition_miss_ordinary_cost.rs"),
        ("ordinary", "production", "decomposition_miss_ordinary_cost.rs"),
        ("marks", "original", "normalizer_kernel_ordinary_cost.rs"),
        ("marks", "production", "normalizer_kernel_ordinary_cost.rs"),
    ]:
        key = f"{mode}-{owner}"
        h.SOURCE = src / source
        package = base / key
        h.manifest(package, build / f"lib-{owner}", h.PRODUCTION / "Cargo.toml", binary=True)
        packages[key] = package
        log = output / f"normalizer-kernel-{key}-cost-clippy.log"
        h.run(["cargo", "clippy", "--offline", "--manifest-path",
               str(package / "Cargo.toml"), "--bin", "oracle", "--", "-D", "warnings"], log, env)
        lint[key] = h.sha(log.read_bytes())
    # All five cost binaries pass Clippy before the first timed sample.
    for key, package in packages.items():
        h.run(["cargo", "build", "--offline", "--manifest-path",
               str(package / "Cargo.toml"), "--release", "--bin", "oracle"],
              output / f"normalizer-kernel-{key}-cost-build.log", env)
        binary = base / f"{key}.bin"
        shutil.copy2(target / "release/oracle", binary)
        binaries[key] = binary
    raw = output / "normalizer-kernel-cost.ndjson"
    h.run([str(binaries["admission-production"])], raw, env)
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(rows) != 192000:
        raise SystemExit("Wrong admission sample denominator")
    groups = {}
    for kind, case, direction, hot, spare in itertools.product(
        [0, 3, 6], [4, 8], range(2), [False, True], [0, 16384],
    ):
        for mode in ["ordinary", "noop", "count", "reject-last"]:
            selected = [r for r in rows if
                (r["kind"], r["case"], r["direction"], r["hot"], r["spare"], r["path"])
                == (kind, case, direction, hot, spare, mode)]
            if sorted(r["sample"] for r in selected) != list(range(1000)):
                raise SystemExit("Wrong admission sample identities")
            if {r["rejected"] for r in selected} != {mode == "reject-last"}:
                raise SystemExit("Wrong rejection state")
            fields = ["work", "allocation", "calls", "glyphs", "mapping_work", "unicode_work"]
            costs = {tuple(r[f] for f in fields) for r in selected}
            if len(costs) != 1:
                raise SystemExit("Unstable operation costs")
            cost = dict(zip(fields, next(iter(costs))))
            if cost["glyphs"] == 0 or (mode in ["count", "reject-last"] and
                    (cost["mapping_work"] == 0 or cost["unicode_work"] == 0)):
                raise SystemExit("Missing output/query costs")
            groups[f"{kind}/{case}/{direction}/{hot}/{spare}/{mode}"] = {
                **times(selected, "elapsed_ns"), **cost,
            }
        full, rejected = (groups[f"{kind}/{case}/{direction}/{hot}/{spare}/{mode}"]
                          for mode in ["count", "reject-last"])
        if (full["calls"] != rejected["calls"] or
                full["work"] - rejected["work"] != full["glyphs"] or
                any(full[f] != rejected[f] for f in ["allocation", "mapping_work", "unicode_work"])):
            raise SystemExit("Last rejection prefix differs")
    summary = {
        **{k: v for k, v in prior.items() if k not in [
            "groups", "raw_sha256", "production_sources_sha256",
        ]},
        "samples": len(rows), "raw_sha256": h.sha(raw.read_bytes()), "groups": groups,
        "production_sources_sha256": verified["production_sources_sha256"],
        "clippy_before_sampling": lint,
        "public_marks_sampled": False,
    }
    (output / "normalizer-kernel-cost-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    names = [f"{kind}-{case}-{direction}/{heat}" for kind in [0, 3, 6] for case in [4, 8]
             for direction in range(2) for heat in ["cold", "hot"]]
    ordinary = {}
    for session in range(3):
        for owner in (["original", "production"] if session % 2 == 0 else ["production", "original"]):
            log = output / f"normalizer-kernel-ordinary-{owner}-{session}.ndjson"
            h.run([str(binaries[f"ordinary-{owner}"])], log, env)
            rows = [json.loads(line) for line in log.read_text().splitlines()]
            if len(rows) != 1000 * len(names) or {r["font"] for r in rows} != set(names):
                raise SystemExit("Wrong ordinary sample denominator")
            for name in names:
                selected = [r for r in rows if r["font"] == name]
                if sorted(r["sample"] for r in selected) != list(range(1000)):
                    raise SystemExit("Wrong ordinary sample identities")
                ordinary[f"{owner}/{name}/{session}"] = {
                    **times(selected, "ns"), "raw_sha256": h.sha(log.read_bytes()),
                }
    summary = {
        **{k: v for k, v in prior_ordinary.items() if k not in [
            "groups", "production_sources_sha256", "binary_sha256",
        ]},
        "production_sources_sha256": verified["production_sources_sha256"],
        "binary_sha256": {owner: h.sha(binaries[f"ordinary-{owner}"].read_bytes())
                          for owner in ["original", "production"]},
        "groups": ordinary, "clippy_before_sampling": lint,
        "public_marks_sampled": False,
    }
    for name, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / name).read_bytes()) != checksum:
            raise SystemExit(f"Source changed during sampling: {name}")
    (output / "normalizer-kernel-ordinary-cost-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    names = [f"{kind}-{case}-{mapping}-{direction}/{heat}"
             for kind in [0, 3, 6] for case in [0, 8] for mapping in [0, 1]
             for direction in range(2) for heat in ["cold", "hot"]]
    groups = {}
    for session in range(3):
        for owner in (["original", "production"] if session % 2 == 0 else ["production", "original"]):
            log = output / f"normalizer-kernel-marks-{owner}-{session}.ndjson"
            h.run([str(binaries[f"marks-{owner}"])], log, env)
            rows = [json.loads(line) for line in log.read_text().splitlines()]
            if len(rows) != 500 * len(names) or {r["font"] for r in rows} != set(names):
                raise SystemExit("Wrong marks sample denominator")
            for name in names:
                selected = [r for r in rows if r["font"] == name]
                if sorted(r["sample"] for r in selected) != list(range(500)):
                    raise SystemExit("Wrong marks sample identities")
                groups[f"{owner}/{name}/{session}"] = {
                    **times(selected, "ns"), "raw_sha256": h.sha(log.read_bytes()),
                }
    source = src / "normalizer_kernel_ordinary_cost.rs"
    summary = {
        "samples": 6 * 500 * len(names), "sessions": 3, "samples_per_input_and_session": 500,
        "warmup_per_input_and_session": 100, "characters": 1280,
        "input": "ÅÅéǻA\\u{30a}a\\u{315}\\u{300}\\u{316} repeated 128",
        "configuration": "Native release; None script; Graphemes; zero spare; same source/features",
        "cases": "nullable glyph ClassDef/Format2 mark classes; no GPOS/PairPos2/Context3; marks mapping present/absent; LTR/RTL; cold/hot",
        "timer": "ordinary shaping and ownership transfer; preparation/construction/drop excluded",
        "order": "original/production, production/original, original/production",
        "system_load_controlled": False, "linker": args.linker,
        "source_sha256": h.sha(source.read_bytes()),
        "fixture_sha256": h.sha((src / "normalizer_kernel_fixtures.rs").read_bytes()),
        "archive_sha256": h.ARCHIVE_SHA,
        "production_sources_sha256": verified["production_sources_sha256"],
        "clippy_before_sampling": lint, "groups": groups,
        "binary_sha256": {owner: h.sha(binaries[f"marks-{owner}"].read_bytes())
                          for owner in ["original", "production"]},
        "public_marks_admitted": False,
    }
    for name, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / name).read_bytes()) != checksum:
            raise SystemExit(f"Source changed during marks sampling: {name}")
    (output / "normalizer-kernel-marks-cost-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("PASS: 192000 admission, 144000 ordinary singleton and 144000 ordinary marks samples")


if __name__ == "__main__":
    main()
