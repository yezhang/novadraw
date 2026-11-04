#!/usr/bin/env python3
"""Replay eleven frozen patches and verify fixed-script planning admission."""
import argparse
import difflib
import json
import os
from pathlib import Path
import shutil
import tarfile
import tempfile

import check_harfrust_buffer_admission as harness

CHANGED = {
    "src/hb/ot_map.rs", "src/hb/ot_shape.rs", "src/hb/ot_shape_plan.rs",
    "src/hb/ot_shaper.rs", "src/hb/ot_shaper_arabic.rs", "src/hb/ot_shaper_hangul.rs",
    "src/hb/ot_shaper_hebrew.rs", "src/hb/ot_shaper_thai.rs",
    "src/hb/ot_shaper_indic.rs", "src/hb/ot_shaper_khmer.rs",
    "src/hb/ot_shaper_use.rs", "src/hb/ot_shaper_myanmar.rs",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Archive checksum mismatch")
    prior = json.loads((output / "general-language-verification.json").read_text())
    patches = {**prior["prior_patches"], "general-language": prior["incremental_patch_sha256"]}
    base = harness.REPO / "target"
    root = Path(tempfile.mkdtemp(prefix="text-harfrust-script-callbacks-", dir=base))
    original = root / "original"
    original.mkdir()
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            if not member.isfile():
                continue
            relative = Path(member.name).relative_to("harfrust-0.3.2")
            if relative.is_absolute() or ".." in relative.parts:
                raise SystemExit("Unexpected archive member")
            dest = original / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(source.extractfile(member).read())
    before = root / "after-general-language"
    shutil.copytree(original, before)
    for name, checksum in patches.items():
        path = output / f"harfrust-0.3.2-{name}.patch"
        if harness.sha(path.read_bytes()) != checksum:
            raise SystemExit(f"Frozen patch changed: {name}")
        harness.run(["git", "apply", "--no-index", str(path)],
                    output / f"script-callbacks-{name}-replay.log", os.environ, before)
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(harness.PRODUCTION))
              for p in (harness.PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources:
        raise SystemExit("Unexpected source addition or removal")
    patch, changed = [], set()
    for relative in sorted(actual):
        old, new = (before / relative).read_bytes(), (harness.PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        if relative not in CHANGED:
            raise SystemExit(f"Unrelated source changed: {relative}")
        changed.add(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}", tofile=f"b/{relative}",
        ))
    if changed != CHANGED:
        raise SystemExit("Expected source change missing")
    path = output / "harfrust-0.3.2-script-callbacks.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after-script-callbacks"
    shutil.copytree(before, applied)
    harness.run(["git", "apply", "--no-index", str(path)],
                output / "script-callbacks-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, harness.REPO / "dependencies" / path.name)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    env["CC"] = "/Library/Developer/CommandLineTools/usr/bin/clang"
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = env["CC"]
    signatures, counts = {}, {}
    expected = {"plan": 39744, "plan_layout": 50688, "private_language": 6720,
                "general_language": 24048, "script_callbacks": 51840}
    for name, library in [("original", original), ("production", harness.PRODUCTION)]:
        lib = root / f"lib-{name}"
        harness.manifest(lib, library / "src/lib.rs", original / "Cargo.toml")
        for oracle, denominator in expected.items():
            harness.SOURCE = harness.REPO / f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs"
            package = root / f"oracle-{name}-{oracle}"
            for profile in ["native", "portable"]:
                harness.manifest(package, lib, original / "Cargo.toml", binary=True)
                manifest = package / "Cargo.toml"
                if profile == "portable":
                    manifest.write_text(manifest.read_text().replace(
                        "path = " + json.dumps(str(lib)) + " }",
                        "path = " + json.dumps(str(lib)) + ", default-features = false }",
                    ))
                harness.run(["cargo", "build", "--offline", "--manifest-path",
                             str(manifest), "--bin", "oracle"],
                            output / f"script-callbacks-{oracle}-{name}-{profile}-build.log", env)
                log = output / f"script-callbacks-{oracle}-{name}-{profile}.log"
                harness.run([str(args.target_dir.resolve() / "debug/oracle")], log, env)
                key = f"{oracle}/{name}/{profile}"
                signatures[key] = harness.sha(log.read_bytes())
                counts[key] = len(log.read_bytes().splitlines())
                if denominator is not None and counts[key] != denominator:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
                if counts[key] == 0:
                    raise SystemExit(f"Empty oracle: {key}")
        if name == "production":
            for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
                harness.run(["cargo", "test", "--offline", "--manifest-path",
                             str(lib / "Cargo.toml"), "--lib", *flags],
                            output / f"script-callbacks-private-{profile}.log", env)
    for oracle in expected:
        selected = {k: v for k, v in signatures.items() if k.startswith(oracle + "/")}
        if len(set(selected.values())) != 1:
            raise SystemExit(f"Ordinary output mismatch: {oracle}")
    consumer = {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features", "--features", "portable"])]:
        log = output / f"script-callbacks-consumer-{profile}.log"
        harness.run(["cargo", "test", "--offline", "--manifest-path",
                     str(harness.REPO / "tests/consumers/harfrust-admission/Cargo.toml"),
                     "--lib", *flags], log, env)
        consumer[profile] = harness.sha(log.read_bytes())
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Source changed during verification: {relative}")
    receipt = {
        "archive_sha256": harness.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": harness.sha(path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_general_language": sorted(CHANGED),
        "unchanged_source_files_after_general_language": len(sources - CHANGED),
        "total_source_files": len(actual), "ordinary_output_signatures": signatures,
        "observations_per_configuration": counts, "external_test_log_sha256": consumer,
        "oracle_sources_sha256": {n: harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{n}_oracle.rs").read_bytes()) for n in expected},
        "syriac_fixture_sha256": harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/script_callbacks_fixtures.rs").read_bytes()),
        "supported_script_plan_callbacks": ["Arabic", "Syriac", "Hangul"],
        "shaping_admission_complete": False, "core_staged_adapter_enabled": False,
    }
    (output / "script-callbacks-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
