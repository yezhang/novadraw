#!/usr/bin/env python3
"""Replay ten frozen patches and verify complete language tag admission."""
import argparse
import difflib
import json
import os
from pathlib import Path
import re
import shutil
import tarfile
import tempfile

import check_harfrust_buffer_admission as harness

CHANGED = {"src/hb/tag.rs", "src/hb/mod.rs", "src/hb/tag_table.rs", "src/hb/ot_shape_plan.rs"}
ADDED = {"src/hb/tag_rules.rs"}


def inventory(reference, production):
    """Exact ordered comparison of every condition and output tag, by branch."""
    body = reference[reference.index("pub fn tags_from_complex_language"):]
    old, group = [], "GLOBAL"
    for match in re.finditer(
        r"(?m)^        b'(.)' =>|^\s*if (.+) \{\n([\s\S]*?)return true;", body
    ):
        if match[1]:
            group = match[1].upper()
            continue
        condition = match[2]
        kind = ("subtag" if condition.startswith("subtag_matches") else
                "both" if "&&" in condition else
                "lang" if condition.startswith("lang_matches") else "exact")
        old.append((group, kind, re.findall(r'"([^"]+)"', condition),
                    re.findall(r'Tag::new\(b"(....)"\)', match[3])))
    new, group = [], None
    for line in production.split("fn subtag_with")[0].splitlines():
        match = re.match(r"const (\w+): &\[Rule\]", line)
        if match:
            group = match[1]
        match = re.match(r'\s+(subtag|both|lang|exact) (.+) => \[(.+)\],', line)
        if match:
            new.append((group, match[1], re.findall(r'"([^"]+)"', match[2]),
                        re.findall(r'b"(....)"', match[3])))
    if old != new or len(old) != 148:
        raise SystemExit("Ordered locked language rule inventory differs")
    return {"rules": len(old), "ordered_inventory_sha256": harness.sha(
        json.dumps(old, ensure_ascii=False).encode()),
        "all_conditions_outputs_and_branch_order_equal": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Archive checksum mismatch")
    prior = json.loads((output / "private-language-verification.json").read_text())
    patches = {**prior["prior_patches"], "private-language": prior["incremental_patch_sha256"]}
    base = harness.REPO / "target/text-harfrust-general-language"
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="run-", dir=base))
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
    before = root / "after-private-language"
    shutil.copytree(original, before)
    for name, checksum in patches.items():
        path = output / f"harfrust-0.3.2-{name}.patch"
        if harness.sha(path.read_bytes()) != checksum:
            raise SystemExit(f"Frozen patch changed: {name}")
        harness.run(["git", "apply", "--no-index", str(path)],
                    output / f"general-language-{name}-replay.log", os.environ, before)
    rules = inventory((before / "src/hb/tag_table.rs").read_text(),
                      (harness.PRODUCTION / "src/hb/tag_rules.rs").read_text())
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(harness.PRODUCTION))
              for p in (harness.PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources | ADDED:
        raise SystemExit("Unexpected source addition or removal")
    patch, changed = [], set()
    for relative in sorted(actual):
        old = (before / relative).read_bytes() if relative in sources else b""
        new = (harness.PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        if relative not in CHANGED | ADDED:
            raise SystemExit(f"Unrelated source changed: {relative}")
        changed.add(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}" if relative in sources else "/dev/null",
            tofile=f"b/{relative}",
        ))
    if changed != CHANGED | ADDED:
        raise SystemExit("Expected source change missing")
    path = output / "harfrust-0.3.2-general-language.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after-general-language"
    shutil.copytree(before, applied)
    harness.run(["git", "apply", "--no-index", str(path)],
                output / "general-language-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, harness.REPO / "dependencies" / path.name)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    env["CC"] = "/Library/Developer/CommandLineTools/usr/bin/clang"
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = env["CC"]
    signatures, counts = {}, {}
    expected = {"plan": 39744, "plan_layout": 50688,
                "private_language": 6720, "general_language": 24048}
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
                            output / f"general-language-{oracle}-{name}-{profile}-build.log", env)
                log = output / f"general-language-{oracle}-{name}-{profile}.log"
                harness.run([str(args.target_dir.resolve() / "debug/oracle")], log, env)
                key = f"{oracle}/{name}/{profile}"
                signatures[key] = harness.sha(log.read_bytes())
                counts[key] = len(log.read_bytes().splitlines())
                if counts[key] != denominator:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
        if name == "production":
            for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
                harness.run(["cargo", "test", "--offline", "--manifest-path",
                             str(lib / "Cargo.toml"), "--lib", *flags],
                            output / f"general-language-private-{profile}.log", env)
    for oracle in expected:
        if len({v for k, v in signatures.items() if k.startswith(oracle + "/")}) != 1:
            raise SystemExit(f"Ordinary output mismatch: {oracle}")
    consumer = {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features", "--features", "portable"])]:
        log = output / f"general-language-consumer-{profile}.log"
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
        "changed_after_private_language": sorted(CHANGED), "added_source_files": sorted(ADDED),
        "unchanged_source_files_after_private_language": len(sources - CHANGED),
        "total_source_files": len(actual), "locked_rules": rules,
        "ordinary_output_signatures": signatures, "observations_per_configuration": counts,
        "oracle_sources_sha256": {n: harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{n}_oracle.rs").read_bytes()) for n in expected},
        "general_language_inputs_sha256": harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/general_language_inputs.rs").read_bytes()),
        "external_test_log_sha256": consumer,
        "language_mapping_admission_supported": True,
        "script_callbacks_and_shaping_admission_complete": False,
        "core_staged_adapter_enabled": False,
    }
    (output / "general-language-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
