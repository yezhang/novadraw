#!/usr/bin/env python3
"""Compare original/production ordinary terminal b-miss in three sessions."""
import argparse
import json
import math
import shutil
from pathlib import Path
import check_harfrust_buffer_admission as h
from check_harfrust_context_admission import reconstruct
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
    root, original, _ = reconstruct(output, verified["prior_patches"], "text-harfrust-decomposition-miss-cost-")
    base = h.REPO / "target/text-harfrust-decomposition-miss-cost/build"
    env = text_admission_build.environment(target, args.linker)
    h.SOURCE = h.REPO / "tests/consumers/harfrust-admission/src/decomposition_miss_ordinary_cost.rs"
    binaries = {}
    for owner, library in [("original", original), ("production", h.PRODUCTION)]:
        lib = base / f"lib-{owner}"
        h.manifest(lib, library / "src/lib.rs", original / "Cargo.toml")
        package = base / f"cost-{owner}"
        h.manifest(package, lib, original / "Cargo.toml", binary=True)
        h.run(["cargo", "build", "--offline", "--manifest-path",
               str(package / "Cargo.toml"), "--release", "--bin", "oracle"],
              output / f"decomposition-miss-ordinary-{owner}-build.log", env)
        binary = root / f"cost-{owner}.bin"
        shutil.copy2(target / "release/oracle", binary)
        binaries[owner] = binary
    names = [f"{kind}-{case}-{direction}/{heat}" for kind in [0, 3, 6] for case in [4, 8]
             for direction in range(2) for heat in ["cold", "hot"]]
    groups = {}
    for session in range(3):
        for owner in (["original", "production"] if session % 2 == 0 else ["production", "original"]):
            log = output / f"decomposition-miss-ordinary-{owner}-{session}.ndjson"
            h.run([str(binaries[owner])], log, env)
            rows = [json.loads(line) for line in log.read_text().splitlines()]
            if len(rows) != 1000 * len(names) or {r["font"] for r in rows} != set(names):
                raise SystemExit("Wrong ordinary sample denominator")
            for name in names:
                selected = [r for r in rows if r["font"] == name]
                if sorted(r["sample"] for r in selected) != list(range(1000)):
                    raise SystemExit("Wrong ordinary sample identity")
                times = sorted(r["ns"] / 1000 for r in selected)
                groups[f"{owner}/{name}/{session}"] = {
                    "samples": len(times), "median_us": times[499],
                    "p95_us": times[math.ceil(len(times) * .95) - 1],
                    "p99_us": times[math.ceil(len(times) * .99) - 1], "max_us": times[-1],
                    "raw_sha256": h.sha(log.read_bytes())}
    receipt = {
        "samples": 6000 * len(names), "sessions": 3, "samples_per_input_and_session": 1000,
        "warmup_per_input_and_session": 100, "archive_sha256": h.ARCHIVE_SHA,
        "source_sha256": h.sha(h.SOURCE.read_bytes()),
        "input_sha256": h.sha((h.SOURCE.parent / "decomposition_miss_fixtures.rs").read_bytes()),
        "binary_sha256": {name: h.sha(path.read_bytes()) for name, path in binaries.items()},
        "input": "ABéÅÅǻCDE repeated 128", "characters": 1152,
        "order": "original/production, production/original, original/production",
        "configuration": "Native release; same source and features", "linker": args.linker,
        "timer": "ordinary shaping and ownership transfer; preparation, construction and successful drop excluded",
        "system_load_controlled": False,
        "production_sources_sha256": verified["production_sources_sha256"], "groups": groups,
    }
    (output / "decomposition-miss-ordinary-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
