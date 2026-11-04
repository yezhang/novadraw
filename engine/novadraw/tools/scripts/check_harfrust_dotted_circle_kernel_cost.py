#!/usr/bin/env python3
"""Sample the final circle kernel with frozen existing and new ordinary inputs."""
import argparse
import itertools
import json
import shutil
from pathlib import Path

import check_harfrust_buffer_admission as h
from check_harfrust_normalizer_kernel_cost import times
import text_admission_build

PREFIX = "dotted-circle-kernel"
MODES = {
    "admission": ("decomposition_miss_cost.rs", "decomposition_miss_fixtures.rs"),
    "ordinary": ("decomposition_miss_ordinary_cost.rs", "decomposition_miss_fixtures.rs"),
    "marks": ("normalizer_kernel_ordinary_cost.rs", "normalizer_kernel_fixtures.rs"),
    "circle": ("dotted_circle_kernel_ordinary_cost.rs", "dotted_circle_kernel_fixtures.rs"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output, target = h.artifact_output(args.output), args.target_dir.resolve()
    verified = json.loads((output / f"{PREFIX}-verification.json").read_text())
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    for mode, old_prefix in [("admission", "cost"), ("ordinary", "ordinary-cost"),
                             ("marks", "marks-cost")]:
        prior = json.loads((output / f"normalizer-kernel-{old_prefix}-summary.json").read_text())
        source, fixture = MODES[mode]
        field = "input_sha256" if mode == "ordinary" else "fixture_sha256"
        if (h.sha((src / source).read_bytes()) != prior["source_sha256"] or
                h.sha((src / fixture).read_bytes()) != prior[field]):
            raise SystemExit(f"Frozen cost input changed: {mode}")
    for name, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / name).read_bytes()) != checksum:
            raise SystemExit(f"Production source changed: {name}")
    env = text_admission_build.environment(target, args.linker)
    build = h.REPO / f"target/text-harfrust-{PREFIX}-build"
    base = h.REPO / f"target/text-harfrust-{PREFIX}-cost"
    base.mkdir(exist_ok=True)
    packages, binaries, lint = {}, {}, {}
    for mode, (source, _) in MODES.items():
        for owner in (["production"] if mode == "admission" else ["original", "production"]):
            key = f"{mode}-{owner}"
            package = base / key
            h.SOURCE = src / source
            h.manifest(package, build / f"lib-{owner}", h.PRODUCTION / "Cargo.toml", binary=True)
            packages[key] = package
            log = output / f"{PREFIX}-{key}-cost-clippy.log"
            h.run(["cargo", "clippy", "--offline", "--manifest-path",
                   str(package / "Cargo.toml"), "--bin", "oracle", "--", "-D", "warnings"], log, env)
            lint[key] = h.sha(log.read_bytes())
    # All seven cost programs are checked before the first timed sample.
    for key, package in packages.items():
        h.run(["cargo", "build", "--offline", "--manifest-path",
               str(package / "Cargo.toml"), "--release", "--bin", "oracle"],
              output / f"{PREFIX}-{key}-cost-build.log", env)
        binary = base / f"{key}.bin"
        shutil.copy2(target / "release/oracle", binary)
        binaries[key] = binary
    raw = output / f"{PREFIX}-admission-cost.ndjson"
    h.run([str(binaries["admission-production"])], raw, env)
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(rows) != 192000:
        raise SystemExit("Wrong admission sample denominator")
    groups = {}
    for kind, case, direction, hot, spare in itertools.product(
        [0, 3, 6], [4, 8], range(2), [False, True], [0, 16384],
    ):
        for path in ["ordinary", "noop", "count", "reject-last"]:
            selected = [r for r in rows if
                        (r["kind"], r["case"], r["direction"], r["hot"], r["spare"], r["path"])
                        == (kind, case, direction, hot, spare, path)]
            if sorted(r["sample"] for r in selected) != list(range(1000)):
                raise SystemExit("Wrong admission sample identities")
            if {r["rejected"] for r in selected} != {path == "reject-last"}:
                raise SystemExit("Wrong admission rejection state")
            fields = ["work", "allocation", "calls", "glyphs", "mapping_work", "unicode_work"]
            costs = {tuple(r[field] for field in fields) for r in selected}
            if len(costs) != 1:
                raise SystemExit("Unstable admission operation costs")
            cost = dict(zip(fields, next(iter(costs))))
            if cost["glyphs"] == 0 or (path in ["count", "reject-last"] and
                    (cost["mapping_work"] == 0 or cost["unicode_work"] == 0)):
                raise SystemExit("Missing output/query operation costs")
            groups[f"{kind}/{case}/{direction}/{hot}/{spare}/{path}"] = {
                **times(selected, "elapsed_ns"), **cost,
            }
        full, rejected = (groups[f"{kind}/{case}/{direction}/{hot}/{spare}/{path}"]
                          for path in ["count", "reject-last"])
        if (full["calls"] != rejected["calls"] or
                full["work"] - rejected["work"] != full["glyphs"] or
                any(full[field] != rejected[field] for field in
                    ["allocation", "mapping_work", "unicode_work"])):
            raise SystemExit("Last rejection prefix differs")
    source, fixture = MODES["admission"]
    summary = {
        "samples": len(rows), "groups": groups, "raw_sha256": h.sha(raw.read_bytes()),
        "source_sha256": h.sha((src / source).read_bytes()),
        "fixture_sha256": h.sha((src / fixture).read_bytes()),
        "production_sources_sha256": verified["production_sources_sha256"],
        "clippy_before_sampling": lint, "public_marks_admitted": False,
        "configuration": "Native release; frozen singleton admitted/noop/count/reject-last inputs",
        "system_load_controlled": False, "linker": args.linker,
    }
    (output / f"{PREFIX}-admission-cost-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    sets = {
        "ordinary": ([f"{kind}-{case}-{direction}/{heat}" for kind in [0, 3, 6]
                      for case in [4, 8] for direction in range(2) for heat in ["cold", "hot"]],
                     1000, "frozen singleton normalization", 1280),
        "marks": ([f"{kind}-{case}-{mapping}-{direction}/{heat}" for kind in [0, 3, 6]
                   for case in [0, 8] for mapping in [0, 1] for direction in range(2)
                   for heat in ["cold", "hot"]],
                  500, "frozen raw marks; precomposed mapping retained", 1280),
        "circle": ([f"{kind}-{case}-{mapping}-{str(circle).lower()}-{direction}/{heat}" for kind in [0, 3, 6]
                    for case in [0, 8] for mapping in [0, 2] for circle in [False, True]
                    for direction in range(2) for heat in ["cold", "hot"]],
                   500, "raw marks; circle mapped/missing; precomposed present/missing growth", 1408),
    }
    for mode, (names, count, description, chars) in sets.items():
        groups = {}
        for session in range(3):
            owners = ["original", "production"] if session % 2 == 0 else ["production", "original"]
            for owner in owners:
                log = output / f"{PREFIX}-{mode}-{owner}-{session}.ndjson"
                h.run([str(binaries[f"{mode}-{owner}"])], log, env)
                rows = [json.loads(line) for line in log.read_text().splitlines()]
                if len(rows) != count * len(names) or {r["font"] for r in rows} != set(names):
                    raise SystemExit(f"Wrong ordinary denominator: {mode}")
                for name in names:
                    selected = [r for r in rows if r["font"] == name]
                    if sorted(r["sample"] for r in selected) != list(range(count)):
                        raise SystemExit(f"Wrong ordinary identities: {mode}/{name}")
                    groups[f"{owner}/{name}/{session}"] = {
                        **times(selected, "ns"), "raw_sha256": h.sha(log.read_bytes()),
                    }
        source, fixture = MODES[mode]
        summary = {
            "samples": 6 * count * len(names), "groups": groups, "sessions": 3,
            "samples_per_input_and_session": count, "warmup_per_input_and_session": 100,
            "characters": chars, "input": description,
            "source_sha256": h.sha((src / source).read_bytes()),
            "fixture_sha256": h.sha((src / fixture).read_bytes()),
            "production_sources_sha256": verified["production_sources_sha256"],
            "clippy_before_sampling": lint, "archive_sha256": h.ARCHIVE_SHA,
            "binary_sha256": {owner: h.sha(binaries[f"{mode}-{owner}"].read_bytes())
                              for owner in ["original", "production"]},
            "configuration": "Native release; preparation/construction/drop excluded",
            "order": "original/production, production/original, original/production",
            "system_load_controlled": False, "public_marks_admitted": False,
            "linker": args.linker,
        }
        (output / f"{PREFIX}-{mode}-cost-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    for name, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / name).read_bytes()) != checksum:
            raise SystemExit(f"Production changed during sampling: {name}")
    print("PASS: 192000 admission, 144000 singleton, 144000 marks, 288000 circle samples")


if __name__ == "__main__":
    main()
