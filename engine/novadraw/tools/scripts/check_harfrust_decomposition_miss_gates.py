#!/usr/bin/env python3
"""Check terminal b-miss receipts and Native/portable/Wasm gates."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import check_harfrust_buffer_admission as h
from check_harfrust_context_admission import reconstruct
from check_harfrust_multiple_gates import warning_signatures
import text_admission_build


def verify_sources(replayed):
    expected = {p.relative_to(replayed) for p in (replayed / "src").rglob("*") if p.is_file()}
    actual = {p.relative_to(h.PRODUCTION) for p in (h.PRODUCTION / "src").rglob("*") if p.is_file()}
    if expected != actual:
        raise SystemExit("Production source addition/removal during gates")
    for relative in expected:
        if (replayed / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Production changed during gates: {relative}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--wasm-target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = h.artifact_output(args.output)
    verified = json.loads((output / "decomposition-miss-verification.json").read_text())
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    if h.sha((src / "space_private_oracle.rs").read_bytes()) != verified["invisible_oracle_source_sha256"]:
        raise SystemExit("Invisible oracle changed")
    for key, checksum in verified["reused_invisible_ordinary_signatures"].items():
        owner, profile = key.split("/")
        rows = [line[line.index(b"SPACE/"):] for line in
                (output / f"space-invisible-{owner}-{profile}.log").read_bytes().splitlines()
                if b"SPACE/" in line]
        if (len(rows) != verified["reused_invisible_ordinary_counts"][key] or
                h.sha(b"\n".join(rows) + b"\n") != checksum):
            raise SystemExit(f"Invisible receipt differs: {key}")
    for name, checksum in verified["consumer_sources_sha256"].items():
        if h.sha((src / name).read_bytes()) != checksum:
            raise SystemExit(f"Consumer changed: {name}")
    patch = output / "harfrust-0.3.2-decomposition-miss.patch"
    if (h.sha(patch.read_bytes()) != verified["incremental_patch_sha256"] or
            patch.read_bytes() != (h.REPO / "dependencies" / patch.name).read_bytes()):
        raise SystemExit("Frozen patch differs")
    for kind, field in [("oracle", "oracle_sources_sha256"), ("fixtures", "fixture_sources_sha256")]:
        for name, checksum in verified[field].items():
            if h.sha((src / f"{name}_{kind}.rs").read_bytes()) != checksum:
                raise SystemExit(f"Frozen {kind} changed: {name}")
    for key, checksum in verified["ordinary_output_signatures"].items():
        oracle, owner, profile = key.split("/")
        digest, count = hashlib.sha256(), 0
        with gzip.open(output / f"decomposition-miss-{oracle}-{owner}-{profile}.log.gz", "rb") as saved:
            while chunk := saved.read(1024 * 1024):
                digest.update(chunk)
                count += chunk.count(b"\n")
        if digest.hexdigest() != checksum or count != verified["observations_per_configuration"][key]:
            raise SystemExit(f"Frozen output differs: {key}")
    for profile in ["native", "portable"]:
        for kind, field in [("consumer", "external_test_log_sha256"), ("private", "private_test_log_sha256")]:
            if h.sha((output / f"decomposition-miss-{kind}-{profile}.log").read_bytes()) != verified[field][profile]:
                raise SystemExit(f"Test receipt differs: {kind}/{profile}")
    for name in ["decomposition-miss-cost", "decomposition-miss-ordinary-cost"]:
        sampled = json.loads((output / f"{name}-summary.json").read_text())
        if h.sha((src / f"{name.replace('-', '_')}.rs").read_bytes()) != sampled["source_sha256"]:
            raise SystemExit(f"Cost source changed: {name}")
        fixture_field = "input_sha256" if "ordinary" in name else "fixture_sha256"
        if h.sha((src / "decomposition_miss_fixtures.rs").read_bytes()) != sampled[fixture_field]:
            raise SystemExit(f"Cost fixture changed: {name}")
        if sampled["production_sources_sha256"] != verified["production_sources_sha256"]:
            raise SystemExit(f"Cost production differs: {name}")
        if "ordinary" in name:
            for key, group in sampled["groups"].items():
                owner, _, _, session = key.split("/")
                if h.sha((output / f"decomposition-miss-ordinary-{owner}-{session}.ndjson").read_bytes()) != group["raw_sha256"]:
                    raise SystemExit(f"Ordinary sample differs: {key}")
        elif h.sha((output / "decomposition-miss-cost.ndjson").read_bytes()) != sampled["raw_sha256"]:
            raise SystemExit("Cost sample differs")
    _, original, before = reconstruct(output, verified["prior_patches"], "text-harfrust-decomposition-miss-lint-")
    # Reconstruct the current patch too, so every unchanged source file is checked.
    h.run(["git", "apply", "--no-index", str(patch)], output / "decomposition-miss-gates-replay.log",
          os.environ, before)
    verify_sources(before)
    # Keep the baseline before the incremental patch for warning comparison.
    _, _, baseline_source = reconstruct(output, verified["prior_patches"], "text-harfrust-decomposition-miss-baseline-")
    build = h.REPO / "target/text-harfrust-decomposition-miss-build"
    baseline = build / "lib-before-decomposition-miss"
    h.manifest(baseline, baseline_source / "src/lib.rs", original / "Cargo.toml")
    native = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    wasm = text_admission_build.environment(args.wasm_target_dir.resolve(), "clang")
    wasm.pop("RUSTFLAGS", None)
    cargo, checks = ["cargo", "+1.96.0"], {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        signatures = {}
        for owner, package in [("baseline", baseline), ("production", build / "lib-production")]:
            log = output / f"decomposition-miss-clippy-{owner}-{profile}.jsonl"
            h.run(cargo + ["clippy", "--offline", "--manifest-path", str(package / "Cargo.toml"),
                          "--lib", "--message-format=json", *flags], log, native, h.REPO)
            signatures[owner] = warning_signatures(log)
            checks[f"clippy-{owner}-{profile}"] = {
                "passed": True, "warning_signatures": signatures[owner], "log_sha256": h.sha(log.read_bytes())}
        if signatures["baseline"] != signatures["production"]:
            raise SystemExit(f"New Clippy diagnostics: {profile}")
        checks[f"clippy-comparison-{profile}"] = {
            "passed": True, "new_warnings": 0, "existing_warnings": len(signatures["baseline"])}
    consumer = h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    commands = [
        ("consumer-clippy-native", cargo + ["clippy", "--offline", "--manifest-path", str(consumer), "--lib", "--", "-D", "warnings"], native),
        ("consumer-clippy-portable", cargo + ["clippy", "--offline", "--manifest-path", str(consumer), "--lib", "--no-default-features", "--features", "portable", "--", "-D", "warnings"], native),
        ("cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(h.REPO / "target/text-harfrust-decomposition-miss-cost/Cargo.toml"), "--bin", "oracle", "--", "-D", "warnings"], native),
        ("ordinary-cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(h.REPO / "target/text-harfrust-decomposition-miss-cost/build/cost-production/Cargo.toml"), "--bin", "oracle", "--", "-D", "warnings"], native),
        ("core-native-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib"], native),
        ("core-wasm-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib", "--target", "wasm32-unknown-unknown"], wasm),
        ("consumer-wasm-build", cargo + ["build", "--offline", "--manifest-path", str(consumer), "--lib", "--no-default-features", "--features", "portable", "--target", "wasm32-unknown-unknown"], wasm),
    ]
    for name, command, env in commands:
        log = output / f"decomposition-miss-{name}.log"
        h.run(command, log, env, h.REPO)
        checks[name] = {"command": command, "passed": True, "log_sha256": h.sha(log.read_bytes())}
    verify_sources(before)
    receipt = {"checks": checks, "linker": args.linker, "wasm_executed": False,
               "all_production_source_bytes_replayed": True,
               "production_sources_sha256": verified["production_sources_sha256"],
               "consumer_sources_sha256": verified["consumer_sources_sha256"]}
    (output / "decomposition-miss-gates.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
