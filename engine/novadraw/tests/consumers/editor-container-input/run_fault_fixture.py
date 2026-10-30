"""Replay E03 counter exhaustion only in a disposable source copy."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    evidence = root / "doc/verification/evidence/p2-e03-followup-2026-10-30"
    expected = json.loads((evidence / "source-fingerprints.json").read_text())
    for name, value in expected.items():
        digest = hashlib.sha256((root / name).read_bytes()).hexdigest()
        if digest != value["sha256"]:
            raise SystemExit(f"Source changed; review and regenerate the fixture: {name}")
    copy = Path(tempfile.mkdtemp(prefix="novadraw-e03-counter-fixture-"))
    for name in ("crates/novadraw", "crates/novadraw-editor", "assets/fonts"):
        shutil.copytree(root / name, copy / name)
    manifest = (root / "Cargo.toml").read_text()
    tail = manifest[manifest.index("[workspace.package]"):]
    (copy / "Cargo.toml").write_text(
        '[workspace]\nresolver = "3"\n'
        'members = ["crates/novadraw", "crates/novadraw-editor"]\n\n' + tail
    )
    shutil.copy2(root / "Cargo.lock", copy / "Cargo.lock")
    subprocess.run(
        ["git", "apply", str(evidence / "isolated-fault-injection.patch")],
        cwd=copy,
        check=True,
    )
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    env["RUSTFLAGS"] = "--cfg e03_fault_fixture --check-cfg=cfg(e03_fault_fixture)"
    print(f"Isolated source copy: {copy}", flush=True)
    subprocess.run(
        [
            "cargo", "+1.96.0", "test", "--manifest-path", str(copy / "Cargo.toml"),
            "-p", "novadraw-editor", "--test", "p2_e03_container_editing_contract",
            "e03_real_revision_exhaustion_blocks_both_tools_before_command_build",
            "--", "--exact", "--nocapture",
        ],
        env=env,
        check=True,
    )


if __name__ == "__main__":
    main()
