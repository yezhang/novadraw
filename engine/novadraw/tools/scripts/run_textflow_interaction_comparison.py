"""Alternate frozen release executables through identical native input replay."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import time

from analyze_textflow_interactions import analyze, distribution


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--cycles", type=int, default=2)
    parser.add_argument("--gpu-wait", action="store_true")
    parser.add_argument("--native", action="store_true",
                        help="Launch each frozen executable through a macOS app bundle")
    parser.add_argument("--no-flow-assembly-reuse", action="store_true",
                        help="Compare the default full-assembly path")
    parser.add_argument("--before-no-flow-assembly-reuse", action="store_true",
                        help="Disable reuse only for before; allows same-binary off/on baselines")
    args = parser.parse_args()
    binaries = {"before": args.before.resolve(), "after": args.after.resolve()}
    hashes = {label: hashlib.sha256(binary.read_bytes()).hexdigest()
              for label, binary in binaries.items()}
    same_config = args.no_flow_assembly_reuse or not args.before_no_flow_assembly_reuse
    if same_config and hashes["before"] == hashes["after"]:
        raise ValueError("Identical binaries and configurations cannot measure a code change")
    args.output.mkdir(parents=True, exist_ok=True)
    receipts, groups = [], {}
    for run in range(args.runs):
        for label in (("before", "after") if run % 2 == 0 else ("after", "before")):
            binary = binaries[label]
            directory = (args.output / f"{label}-{run}").resolve()
            command = [str(binary), "--interaction-replay", "--cycles", str(args.cycles),
                       "--timings", "--natural-damage",
                       "--exit-after-replay", "--output", str(directory)]
            if not args.native:
                command.append("--offscreen-gpu")
            if not args.no_flow_assembly_reuse and not (
                    label == "before" and args.before_no_flow_assembly_reuse):
                command.append("--flow-assembly-reuse")
            if args.gpu_wait:
                command.append("--gpu-wait")
            if args.native:
                bundle = directory / "TextFlowLarge.app"
                executable_dir = bundle / "Contents" / "MacOS"
                executable_dir.mkdir(parents=True, exist_ok=True)
                shutil.copy2(binary, executable_dir / "textflow-large-app")
                with (bundle / "Contents" / "Info.plist").open("wb") as output:
                    plistlib.dump({
                        "CFBundleIdentifier": "org.novadraw.textflow-large-research",
                        "CFBundleName": "TextFlowLarge",
                        "CFBundleExecutable": "textflow-large-app",
                        "CFBundlePackageType": "APPL",
                        "CFBundleVersion": "0.1.0",
                        "LSUIElement": False,
                        "NSHighResolutionCapable": True,
                    }, output)
                for name in ("app-stdout.txt", "app-stderr.txt"):
                    (directory / name).touch()
                command = [
                    "open", "-n", "-W",
                    "--stdout", str(directory / "app-stdout.txt"),
                    "--stderr", str(directory / "app-stderr.txt"),
                    str(bundle), "--args", *command[1:]]
            started = time.time()
            directory.mkdir(parents=True, exist_ok=True)
            with (directory / "stdout.txt").open("w") as stdout, (
                    directory / "stderr.txt").open("w") as stderr:
                try:
                    result = subprocess.run(
                        command, stdout=stdout, stderr=stderr, text=True, timeout=180)
                except subprocess.TimeoutExpired:
                    (directory / "failure-receipt.json").write_text(json.dumps({
                        "label": label, "run": run, "started": started,
                        "command": command, "outcome": "launcher_timeout",
                        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    }, indent=2))
                    raise
            result.check_returncode()
            timing_files = list(directory.glob("timings-*.json"))
            if len(timing_files) != 1:
                raise ValueError(f"Expected one timing file in {directory}")
            document = json.loads(timing_files[0].read_text())
            if not any(r["kind"] == "replay_complete" for r in document["records"]):
                raise ValueError(f"Incomplete replay in {directory}")
            expected = "native_backend_presented" if args.native else "gpu_offscreen_published"
            if not any(r["kind"] == "frame" and r["outcome"] == expected
                       for r in document["records"]):
                raise ValueError(f"No {expected} frames in {directory}")
            config, samples = analyze(timing_files[0])
            receipt = {
                "label": label, "run": run, "started": started, "command": command,
                "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "config": config, "samples": samples,
            }
            receipts.append(receipt)
            for sample in samples:
                groups.setdefault((label, sample["operation"]), []).append(sample)
            (args.output / "receipts.json").write_text(json.dumps(receipts, indent=2))
            print(f"{label}-{run}: {len(samples)} operations", flush=True)
    summary = {
        f"{label}/{name}": {
            metric: distribution([s[metric] for s in samples]) for metric in (
                "active_ms", "operation_ms", "input_extra_ms", "frame_ms", "gpu_wait_ms",
                "latency_ms", "glyphs", "commands")
        } for (label, name), samples in groups.items()
    }
    geometry_fields = ("bytes", "lines", "runs", "fragments", "effective_width", "first_source_text")
    baseline = {receipt["run"]: receipt for receipt in receipts if receipt["label"] == "before"}
    compared = 0
    for receipt in receipts:
        if receipt["label"] != "after":
            continue
        before = baseline[receipt["run"]]["samples"]
        after = receipt["samples"]
        if len(before) != len(after):
            raise ValueError("Before/after operation counts differ")
        for old, new in zip(before, after):
            if old["operation"] != new["operation"] or abs(old["delta_y"] - new["delta_y"]) > 1e-6:
                raise ValueError("Before/after native input results differ")
            if any(old["snapshot"][field] != new["snapshot"][field] for field in geometry_fields):
                raise ValueError("Before/after text geometry differs")
            compared += 1
    (args.output / "geometry-check.json").write_text(json.dumps({
        "matched_operations": compared, "fields": geometry_fields,
        "scope": "text summary and viewport displacement; pixel equivalence requires separate capture"
    }, indent=2))
    (args.output / "comparison.json").write_text(json.dumps(summary, indent=2))
    for name in sorted({name for _, name in groups}):
        print(name, json.dumps({
            label: {metric: summary[f"{label}/{name}"][metric]["p50"] for metric in (
                "active_ms", "frame_ms", "latency_ms")}
            for label in binaries
        }), flush=True)


if __name__ == "__main__":
    main()
