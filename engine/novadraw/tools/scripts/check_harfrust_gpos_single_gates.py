#!/usr/bin/env python3
"""Check scalar GPOS receipts, baseline diagnostics and Native/Wasm builds."""
import argparse
import gzip
import json
from pathlib import Path

import check_harfrust_buffer_admission as harness
from check_harfrust_context_admission import reconstruct
from check_harfrust_multiple_gates import warning_signatures
import text_admission_build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--wasm-target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    verified = json.loads((output / "gpos-single-verification.json").read_text())
    for relative, checksum in verified["production_sources_sha256"].items():
        if harness.sha((harness.PRODUCTION / relative).read_bytes()) != checksum:
            raise SystemExit(f"Production changed: {relative}")
    patch = output / "harfrust-0.3.2-gpos-single.patch"
    if harness.sha(patch.read_bytes()) != verified["incremental_patch_sha256"]:
        raise SystemExit("Frozen patch changed")
    if patch.read_bytes() != (harness.REPO / "dependencies" / patch.name).read_bytes():
        raise SystemExit("Dependency patch copy differs")
    for oracle, checksum in verified["oracle_sources_sha256"].items():
        if harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs").read_bytes()) != checksum:
            raise SystemExit(f"Frozen oracle changed: {oracle}")
    for fixture, checksum in verified["fixture_sources_sha256"].items():
        if harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{fixture}_fixtures.rs").read_bytes()) != checksum:
            raise SystemExit(f"Frozen fixture changed: {fixture}")
    for key, checksum in verified["ordinary_output_signatures"].items():
        oracle, owner, profile = key.split("/")
        data = gzip.decompress((output / f"gpos-single-{oracle}-{owner}-{profile}.log.gz").read_bytes())
        if harness.sha(data) != checksum or len(data.splitlines()) != verified["observations_per_configuration"][key]:
            raise SystemExit(f"Output receipt differs: {key}")
    for profile in ["native", "portable"]:
        for kind, field in [("consumer", "external_test_log_sha256"), ("private", "private_test_log_sha256")]:
            if harness.sha((output / f"gpos-single-{kind}-{profile}.log").read_bytes()) != verified[field][profile]:
                raise SystemExit(f"Test receipt differs: {kind}/{profile}")
    for name in ["gpos-single-cost", "gpos-single-ordinary-cost"]:
        sampled = json.loads((output / f"{name}-summary.json").read_text())
        source = harness.REPO / f"tests/consumers/harfrust-admission/src/{name.replace('-', '_')}.rs"
        if harness.sha(source.read_bytes()) != sampled["source_sha256"]:
            raise SystemExit(f"Cost source changed: {name}")
        if name.endswith("ordinary-cost"):
            for key, group in sampled["groups"].items():
                owner, _, _, session = key.split("/")
                if harness.sha((output / f"gpos-single-ordinary-{owner}-{session}.ndjson").read_bytes()) != group["raw_sha256"]:
                    raise SystemExit(f"Ordinary sample differs: {key}")
        elif harness.sha((output / "gpos-single-cost.ndjson").read_bytes()) != sampled["raw_sha256"]:
            raise SystemExit("Cost sample differs")
    _, original, before = reconstruct(output, verified["prior_patches"], "text-harfrust-gpos-lint-")
    build = harness.REPO / "target/text-harfrust-gpos-single-build"
    package = build / "lib-before-gpos"
    harness.manifest(package, before / "src/lib.rs", original / "Cargo.toml")
    native = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    wasm = text_admission_build.environment(args.wasm_target_dir.resolve(), "clang")
    wasm.pop("RUSTFLAGS", None)
    consumer = harness.REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    cargo, checks = ["cargo", "+1.96.0"], {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        collected = {}
        for owner, manifest in [("baseline", package / "Cargo.toml"), ("production", build / "lib-production/Cargo.toml")]:
            command = cargo + ["clippy", "--offline", "--manifest-path", str(manifest),
                               "--lib", "--message-format=json", *flags]
            log = output / f"gpos-single-clippy-{owner}-{profile}.jsonl"
            harness.run(command, log, native, harness.REPO)
            collected[owner] = warning_signatures(log)
            checks[f"clippy-{owner}-{profile}"] = {"log_sha256": harness.sha(log.read_bytes()),
                "warning_signatures": collected[owner], "passed": True}
        if collected["baseline"] != collected["production"]:
            raise SystemExit(f"New Clippy diagnostics: {profile}")
        checks[f"clippy-comparison-{profile}"] = {
            "passed": True, "new_warnings": 0, "existing_warnings": len(collected["baseline"])}
    commands = [
        ("consumer-clippy-native", cargo + ["clippy", "--offline", "--manifest-path", str(consumer),
            "--lib", "--", "-D", "warnings"], native),
        ("consumer-clippy-portable", cargo + ["clippy", "--offline", "--manifest-path", str(consumer),
            "--lib", "--no-default-features", "--features", "portable", "--", "-D", "warnings"], native),
        ("cost-clippy", cargo + ["clippy", "--offline", "--manifest-path",
            str(harness.REPO / "target/text-harfrust-gpos-single-cost/Cargo.toml"),
            "--bin", "oracle", "--", "-D", "warnings"], native),
        ("ordinary-cost-clippy", cargo + ["clippy", "--offline", "--manifest-path",
            str(harness.REPO / "target/text-harfrust-gpos-single-cost/build/cost-production/Cargo.toml"),
            "--bin", "oracle", "--", "-D", "warnings"], native),
        ("core-native-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib"], native),
        ("core-wasm-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib",
            "--target", "wasm32-unknown-unknown"], wasm),
        ("consumer-wasm-build", cargo + ["build", "--offline", "--manifest-path", str(consumer),
            "--lib", "--no-default-features", "--features", "portable",
            "--target", "wasm32-unknown-unknown"], wasm)]
    for name, command, env in commands:
        log = output / f"gpos-single-{name}.log"
        harness.run(command, log, env, harness.REPO)
        checks[name] = {"command": command, "log_sha256": harness.sha(log.read_bytes()), "passed": True}
    receipt = {"checks": checks, "linker": args.linker, "wasm_executed": False,
        "production_sources_sha256": verified["production_sources_sha256"],
        "consumer_source_sha256": harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/gpos_single_tests.rs").read_bytes())}
    for relative, checksum in verified["production_sources_sha256"].items():
        if harness.sha((harness.PRODUCTION / relative).read_bytes()) != checksum:
            raise SystemExit(f"Production changed during gates: {relative}")
    (output / "gpos-single-gates.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
