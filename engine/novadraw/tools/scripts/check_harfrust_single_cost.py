#!/usr/bin/env python3
"""Sample single-substitution admission and verify every sample's logical fee."""
import argparse
import hashlib
import json
from pathlib import Path

import check_harfrust_buffer_admission as harness
import text_admission_build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="clang")
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    target = args.target_dir.resolve()
    env = text_admission_build.environment(target, args.linker)
    manifest = harness.REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    harness.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                 str(manifest), "--release", "--bin", "single-cost"],
                output / "single-cost-build.log", env)
    raw = output / "single-cost.ndjson"
    harness.run([str(target / "release/single-cost")], raw, env)
    # Keep all denominator, cold/hot storage and rejection checks in one place.
    harness.run(["python3", str(harness.REPO / "tools/scripts/summarize_harfrust_single_cost.py"),
                 "--output", str(output)], output / "single-cost-summary.log", env)
    receipt = json.loads((output / "single-cost-summary.json").read_text())
    receipt["linker"] = args.linker
    receipt["production_admission_sha256"] = hashlib.sha256((harness.PRODUCTION /
        "src/hb/shape_admission.rs").read_bytes()).hexdigest()
    (output / "single-cost-summary.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
