"""Pair saved release executables; keep batch timings and allocation requests."""

import argparse
import gzip
import hashlib
import json
import platform
import re
import statistics
import subprocess
from datetime import datetime, timezone
from pathlib import Path


def artifact(path):
    data = path.read_bytes()
    return {
        "path": str(path),
        "raw_bytes": len(data),
        "gzip_bytes": len(gzip.compress(data, mtime=0)),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def measure(path):
    output = subprocess.run([str(path.resolve())], check=True, capture_output=True, text=True)
    print(output.stdout, end="", flush=True)
    result = {}
    for line in output.stdout.splitlines():
        fields = {key: int(value) for key, value in re.findall(r"(\w+)=(\d+)", line)}
        if "frame_mode" in fields:
            key = f"frame_{fields['frame_mode']}"
        elif "border_mode" in fields:
            key = f"border_{fields['border_mode']}"
        else:
            key = "style"
        result[key] = fields
    if len(result) != 8:
        raise ValueError(f"incomplete workload output: {path}")
    return result


def summarize(rounds):
    return {
        mode: {
            metric: {
                "median": statistics.median(sample[mode][metric] for sample in rounds),
                "min": min(sample[mode][metric] for sample in rounds),
                "max": max(sample[mode][metric] for sample in rounds),
            }
            for metric in ("p50_ns", "p95_ns", "allocations", "requested_bytes")
        }
        for mode in rounds[0]
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--rounds", type=int, default=10)
    args = parser.parse_args()
    directory = args.artifacts.resolve()
    receipt = {
        "started_at": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(),
        "machine": platform.machine(),
        "profile": {"opt_level": "s", "lto": True, "codegen_units": 1, "panic": "abort", "strip": True},
        "native_toolchain": "1.96.0",
        "wasm_toolchain": "stable 1.99.1",
        "baseline_commit": "cfc7ac9c",
        "samples_per_run": 101,
        "updates_per_sample": 120,
        "timing_unit": "ns per batch including setup and teardown",
        "allocations": "allocator request count; requested bytes include realloc size",
        "artifacts": {},
        "measurements": {},
    }
    for configuration in ("default", "off"):
        for platform_name, extension in (("native", "bin"), ("wasm", "wasm")):
            for version in ("before", "after"):
                name = f"{platform_name}-{configuration}-{version}.{extension}"
                receipt["artifacts"][name] = artifact(directory / name)
        runs = {"before": [], "after": []}
        for iteration in range(args.rounds):
            order = ("before", "after") if iteration % 2 == 0 else ("after", "before")
            for version in order:
                print(f"configuration={configuration} iteration={iteration} version={version}", flush=True)
                runs[version].append(measure(directory / f"native-{configuration}-{version}.bin"))
        receipt["measurements"][configuration] = {
            "runs": runs,
            "summary": {version: summarize(samples) for version, samples in runs.items()},
        }
    receipt["finished_at"] = datetime.now(timezone.utc).isoformat()
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    main()
