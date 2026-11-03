#!/usr/bin/env python3
"""Re-sample unchanged four cost workloads against the grapheme kernel."""
import argparse
import json
from pathlib import Path

import check_harfrust_buffer_admission as h
import check_harfrust_dotted_circle_kernel_cost as shared


def main():
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--output", type=Path, required=True)
    args, _ = parser.parse_known_args()
    args.output = h.artifact_output(args.output)
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    for mode, (source, fixture) in shared.MODES.items():
        prior = json.loads((args.output / f"dotted-circle-kernel-{mode}-cost-summary.json").read_text())
        if (h.sha((src / source).read_bytes()) != prior["source_sha256"] or
                h.sha((src / fixture).read_bytes()) != prior["fixture_sha256"]):
            raise SystemExit(f"Frozen cost input changed: {mode}")
    shared.PREFIX = "grapheme-kernel"
    shared.main()


if __name__ == "__main__":
    main()
