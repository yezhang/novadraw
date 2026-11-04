#!/usr/bin/env python3
"""Measure assembly attribution and diagnostic perturbation, never reuse speedup."""
import argparse
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
ASSEMBLY_STAGES = {
    "flow-contribution", "flow-visit", "flow-dispatch", "flow-resolve",
    "flow-contributor", "flow-handler", "flow-text", "flow-text-append",
    "flow-source-map", "flow-box-intent", "flow-decorate", "flow-block",
    "flow-children", "flow-border", "flow-border-renderer", "flow-border-metrics",
}
OPERATION_ROOTS = {
    "initial": "stable-query", "append": "stable-query", "stable": "stable-query",
    "scroll": "stable-query", "resize": "stable-query", "record": "prepare-submission",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command_output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def distribution(values):
    values = sorted(values)
    return {
        "n": len(values), "sum": sum(values), "max": max(values),
        **{f"p{int(p * 100)}": values[math.ceil(p * len(values)) - 1]
           for p in (0.5, 0.95, 0.99)},
    }


def validate(records, samples, workload, enabled):
    issues = []
    for index in range(samples):
        rows = [row for row in records if row.get("sample") == index]
        ok = [row for row in rows if row.get("status") == "ok"]
        extended = [row for row in rows if row.get("status") == "extended"]
        if len(ok) != 1 or len(extended) != 1:
            issues.append({"sample": index, "reason": "missing successful outcome"})
            continue
        if workload == "original":
            expected = {"paragraphs": 40, "bytes": 51232, "lines": 561,
                        "runs": 1022, "fragments": 4168, "effective_width": 586}
            if any(ok[0].get(key) != value for key, value in expected.items()):
                issues.append({"sample": index, "reason": "original denominator"})
            if (extended[0]["commands"] != 7889 or
                    extended[0]["native_glyphs"] != 51233):
                issues.append({"sample": index, "reason": "original output denominator"})
        if extended[0]["scroll_layout_calls"] != 0:
            issues.append({"sample": index, "reason": "scroll performed new layout"})
        receipts = [row for row in rows if row.get("phase") == "source-commit"]
        if len(receipts) != 1 or receipts[0]["revision"] != receipts[0]["previous_revision"] + 1:
            issues.append({"sample": index, "reason": "source receipt"})
        for operation, root in OPERATION_ROOTS.items():
            prefix = f"runtime-{operation}/"
            phases = [row for row in rows if row.get("phase", "").startswith(prefix)]
            roots = [row for row in phases if row["phase"] == prefix + root]
            if enabled:
                if (len(roots) != 1 or
                        any(row["exclusive_ns"] > row["inclusive_ns"] for row in phases) or
                        sum(row["exclusive_ns"] for row in phases) != roots[0]["inclusive_ns"]):
                    issues.append({"sample": index, "reason": f"{operation} accounting"})
            elif phases:
                issues.append({"sample": index, "reason": "disabled timing emitted stages"})
    for row in records:
        if row.get("status") not in ("ok", "extended", "work"):
            issues.append({"reason": "driver failure", "record": row})
    return issues


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--coarse", type=Path, required=True)
    parser.add_argument("--fine", type=Path, required=True)
    parser.add_argument("--sessions", type=int, default=5)
    parser.add_argument("--samples", type=int, default=40)
    parser.add_argument("--warmup", type=int, default=5)
    parser.add_argument("--timeout", type=int, default=60)
    parser.add_argument("--workloads", nargs="+", default=["original", "paragraph", "line", "mixed"],
                        choices=["original", "paragraph", "line", "mixed"])
    args = parser.parse_args()
    if args.sessions < 1 or args.samples < 1 or args.warmup < 0:
        parser.error("invalid sample counts")
    binaries = {"coarse": args.coarse.resolve(), "fine": args.fine.resolve()}
    manifests = {}
    for name, directory in binaries.items():
        manifests[name] = json.loads((directory / "manifest.json").read_text())
        if digest(directory / "textflow-perf") != manifests[name]["binary_sha256"]:
            raise ValueError(f"{name} binary differs from frozen receipt")
    for field in ("cargo_lock_sha256", "fonts", "fixture_source_sha256", "rustc", "cargo"):
        if manifests["coarse"][field] != manifests["fine"][field]:
            raise ValueError(f"unmatched {field}")
    args.destination.mkdir(parents=True, exist_ok=False)
    receipt = {
        "source_commit": command_output("git", "rev-parse", "HEAD"),
        "script_sha256": digest(Path(__file__)), "configuration": vars(args).copy(),
        "binaries": {name: manifests[name]["binary_sha256"] for name in binaries},
        "frozen_manifests": {name: digest(directory / "manifest.json")
                             for name, directory in binaries.items()},
        "machine": platform.machine(), "os": platform.platform(),
        "hardware": command_output("sysctl", "-n", "machdep.cpu.brand_string"),
        "power_start": command_output("pmset", "-g", "batt"),
        "load_start": os.getloadavg(),
        "scope": "fresh Runtime CPU attribution; on/off instrumentation controls",
        "limitation": "background load and OS cache uncontrolled; no production reuse or present",
    }
    receipt["configuration"] = {key: str(value) if isinstance(value, Path) else value
                                for key, value in receipt["configuration"].items()}
    groups, denominators, failures = {}, {}, []
    lanes = [("coarse", True), ("fine", True), ("coarse", False), ("fine", False)]
    raw_path = args.destination / "samples.ndjson.gz"
    with gzip.open(raw_path, "wt") as output:
        for workload in args.workloads:
            for session in range(args.sessions):
                # Rotate every lane through every position in each four-session block.
                order = lanes[session % len(lanes):] + lanes[:session % len(lanes)]
                for version, enabled in order:
                    count = args.samples + args.warmup
                    command = [str(binaries[version] / "textflow-perf"), str(count),
                               "extended", workload, "timing-on" if enabled else "timing-off"]
                    start = time.monotonic_ns()
                    try:
                        result = subprocess.run(command, cwd=ROOT, capture_output=True,
                                                text=True, timeout=args.timeout)
                        records = [json.loads(line) for line in result.stdout.splitlines()]
                        issues = validate(records, count, workload, enabled)
                        if result.returncode:
                            issues.append({"reason": "nonzero exit", "exit_code": result.returncode})
                        process = {"exit_code": result.returncode, "stderr": result.stderr}
                    except (subprocess.TimeoutExpired, json.JSONDecodeError) as error:
                        records = []
                        issues = [{"reason": type(error).__name__, "detail": str(error)}]
                        process = {"failure": str(error)}
                        if isinstance(error, subprocess.TimeoutExpired):
                            process["partial_stdout"] = (error.stdout or b"").decode(errors="replace")
                            process["partial_stderr"] = (error.stderr or b"").decode(errors="replace")
                        else:
                            process.update(stdout=result.stdout, stderr=result.stderr)
                    lane = f"{workload}/{version}/{'on' if enabled else 'off'}"
                    output.write(json.dumps({"status": "process", "lane": lane, "session": session,
                                             "command": command, "elapsed_ns": time.monotonic_ns() - start,
                                             **process}) + "\n")
                    for issue in issues:
                        failures.append({"lane": lane, "session": session, **issue})
                    for row in records:
                        warmup = row.get("sample", count) < args.warmup
                        output.write(json.dumps({**row, "lane": lane, "session": session,
                                                 "warmup": warmup}) + "\n")
                        if warmup:
                            continue
                        if row["status"] in ("ok", "extended"):
                            for key, value in row.items():
                                if key.endswith("_ns"):
                                    groups.setdefault(f"{lane}/{key}", []).append(value)
                            if row["status"] == "ok":
                                fields = ("paragraphs", "bytes", "lines", "runs", "fragments",
                                          "effective_width")
                            else:
                                fields = ("commands", "native_glyphs", "scroll_layout_calls",
                                          "resize_layout_calls", "resize_width")
                            for key in fields:
                                denominators.setdefault(f"{lane}/{key}", set()).add(row[key])
                        elif row["status"] == "work" and row.get("phase", "").startswith("runtime-"):
                            for key in ("calls", "inclusive_ns", "exclusive_ns"):
                                groups.setdefault(f"{lane}/{row['phase']}/{key}", []).append(row[key])
                        elif row["status"] == "work":
                            for key in ("builds", "layout_calls", "paragraph_hits",
                                        "source_geometry_calls"):
                                if key in row:
                                    name = f"{lane}/{row['phase']}/{key}"
                                    denominators.setdefault(name, set()).add(row[key])
                    output.flush()
                    print(f"{lane} session={session} failures={len(issues)}", flush=True)
    receipt.update(power_end=command_output("pmset", "-g", "batt"), load_end=os.getloadavg(),
                   raw_sha256=digest(raw_path), failures=len(failures))
    summary = {
        "units": "ns", "groups": {key: distribution(values) for key, values in groups.items()},
        "denominators": {key: sorted(values) for key, values in denominators.items()},
        "failures": failures,
        "assembly_stages": sorted(ASSEMBLY_STAGES),
        "statistics": "nearest-rank; exclude warmup; stage quantiles are not additive",
    }
    (args.destination / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    (args.destination / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    if failures:
        raise SystemExit(f"{len(failures)} validation failures retained")


if __name__ == "__main__":
    main()
