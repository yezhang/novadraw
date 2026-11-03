#!/usr/bin/env python3
"""Check all input-buffer cost samples and report their local latency distribution."""
import argparse
from collections import defaultdict
import hashlib
import json
import platform
from pathlib import Path
from check_harfrust_buffer_admission import artifact_output
import subprocess

REPO = Path(__file__).resolve().parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--samples", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output = artifact_output(args.output)
    groups = defaultdict(list)
    rows = [json.loads(line) for line in args.samples.read_text().splitlines()]
    if len(rows) != 6000:
        raise SystemExit(f"Expected 6000 samples, got {len(rows)}")
    for row in rows:
        key = (row["path"], row["warm"])
        if row["path"] not in {"ordinary", "admitted", "reject-append"}:
            raise SystemExit(f"Unknown path: {key}")
        n, b = row["characters"], row["bytes"]
        expected_reject = row["path"] == "reject-append"
        if row["rejected"] != expected_reject:
            raise SystemExit("Unexpected sample result")
        if row["path"] == "ordinary":
            costs = (0, 0, 0)
        else:
            work = b + 2 + 2 * n
            if not expected_reject:
                work += b + n
            costs = (4 if row["warm"] else 5, work, 0 if row["warm"] else 2 * n)
        if (row["calls"], row["work"], row["allocation"]) != costs:
            raise SystemExit(f"Cost mismatch at {key}: {row}")
        groups[key].append(row)
    summaries = []
    for (path, warm), samples in sorted(groups.items()):
        if len(samples) != 1000 or {r["sample"] for r in samples} != set(range(1000)):
            raise SystemExit(f"Incomplete sample denominator: {path}/{warm}")
        times = sorted(row["elapsed_ns"] for row in samples)
        summaries.append({
            "path": path, "warm_capacity": warm, "samples": len(samples),
            "p50_us": times[499] / 1000, "p95_us": times[949] / 1000,
            "p99_us": times[989] / 1000,
            "bytes": samples[0]["bytes"], "characters": samples[0]["characters"],
        })
    result = {
        "scope": "input buffer only; no shaping, Core budget or width reuse",
        "sampling": "sequential six groups; cold includes previous empty buffer release",
        "environment": {"platform": platform.platform(), "machine": platform.machine(),
                        "rustc": subprocess.check_output(["rustc", "+1.96.0", "-V"], text=True).strip()},
        "samples_sha256": digest(args.samples), "binary_sha256": digest(args.binary),
        "source_sha256": digest(REPO / "tests/consumers/harfrust-admission/src/cost.rs"),
        "samples": len(rows), "unexpected_results": 0, "groups": summaries,
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
