#!/usr/bin/env python3
"""Run serial checks for the multiple-substitution dependency slice."""
import argparse
import gzip
import json
from pathlib import Path
import os
import tarfile
import tempfile

import check_harfrust_buffer_admission as harness
import text_admission_build


def warning_signatures(log):
    signatures = []
    for line in log.read_text().splitlines():
        if not line.startswith("{"):
            continue
        item = json.loads(line)
        if item.get("reason") != "compiler-message":
            continue
        message = item["message"]
        if message["level"] != "warning":
            continue
        spans = [span for span in message["spans"] if span["is_primary"]]
        signatures.append({
            "code": message["code"]["code"] if message["code"] else None,
            "message": message["message"],
            "spans": [{"source": str(Path(span["file_name"])).split("/src/")[-1],
                       "text": [part["text"] for part in span["text"]]} for span in spans],
        })
    return sorted(signatures, key=lambda item: json.dumps(item, sort_keys=True))


def lint_baseline(output, patches):
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Archive receipt differs")
    root = Path(tempfile.mkdtemp(prefix="text-harfrust-multiple-lint-", dir=harness.REPO / "target"))
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            if not member.isfile():
                continue
            relative = Path(member.name).relative_to("harfrust-0.3.2")
            if relative.is_absolute() or ".." in relative.parts:
                raise SystemExit("Unexpected archive member")
            dest = root / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(source.extractfile(member).read())
    for name in patches:
        harness.run(["git", "apply", "--no-index", str(output / f"harfrust-0.3.2-{name}.patch")],
                    output / f"multiple-lint-{name}-replay.log", os.environ, root)
    package = harness.REPO / "target/text-harfrust-multiple-build/lib-before-multiple"
    harness.manifest(package, root / "src/lib.rs", root / "Cargo.toml")
    return package / "Cargo.toml"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--wasm-target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="clang")
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    verified = json.loads((output / "multiple-verification.json").read_text())
    if not verified["patch_replays_and_reproduces_exact_source"]:
        raise SystemExit("Source replay must pass before gates")
    for name, checksum in verified["prior_patches"].items():
        if harness.sha((output / f"harfrust-0.3.2-{name}.patch").read_bytes()) != checksum:
            raise SystemExit(f"Frozen patch changed: {name}")
    if harness.sha((output / "harfrust-0.3.2-multiple.patch").read_bytes()) != verified["incremental_patch_sha256"]:
        raise SystemExit("Multiple patch changed after verification")
    for oracle, signature in verified["oracle_sources_sha256"].items():
        source = harness.REPO / f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs"
        if harness.sha(source.read_bytes()) != signature:
            raise SystemExit(f"Frozen oracle source changed: {oracle}")
    for fixture in ["multiple", "single"]:
        source = harness.REPO / f"tests/consumers/harfrust-admission/src/{fixture}_fixtures.rs"
        if harness.sha(source.read_bytes()) != verified[f"{fixture}_fixture_sha256"]:
            raise SystemExit(f"Fixture receipt differs: {fixture}")
    for key, signature in verified["ordinary_output_signatures"].items():
        oracle, owner, profile = key.split("/")
        log = output / f"multiple-{oracle}-{owner}-{profile}.log.gz"
        data = gzip.decompress(log.read_bytes())
        if (harness.sha(data) != signature or len(data.splitlines()) !=
                verified["observations_per_configuration"][key]):
            raise SystemExit(f"Oracle receipt differs: {key}")
    for profile in ["native", "portable"]:
        for kind, receipt in [("consumer", "external_test_log_sha256"),
                              ("private", "private_test_log_sha256")]:
            if harness.sha((output / f"multiple-{kind}-{profile}.log").read_bytes()) != verified[receipt][profile]:
                raise SystemExit(f"Test receipt differs: {kind}/{profile}")
    for name in ["multiple-cost", "multiple-ordinary-cost"]:
        sampled = json.loads((output / f"{name}-summary.json").read_text())
        source = harness.REPO / f"tests/consumers/harfrust-admission/src/{name.replace('-', '_')}.rs"
        if harness.sha(source.read_bytes()) != sampled["source_sha256"]:
            raise SystemExit(f"Cost source receipt differs: {name}")
        if name == "multiple-cost":
            if harness.sha((output / "multiple-cost.ndjson").read_bytes()) != sampled["raw_sha256"]:
                raise SystemExit("Cost sample receipt differs")
        else:
            for key, group in sampled["groups"].items():
                owner, _, _, session = key.split("/")
                raw = output / f"multiple-ordinary-{owner}-{session}.ndjson"
                if harness.sha(raw.read_bytes()) != group["raw_sha256"]:
                    raise SystemExit(f"Ordinary sample receipt differs: {key}")
    native = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    wasm = text_admission_build.environment(args.wasm_target_dir.resolve(), "clang")
    wasm.pop("RUSTFLAGS", None)
    consumer = harness.REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    dependency = harness.REPO / "target/text-harfrust-multiple-build/lib-production/Cargo.toml"
    cargo = ["cargo"]
    baseline = lint_baseline(output, verified["prior_patches"])
    receipts = {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        collected = {}
        for owner, manifest in [("baseline", baseline), ("production", dependency)]:
            command = cargo + ["clippy", "--offline", "--manifest-path", str(manifest),
                               "--lib", "--message-format=json", *flags]
            log = output / f"multiple-clippy-{owner}-{profile}.jsonl"
            harness.run(command, log, native, harness.REPO)
            collected[owner] = warning_signatures(log)
            receipts[f"clippy-{owner}-{profile}"] = {
                "command": command, "log_sha256": harness.sha(log.read_bytes()),
                "warning_signatures": collected[owner], "completed": True,
            }
        if collected["baseline"] != collected["production"]:
            raise SystemExit(f"New or changed dependency Clippy diagnostics: {profile}")
        receipts[f"clippy-comparison-{profile}"] = {
            "passed": True, "new_warnings": 0, "existing_warnings": len(collected["baseline"]),
        }
    checks = [
        ("consumer-clippy-native", cargo + ["clippy", "--offline", "--manifest-path", str(consumer),
                                           "--lib", "--", "-D", "warnings"], native),
        ("consumer-clippy-portable", cargo + ["clippy", "--offline", "--manifest-path", str(consumer),
                                             "--lib", "--no-default-features", "--features", "portable",
                                             "--", "-D", "warnings"], native),
        ("cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(consumer),
                                "--bin", "multiple-cost", "--", "-D", "warnings"], native),
        ("ordinary-cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(
            harness.REPO / "target/text-harfrust-multiple-cost/build/cost-production/Cargo.toml"),
                                         "--bin", "oracle", "--", "-D", "warnings"], native),
        ("core-native-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib"], native),
        ("core-wasm-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib",
                                    "--target", "wasm32-unknown-unknown"], wasm),
        ("consumer-wasm-build", cargo + ["build", "--offline", "--manifest-path", str(consumer),
                                        "--lib", "--no-default-features", "--features", "portable",
                                        "--target", "wasm32-unknown-unknown"], wasm),
    ]
    for name, command, env in checks:
        log = output / f"multiple-{name}.log"
        harness.run(command, log, env, harness.REPO)
        receipts[name] = {"command": command, "log_sha256": harness.sha(log.read_bytes()),
                          "passed": True}
    receipt = {
        "checks": receipts, "linker": args.linker, "wasm_executed": False,
        "production_sources_sha256": {str(path.relative_to(harness.PRODUCTION)):
            harness.sha(path.read_bytes()) for path in
            [harness.PRODUCTION / "src/hb/shape_admission.rs",
             harness.PRODUCTION / "src/hb/buffer_admission.rs"]},
    }
    (output / "multiple-gates.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
