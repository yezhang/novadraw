#!/usr/bin/env python3
"""Freeze recoverable CPU baselines and retain raw matched TextFlow samples."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tarfile
import time

ROOT = Path(__file__).resolve().parent.parent


def run(*args):
    return subprocess.check_output(args, cwd=ROOT).decode().strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def freeze_materialized(destination, binary, source):
    """The archive, rather than an evolving working tree, is this binary's source authority."""
    destination.mkdir(parents=True, exist_ok=False)
    source = source.resolve()
    frozen_binary = destination / "textflow-perf"
    shutil.copy2(binary, frozen_binary)
    archive_path = destination / "source.tar.gz"
    with tarfile.open(archive_path, "w:gz") as archive:
        for path in sorted(source.iterdir()):
            if path.name not in ("target", ".git"):
                archive.add(path, arcname=path.name)
    fonts = list((source / "assets/fonts").glob("*.ttf"))
    fonts += list((source / "examples/native/draw2d-official-examples/assets").glob("*.ttf"))
    manifest = {
        "version": 2,
        "source_authority": "complete materialized source.tar.gz, including Cargo.lock",
        "source_archive_sha256": digest(archive_path),
        "cargo_lock_sha256": digest(source / "Cargo.lock"),
        "configuration_sha256": digest(source / "Cargo.toml"),
        "binary_sha256": digest(frozen_binary),
        "binary_bytes": frozen_binary.stat().st_size,
        "driver_sha256": digest(source / "examples/native/draw2d-official-examples/src/bin/textflow-perf.rs"),
        "workloads_sha256": digest(source / "examples/native/draw2d-official-examples/src/bin/textflow-perf/workloads.rs"),
        "sampler_sha256": digest(Path(__file__)),
        "fixture_source_sha256": digest(source / "examples/native/draw2d-official-examples/src/examples/text_examples.rs"),
        "fonts": {str(p.relative_to(source)): digest(p) for p in fonts},
        "rustc": run("rustc", "-vV"), "cargo": run("cargo", "-V"),
        "machine": platform.machine(), "os": platform.platform(),
        "hardware": run("sysctl", "-n", "machdep.cpu.brand_string"),
        "power": run("pmset", "-g", "batt"),
        "build": {"command": f"cargo build --locked --release -p draw2d-official-examples --bin textflow-perf --manifest-path {source / 'Cargo.toml'}",
                  "target_directory": str(binary.resolve().parent.parent),
                  "profile": "release", "features": "default",
                  "environment": {k: os.environ[k] for k in
                                  ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET")
                                  if k in os.environ}},
        "instrumentation": "identical opt-in Runtime timing and cumulative Flow counters",
        "scope": "CPU exact layout, fixed append, stable query; optional scroll/resize/IR",
        "present": "unavailable: no window or GPU",
        "cache": "fresh Runtime per sample; cold-process means new process, not controlled OS cache",
        "statistics": "integer monotonic ns; nearest-rank ceil(p*n); failures retained",
    }
    (destination / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return frozen_binary


def freeze(destination, binary, workload="original"):
    destination.mkdir(parents=True, exist_ok=False)
    (destination / "working.patch").write_bytes(
        subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=ROOT)
    )
    untracked = run("git", "ls-files", "--others", "--exclude-standard", "-z").split("\0")
    sources = [p for p in untracked if p and Path(p).suffix in
               {".rs", ".toml", ".lock", ".md", ".py", ".sh"}]
    with tarfile.open(destination / "untracked.tar.gz", "w:gz") as archive:
        for path in sources:
            archive.add(ROOT / path, arcname=path)
    frozen_binary = destination / "textflow-perf"
    shutil.copy2(binary, frozen_binary)
    shutil.copy2(ROOT / "Cargo.lock", destination / "Cargo.lock")
    fonts = list((ROOT / "assets/fonts").glob("*.ttf"))
    fonts += list((ROOT / "examples/native/draw2d-official-examples/assets").glob("*.ttf"))
    manifest = {
        "version": 1,
        "commit": run("git", "rev-parse", "HEAD"),
        "patch_sha256": digest(destination / "working.patch"),
        "untracked": {p: digest(ROOT / p) for p in sources},
        "cargo_lock_sha256": digest(ROOT / "Cargo.lock"),
        "driver_sha256": digest(ROOT / "scripts/textflow_performance.py"),
        "binary_sha256": digest(frozen_binary),
        "binary_bytes": frozen_binary.stat().st_size,
        "rustc": run("rustc", "-vV"),
        "cargo": run("cargo", "-V"),
        "machine": platform.machine(),
        "os": platform.platform(),
        "hardware": run("sysctl", "-n", "machdep.cpu.brand_string"),
        "power": run("pmset", "-g", "batt"),
        "fonts": {str(p.relative_to(ROOT)): digest(p) for p in fonts},
        "font_order_source": ["novadraw/src/render/text.rs",
                              "examples/native/draw2d-official-examples/src/examples/text_examples.rs"],
        "build": {
            "command": "cargo build --release -p draw2d-official-examples --bin textflow-perf",
            "profile": "release",
            "features": "Cargo default features of draw2d-official-examples",
            "configuration_sha256": digest(ROOT / "Cargo.toml"),
            "environment": {k: os.environ[k] for k in
                            ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET",
                             "CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS")
                            if k in os.environ},
        },
        "fixture": "original TextFlowLargeExample public factory; fixed first-source append Z",
        "fixture_source_sha256": digest(ROOT / "examples/native/draw2d-official-examples/src/examples/text_examples.rs"),
        "layout": {"initial_canvas": [600, 500], "effective_width": 586,
                   "font": "13px Arial", "paragraphs": 40, "bytes": 51232,
                   "lines": 561, "runs": 1022, "fragments": 4168},
        "scope": "CPU construction, exact stabilization, dispatch and append end-to-end",
        "present": "unavailable: this driver does not create a window or GPU",
        "cache": "fresh Runtime per sample; OS/file cache uncontrolled; warm samples in same process",
        "statistics": "integer monotonic nanoseconds; nearest-rank ceil(p*n); failures retained",
        "instrumentation": "Flow phase counters and opt-in Runtime inclusive/exclusive timing enabled",
        "workload": workload,
    }
    if workload != "original":
        manifest["fixture"] = f"independent {workload} workload; fixed first-source append Z"
        manifest["layout"] = None
        manifest["fixture_source_sha256"] = digest(
            ROOT / "examples/native/draw2d-official-examples/src/bin/textflow-perf/workloads.rs")
    (destination / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return frozen_binary


def sample(binary, count, session, kind, output, timeout, warmup=0, extended=False,
           workload="original"):
    start = time.monotonic_ns()
    try:
        command = [str(binary), str(count + warmup)]
        if extended:
            command.append("extended")
        elif workload != "original":
            command.append("standard")
        if workload != "original":
            command.append(workload)
        result = subprocess.run(command, cwd=ROOT,
                                capture_output=True, text=True, timeout=timeout)
        elapsed = time.monotonic_ns() - start
        records = []
        for line in result.stdout.splitlines():
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                record = {"status": "invalid_output", "output": line}
            record.update(session=session, kind=(
                "warmup" if record.get("sample", warmup) < warmup else kind
            ))
            records.append(record)
        outcomes = [r for r in records if r["status"] not in ("work", "extended", "resize_failure")]
        if len(outcomes) != count + warmup:
            records.append({"status": "sample_count_mismatch", "session": session, "kind": kind,
                            "expected": count + warmup, "actual": len(outcomes)})
        for record in outcomes:
            expected = {
                "original": {"paragraphs": 40, "bytes": 51232, "lines": 561, "runs": 1022,
                             "fragments": 4168, "effective_width": 586},
                "paragraph": {"fixture": "paragraph", "paragraphs": 1,
                              "bytes": len("alpha beta gamma delta epsilon. ") * 1600,
                              "effective_width": 586},
                "line": {"fixture": "line", "paragraphs": 1, "bytes": 51200,
                         "lines": 1},
                "mixed": {"fixture": "mixed", "paragraphs": 40,
                          "bytes": (len("mixed Figure text ") * 64 +
                                    len(" after inline Figure.")) * 40,
                          "effective_width": 586},
            }[workload]
            if record["status"] == "ok" and any(record.get(key) != value
                                               for key, value in expected.items()):
                record["status"] = "fixture_mismatch"
            if record["status"] == "ok":
                for phase in ("runtime-initial/", "runtime-append/"):
                    stages = [r for r in records if r.get("sample") == record["sample"]
                              and r.get("phase", "").startswith(phase)]
                    roots = [r for r in stages if r["phase"] == phase + "stable-query"]
                    if (len(roots) != 1 or any(r["exclusive_ns"] > r["inclusive_ns"]
                                               for r in stages)
                            or sum(r["exclusive_ns"] for r in stages) !=
                            roots[0]["inclusive_ns"]):
                        record["status"] = "timing_accounting_mismatch"
                if workload != "original":
                    receipts = [r for r in records if r.get("sample") == record["sample"]
                                and r.get("phase") == "source-commit"]
                    if (len(receipts) != 1 or receipts[0]["revision"] !=
                            receipts[0]["previous_revision"] + 1):
                        record["status"] = "source_receipt_mismatch"
        if extended:
            extensions = [r for r in records if r["status"] == "extended"]
            if len(extensions) != count + warmup:
                records.append({"status": "extended_count_mismatch", "session": session,
                                "kind": kind, "expected": count + warmup,
                                "actual": len(extensions)})
            for record in extensions:
                if (workload != "line" and record.get("resize_width") != 386) or (
                        workload == "original" and record.get("native_glyphs") != 51233):
                    record["status"] = "extended_fixture_mismatch"
        records.append({"status": "process", "session": session, "kind": kind,
                        "exit_code": result.returncode, "process_ns": elapsed,
                        "stderr": result.stderr})
    except subprocess.TimeoutExpired as error:
        records = [{"status": "timeout", "session": session, "kind": kind,
                    "timeout_seconds": timeout,
                    "partial_stdout": (error.stdout or b"").decode(errors="replace"),
                    "partial_stderr": (error.stderr or b"").decode(errors="replace")}]
    for record in records:
        output.write(json.dumps(record) + "\n")
    output.flush()


def summarize(destination):
    manifest = json.loads((destination / "manifest.json").read_text())
    assert digest(destination / "textflow-perf") == manifest["binary_sha256"]
    records = [json.loads(line) for line in (destination / "samples.ndjson").read_text().splitlines()]
    timings = ["construct_ns", "initial_ns", "dispatch_ns", "append_update_ns",
               "append_e2e_ns", "stable_ns"]
    groups = {}
    for record in records:
        if record.get("status") == "ok" and record.get("kind") != "warmup":
            for grouping in (f"{record['kind']}/session-{record['session']}", record["kind"]):
                group = groups.setdefault(grouping, {key: [] for key in timings})
                for key in timings:
                    group[key].append(record[key])
    summary = {}
    for name, group in groups.items():
        summary[name] = {}
        for metric, values in group.items():
            values.sort()
            summary[name][metric] = {
                "n": len(values), "max": max(values),
                **{f"p{int(p*100)}": values[math.ceil(p * len(values)) - 1]
                   for p in (0.5, 0.95, 0.99)},
            }
    extensions = {}
    for record in records:
        if record["status"] == "extended" and record["kind"] != "warmup":
            for metric in ("scroll_ns", "resize_ns", "record_ns"):
                extensions.setdefault(metric, []).append(record[metric])
    extended_summary = {}
    for metric, values in extensions.items():
        values.sort()
        extended_summary[metric] = {"n": len(values), "max": max(values),
            **{f"p{int(p*100)}": values[math.ceil(p*len(values))-1] for p in (0.5, 0.95, 0.99)}}
    additional = {}
    for record in records:
        if record["kind"] == "warmup":
            continue
        if record["status"] == "extended":
            metrics = ("scroll_ns", "resize_ns", "record_ns", "scroll_layout_calls",
                       "resize_layout_calls", "commands", "native_glyphs")
            category = "extended"
        elif record["status"] == "process" and record["exit_code"] == 0:
            metrics = ("process_ns",)
            category = "process"
        elif record["status"] == "work":
            metrics = tuple(k for k in record if k.endswith("_ns") or k in
                ("builds", "layout_calls", "paragraph_hits", "source_geometry_calls", "calls"))
            category = f"work/{record['phase']}"
        else:
            continue
        for grouping in (f"{category}/{record['kind']}/session-{record['session']}",
                         f"{category}/{record['kind']}"):
            group = additional.setdefault(grouping, {})
            for metric in metrics:
                group.setdefault(metric, []).append(record[metric])
    additional_summary = {}
    for grouping, group in additional.items():
        additional_summary[grouping] = {}
        for metric, values in group.items():
            values.sort()
            additional_summary[grouping][metric] = {
                "n": len(values), "sum": sum(values), "max": max(values),
                **{f"p{int(p*100)}": values[math.ceil(p*len(values))-1]
                   for p in (0.5, 0.95, 0.99)},
            }
    failures = [r for r in records if r["status"] not in ("ok", "work", "process", "extended")
                or (r["status"] == "process" and r["exit_code"] != 0)]
    (destination / "summary.json").write_text(json.dumps({
        "units": "ns", "groups": summary, "failures": failures,
        "extended": extended_summary,
        "additional_groups": additional_summary,
        "summarizer_sha256": digest(Path(__file__)),
        "binary_sha256": manifest["binary_sha256"],
        "present": "unavailable",
    }, indent=2) + "\n")

def memory(destination, workload="original"):
    records = []
    for sample_index in range(5):
        order = ("baseline", "optimized") if sample_index % 2 == 0 else ("optimized", "baseline")
        for version in order:
            path = destination / version
            manifest = json.loads((path / "manifest.json").read_text())
            assert digest(path / "textflow-perf") == manifest["binary_sha256"]
            command = ["/usr/bin/time", "-l", str(path.resolve() / "textflow-perf"), "1"]
            if workload != "original":
                command.extend(["standard", workload])
            result = subprocess.run(command,
                                    cwd=ROOT, capture_output=True, text=True, timeout=600)
            match = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
            records.append({"version": version, "sample": sample_index,
                            "exit_code": result.returncode,
                            "maximum_resident_bytes": int(match[1]) if match else None,
                            "binary_bytes": (path / "textflow-perf").stat().st_size,
                            "stdout": result.stdout, "stderr": result.stderr})
    (destination / "memory.json").write_text(json.dumps({
        "scope": f"macOS whole-process peak RSS, {workload} CPU fixture, 5 interleaved new processes per version",
        "sampler_sha256": digest(Path(__file__)),
        "limitation": "not retained allocation attribution, GPU memory, Web size or a hard budget proof",
        "records": records,
    }, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["freeze", "run", "summarize", "smoke", "compare", "memory"])
    parser.add_argument("destination", type=Path)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/textflow-perf")
    parser.add_argument("--sessions", type=int, default=5)
    parser.add_argument("--samples", type=int, default=200)
    parser.add_argument("--warmup", type=int, default=20)
    parser.add_argument("--cold-per-session", type=int, default=40)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--baseline", type=Path, default=ROOT / "target/verification/long-text/baseline")
    parser.add_argument("--extended", action="store_true")
    parser.add_argument("--workload", choices=["original", "paragraph", "line", "mixed"],
                        default="original")
    parser.add_argument("--source-directory", type=Path)
    parser.add_argument("--optimized", type=Path)
    args = parser.parse_args()
    assert args.sessions > 0 and args.samples > 0
    assert args.warmup >= 0 and args.cold_per_session >= 0
    if args.action == "smoke":
        args.destination = args.destination / f"run-{time.time_ns()}"
        freeze(args.destination, args.binary, args.workload)
        args.sessions, args.samples, args.warmup, args.cold_per_session = 1, 3, 1, 1
        print(args.destination, flush=True)
    if args.action == "memory":
        memory(args.destination, args.workload)
    elif args.action == "compare":
        if args.optimized:
            args.binary = args.optimized / "textflow-perf"
        baseline_manifest = json.loads((args.baseline / "manifest.json").read_text())
        assert digest(args.baseline / "textflow-perf") == baseline_manifest["binary_sha256"]
        if "source_archive_sha256" in baseline_manifest:
            assert digest(args.baseline / "source.tar.gz") == baseline_manifest["source_archive_sha256"]
        assert digest(args.binary) != baseline_manifest["binary_sha256"], (
            "comparison binaries are identical; use separate Cargo target directories")
        if args.optimized:
            optimized_manifest = json.loads((args.optimized / "manifest.json").read_text())
            assert digest(args.binary) == optimized_manifest["binary_sha256"]
            if "source_archive_sha256" in optimized_manifest:
                assert digest(args.optimized / "source.tar.gz") == optimized_manifest["source_archive_sha256"]
            for key in ("cargo_lock_sha256", "configuration_sha256", "driver_sha256",
                        "workloads_sha256", "fixture_source_sha256", "fonts", "instrumentation"):
                assert baseline_manifest.get(key) == optimized_manifest.get(key), (
                    f"comparison inputs differ: {key}")
        args.destination.mkdir(parents=True, exist_ok=False)
        old = args.destination / "baseline"
        old.mkdir()
        for name in ("textflow-perf", "manifest.json", "working.patch", "untracked.tar.gz",
                     "source.tar.gz", "Cargo.lock"):
            if (args.baseline / name).exists():
                shutil.copy2(args.baseline / name, old / name)
        if args.optimized:
            optimized = args.destination / "optimized"
            optimized.mkdir()
            for name in ("textflow-perf", "manifest.json", "working.patch", "untracked.tar.gz",
                         "source.tar.gz", "Cargo.lock"):
                if (args.optimized / name).exists():
                    shutil.copy2(args.optimized / name, optimized / name)
        elif args.source_directory:
            freeze_materialized(args.destination / "optimized", args.binary, args.source_directory)
        else:
            freeze(args.destination / "optimized", args.binary, args.workload)
        versions = ["baseline", "optimized"]
        for version in versions:
            path = args.destination / version
            manifest = json.loads((path / "manifest.json").read_text())
            assert digest(path / "textflow-perf") == manifest["binary_sha256"]
            manifest["sampling"] = {k: getattr(args, k) for k in
                ("sessions", "samples", "warmup", "cold_per_session", "timeout")}
            manifest["sampling"]["order"] = "alternate version order by session, cold pairs interleaved"
            manifest["sampling"]["runner_sha256"] = digest(Path(__file__))
            manifest["sampling"]["extended"] = args.extended
            manifest["sampling"]["workload"] = args.workload
            manifest["sampling"]["timing_validation"] = (
                "each initial/append exclusive stage sum equals stable-query inclusive ns")
            manifest["sampling"]["source_commit"] = (
                "original key dispatch, independent receipt unavailable"
                if args.workload == "original" else
                "SetStructuredFlowText receipt; revision increases once and text gains fixed Z")
            manifest["fixture_parameters"] = {
                "initial_canvas": [600, 500],
                "declared_font": ("13px Arial" if args.workload == "original"
                                  else "13px Inter Variable"),
                "wrap": args.workload != "line",
                "actual_layout_denominators": "validated individually in samples.ndjson",
                "preparation_budget": {"work": 10000000, "allocation_entries": 2000000},
                "input_authority": "frozen factory/workloads source and font hashes",
            }
            manifest["sampling"]["process_ns"] = "launcher to process exit, includes append and destruction; not first-ready or present latency"
            (path / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
            if args.extended:
                preflight = path / "extended-preflight.ndjson"
                with preflight.open("w") as output:
                    sample(path / "textflow-perf", 1, -1, "preflight", output,
                           args.timeout, extended=True, workload=args.workload)
                records = [json.loads(line) for line in preflight.read_text().splitlines()]
                assert all(r["status"] in ("ok", "work", "extended", "process")
                           and (r["status"] != "process" or r["exit_code"] == 0)
                           for r in records), f"{version} does not support the extended fixture"
        for session in range(args.sessions):
            order = versions if session % 2 == 0 else versions[::-1]
            print(f"session {session}: {','.join(order)}", flush=True)
            (args.destination / f"session-{session}-environment.json").write_text(json.dumps({
                "time_ns": time.time_ns(), "power": run("pmset", "-g", "batt"),
                "load_average": os.getloadavg(), "order": order,
                "sampling_policy": "no agent-driven builds or tests during samples; external background load uncontrolled",
            }, indent=2) + "\n")
            for _ in range(args.cold_per_session):
                for version in order:
                    path = args.destination / version
                    with (path / "samples.ndjson").open("a") as output:
                        sample(path / "textflow-perf", 1, session, "cold-process", output,
                               args.timeout, extended=args.extended, workload=args.workload)
            for version in order:
                path = args.destination / version
                with (path / "samples.ndjson").open("a") as output:
                    sample(path / "textflow-perf", args.samples, session, "fresh-runtime", output,
                           args.timeout, args.warmup, extended=args.extended, workload=args.workload)
                print(f"session {session}: {version} complete", flush=True)
        for version in versions:
            summarize(args.destination / version)
    elif args.action == "freeze":
        if args.source_directory:
            print(freeze_materialized(args.destination, args.binary, args.source_directory))
        else:
            print(freeze(args.destination, args.binary, args.workload))
    elif args.action == "summarize":
        summarize(args.destination)
    else:
        assert args.sessions > 0 and args.samples > 0
        assert args.warmup >= 0 and args.cold_per_session >= 0
        binary = args.destination.resolve() / "textflow-perf"
        manifest_path = args.destination / "manifest.json"
        manifest = json.loads(manifest_path.read_text())
        assert digest(binary) == manifest["binary_sha256"], "frozen binary changed"
        manifest["sampling"] = {k: getattr(args, k) for k in
                                ("sessions", "samples", "warmup", "cold_per_session", "timeout")}
        manifest["sampling"]["extended"] = args.extended
        manifest["sampling"]["workload"] = args.workload
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
        with (args.destination / "samples.ndjson").open("a") as output:
            for session in range(args.sessions):
                (args.destination / f"session-{session}-environment.json").write_text(
                    json.dumps({"time_ns": time.time_ns(), "power": run("pmset", "-g", "batt"),
                                "load_average": os.getloadavg(),
                                "sampling_policy": "no agent-driven builds or tests during samples; external background load uncontrolled"},
                               indent=2) + "\n"
                )
                for _ in range(args.cold_per_session):
                    sample(binary, 1, session, "cold-process", output, args.timeout,
                           extended=args.extended, workload=args.workload)
                sample(binary, args.samples, session, "fresh-runtime", output,
                       args.timeout, args.warmup, extended=args.extended, workload=args.workload)
                print(f"session {session}: complete", flush=True)
        summarize(args.destination)


if __name__ == "__main__":
    main()
