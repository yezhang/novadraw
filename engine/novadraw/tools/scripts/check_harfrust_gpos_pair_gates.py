#!/usr/bin/env python3
"""Check frozen PairPos receipts, baseline Clippy and Native/Wasm builds."""
import argparse
import gzip
import json
from pathlib import Path

import check_harfrust_buffer_admission as h
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
    output = h.artifact_output(args.output)
    verified = json.loads((output / "gpos-pair-verification.json").read_text())
    if h.sha((h.REPO / "tests/consumers/harfrust-admission/src/lib.rs").read_bytes()) != verified["consumer_module_source_sha256"]:
        raise SystemExit("Consumer module configuration changed")
    for relative, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / relative).read_bytes()) != checksum:
            raise SystemExit(f"Production changed: {relative}")
    patch = output / "harfrust-0.3.2-gpos-pair.patch"
    if h.sha(patch.read_bytes()) != verified["incremental_patch_sha256"] or patch.read_bytes() != (h.REPO / "dependencies" / patch.name).read_bytes():
        raise SystemExit("Frozen patch differs")
    for kind, checksums in [("oracle", verified["oracle_sources_sha256"]), ("fixtures", verified["fixture_sources_sha256"])]:
        for name, checksum in checksums.items():
            if h.sha((h.REPO / f"tests/consumers/harfrust-admission/src/{name}_{kind}.rs").read_bytes()) != checksum:
                raise SystemExit(f"Frozen {kind} changed: {name}")
    for key, checksum in verified["ordinary_output_signatures"].items():
        oracle, owner, profile = key.split("/")
        data = gzip.decompress((output / f"gpos-pair-{oracle}-{owner}-{profile}.log.gz").read_bytes())
        if h.sha(data) != checksum or len(data.splitlines()) != verified["observations_per_configuration"][key]:
            raise SystemExit(f"Output receipt differs: {key}")
    for profile in ["native", "portable"]:
        for kind, field in [("consumer", "external_test_log_sha256"), ("private", "private_test_log_sha256")]:
            if h.sha((output / f"gpos-pair-{kind}-{profile}.log").read_bytes()) != verified[field][profile]:
                raise SystemExit(f"Test receipt differs: {kind}/{profile}")
    for name in ["gpos-pair-cost", "gpos-pair-ordinary-cost"]:
        sampled = json.loads((output / f"{name}-summary.json").read_text())
        if h.sha((h.REPO / f"tests/consumers/harfrust-admission/src/{name.replace('-', '_')}.rs").read_bytes()) != sampled["source_sha256"]:
            raise SystemExit(f"Cost source changed: {name}")
        if name.endswith("ordinary-cost"):
            for key, group in sampled["groups"].items():
                owner, _, _, session = key.split("/")
                if h.sha((output / f"gpos-pair-ordinary-{owner}-{session}.ndjson").read_bytes()) != group["raw_sha256"]:
                    raise SystemExit(f"Ordinary sample differs: {key}")
        elif h.sha((output / "gpos-pair-cost.ndjson").read_bytes()) != sampled["raw_sha256"]:
            raise SystemExit("Cost sample differs")
    _, original, before = reconstruct(output, verified["prior_patches"], "text-harfrust-pair-lint-")
    build = h.REPO / "target/text-harfrust-gpos-pair-build"
    package = build / "lib-before-pair"
    h.manifest(package, before / "src/lib.rs", original / "Cargo.toml")
    native = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    wasm = text_admission_build.environment(args.wasm_target_dir.resolve(), "clang")
    wasm.pop("RUSTFLAGS", None)
    cargo, checks = ["cargo", "+1.96.0"], {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        collected = {}
        for owner, manifest in [("baseline", package / "Cargo.toml"), ("production", build / "lib-production/Cargo.toml")]:
            log = output / f"gpos-pair-clippy-{owner}-{profile}.jsonl"
            h.run(cargo + ["clippy", "--offline", "--manifest-path", str(manifest), "--lib", "--message-format=json", *flags], log, native, h.REPO)
            collected[owner] = warning_signatures(log)
            checks[f"clippy-{owner}-{profile}"] = {"passed": True, "warning_signatures": collected[owner], "log_sha256": h.sha(log.read_bytes())}
        if collected["baseline"] != collected["production"]:
            raise SystemExit(f"New Clippy diagnostics: {profile}")
        checks[f"clippy-comparison-{profile}"] = {"passed": True, "new_warnings": 0, "existing_warnings": len(collected["baseline"])}
    consumer = h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    commands = [
        ("consumer-clippy-native", cargo + ["clippy", "--offline", "--manifest-path", str(consumer), "--lib", "--", "-D", "warnings"], native),
        ("consumer-clippy-portable", cargo + ["clippy", "--offline", "--manifest-path", str(consumer), "--lib", "--no-default-features", "--features", "portable", "--", "-D", "warnings"], native),
        ("cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(h.REPO / "target/text-harfrust-gpos-pair-cost/Cargo.toml"), "--bin", "oracle", "--", "-D", "warnings"], native),
        ("ordinary-cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(h.REPO / "target/text-harfrust-gpos-pair-cost/build/cost-production/Cargo.toml"), "--bin", "oracle", "--", "-D", "warnings"], native),
        ("core-native-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib"], native),
        ("core-wasm-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib", "--target", "wasm32-unknown-unknown"], wasm),
        ("consumer-wasm-build", cargo + ["build", "--offline", "--manifest-path", str(consumer), "--lib", "--no-default-features", "--features", "portable", "--target", "wasm32-unknown-unknown"], wasm),
    ]
    for name, command, env in commands:
        log = output / f"gpos-pair-{name}.log"
        h.run(command, log, env, h.REPO)
        checks[name] = {"command": command, "passed": True, "log_sha256": h.sha(log.read_bytes())}
    for relative, checksum in verified["production_sources_sha256"].items():
        if h.sha((h.PRODUCTION / relative).read_bytes()) != checksum:
            raise SystemExit(f"Production changed during gates: {relative}")
    receipt = {"checks": checks, "linker": args.linker, "wasm_executed": False,
        "production_sources_sha256": verified["production_sources_sha256"],
        "consumer_source_sha256": h.sha((h.REPO / "tests/consumers/harfrust-admission/src/gpos_pair_tests.rs").read_bytes())}
    (output / "gpos-pair-gates.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
