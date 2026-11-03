#!/usr/bin/env python3
"""Sample the same ordinary plan API against original and production."""
import argparse
import json
import math
from pathlib import Path
import shutil
import tarfile
import tempfile

import check_harfrust_buffer_admission as harness
import text_admission_build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="clang")
    parser.add_argument("--input-kind", choices=["plan-layout", "private-language", "general-language",
                                               "script-callbacks", "shape", "single", "multiple", "ligature", "context", "context-spill", "gpos-single", "gpos-pair", "rtl", "simple-unicode", "gdef", "general-scalar", "normalization"],
                        default="plan-layout")
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Archive checksum mismatch")
    kind = args.input_kind
    base = harness.REPO / f"target/text-harfrust-{kind}-cost"
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="run-", dir=base))
    original = root / "original"
    original.mkdir()
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            if not member.isfile():
                continue
            relative = Path(member.name).relative_to("harfrust-0.3.2")
            if relative.is_absolute() or ".." in relative.parts:
                raise SystemExit("Unexpected archive member")
            dest = original / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(source.extractfile(member).read())
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    harness.SOURCE = harness.REPO / (
        f"tests/consumers/harfrust-admission/src/{kind.replace('-', '_')}_ordinary_cost.rs"
    )
    binaries = {}
    build_root = base / "build"
    build_root.mkdir(exist_ok=True)
    for name, library in [("original", original), ("production", harness.PRODUCTION)]:
        lib = build_root / f"lib-{name}"
        harness.manifest(lib, library / "src/lib.rs", original / "Cargo.toml")
        package = build_root / f"cost-{name}"
        harness.manifest(package, lib, original / "Cargo.toml", binary=True)
        harness.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                     str(package / "Cargo.toml"), "--release", "--bin", "oracle"],
                    output / f"{kind}-ordinary-{name}-build.log", env)
        binary = root / f"cost-{name}.bin"
        shutil.copy2(args.target_dir.resolve() / "release/oracle", binary)
        binaries[name] = binary
    summary = {}
    names = ([f"{layout}-{case}-{direction}/{heat}" for layout in [0, 3, 6] for case in [4, 8]
              for direction in range(2) for heat in ["cold", "hot"]] if kind in ["gdef", "general-scalar", "normalization"] else
             [f"{cmap}-{script}-{case}/{heat}" for cmap in range(3) for script in ["native", "absent"]
              for case in ["simple", "staged"] for heat in ["cold", "hot"]] if kind == "simple-unicode" else
             [f"{script}-{case}/{heat}" for script in ["native", "absent"] for case in ["simple", "staged"]
              for heat in ["cold", "hot"]] if kind == "rtl" else
             [f"pair{format}-{case}/{heat}" for format in [1, 2] for case in ["simple", "staged"]
              for heat in ["cold", "hot"]] if kind == "gpos-pair" else
             ["struct_gsub", "struct_gpos", "inter_variable", "cjk_default"]
             if kind == "plan-layout" else ["arabic", "syriac", "hangul"]
             if kind == "script-callbacks" else ["inter/default", "inter/spare", "cjk/default", "cjk/spare"]
             if kind == "shape" else ["simple/cold", "simple/hot", "staged/cold", "staged/hot",
                                     "mixed/cold", "mixed/hot"]
             if kind == "ligature" else ["simple/cold", "simple/hot", "staged/cold", "staged/hot"]
             if kind in ["single", "multiple", "context", "context-spill", "gpos-single"] else ["inline", "spill", "long"])
    for session in range(3):
        for name in (["original", "production"] if session % 2 == 0 else ["production", "original"]):
            log = output / f"{kind}-ordinary-{name}-{session}.ndjson"
            harness.run([str(binaries[name])], log, env)
            rows = [json.loads(line) for line in log.read_text().splitlines()]
            if len(rows) != 1000 * len(names) or {row["font"] for row in rows} != set(names):
                raise SystemExit("Wrong ordinary cost denominator")
            for font in names:
                group = [row for row in rows if row["font"] == font]
                if sorted(row["sample"] for row in group) != list(range(1000)):
                    raise SystemExit("Wrong ordinary sample identity")
                times = sorted(row["ns"] / 1000 for row in group)
                summary[f"{name}/{font}/{session}"] = {
                    "samples": len(times), "median_us": times[499],
                    "p95_us": times[math.ceil(len(times) * .95) - 1],
                    "p99_us": times[math.ceil(len(times) * .99) - 1], "max_us": times[-1],
                    "raw_sha256": harness.sha(log.read_bytes()),
                }
    receipt = {
        "samples": 6000 * len(names), "sessions": 3, "samples_per_input_and_session": 1000,
        "warmup_per_input_and_session": 100,
        "archive_sha256": harness.ARCHIVE_SHA,
        "source_sha256": harness.sha(harness.SOURCE.read_bytes()),
        "input_sha256": harness.sha((
            harness.SOURCE.parent / "plan_layout_fixtures.rs" if kind == "plan-layout"
            else harness.SOURCE.parent / "script_callbacks_fixtures.rs" if kind == "script-callbacks"
            else harness.SOURCE.parent / "shape_fixtures.rs" if kind == "shape"
            else harness.SOURCE.parent / "single_fixtures.rs" if kind == "single"
            else harness.SOURCE.parent / "multiple_fixtures.rs" if kind == "multiple"
            else harness.SOURCE.parent / "ligature_fixtures.rs" if kind == "ligature"
            else harness.SOURCE.parent / "context_fixtures.rs" if kind in ["context", "context-spill"]
            else harness.SOURCE.parent / "gpos_single_fixtures.rs" if kind == "gpos-single"
            else harness.SOURCE.parent / "gpos_pair_fixtures.rs" if kind == "gpos-pair"
            else harness.SOURCE.parent / "rtl_fixtures.rs" if kind == "rtl"
            else harness.SOURCE.parent / "simple_unicode_fixtures.rs" if kind == "simple-unicode"
            else harness.SOURCE.parent / "gdef_fixtures.rs" if kind == "gdef"
            else harness.SOURCE.parent / "general_scalar_fixtures.rs" if kind == "general-scalar"
            else harness.SOURCE.parent / "normalization_fixtures.rs" if kind == "normalization"
            else harness.REPO / "assets/fonts/InterVariable.ttf"
        ).read_bytes()),
        "binary_sha256": {name: harness.sha(path.read_bytes()) for name, path in binaries.items()},
        "order": "original/production, production/original, original/production",
        "configuration": "Native release; same source and features",
        "linker": args.linker,
        "timer": ("ordinary nonempty shaping and buffer ownership transfer; input preparation, "
                  "plan/data/shaper construction and successful drop excluded" if kind in ["shape", "single", "multiple", "ligature", "context", "context-spill", "gpos-single", "gpos-pair", "rtl", "simple-unicode", "gdef", "general-scalar", "normalization"] else
                  "fresh ordinary plan; language, data, instance, shaper and successful drop excluded"),
        "system_load_controlled": False, "groups": summary,
    }
    (output / f"{kind}-ordinary-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
