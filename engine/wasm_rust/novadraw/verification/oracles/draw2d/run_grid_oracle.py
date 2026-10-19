#!/usr/bin/env python3
"""Compile official source against explicit Draw2D/SWT artifacts and record callback evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

BASELINE = "4463d9d0ce13c19d10fbe769d29f28b7345a8cba"


def fingerprint(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference-root", required=True, type=Path)
    parser.add_argument("--draw2d-jar", required=True, type=Path)
    parser.add_argument("--swt-jar", required=True, type=Path)
    parser.add_argument("--native-library-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    root = args.reference_root.resolve()
    revision = subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != BASELINE:
        raise RuntimeError(f"Expected {BASELINE}, got {revision}")
    source_root = root / "org.eclipse.draw2d.examples/src"
    sources = [
        source_root / "org/eclipse/draw2d/examples/AbstractExample.java",
        source_root / "org/eclipse/draw2d/examples/layouts/GridLayoutExample.java",
        Path(__file__).resolve().with_name("GridEventOracle.java"),
    ]
    artifacts = [args.draw2d_jar.resolve(), args.swt_jar.resolve()]
    classpath = ":".join(map(str, artifacts))
    with tempfile.TemporaryDirectory(prefix="novadraw-grid-oracle-") as directory:
        subprocess.run(
            ["javac", "-cp", classpath, "-d", directory, *map(str, sources)],
            check=True, timeout=60,
        )
        result = subprocess.run(
            ["java", "-XstartOnFirstThread",
             f"-Djava.library.path={args.native_library_dir.resolve()}",
             "-cp", f"{directory}:{classpath}",
             "org.eclipse.draw2d.examples.layouts.GridEventOracle"],
            capture_output=True, text=True, timeout=60,
        )
        if result.returncode:
            raise RuntimeError(result.stdout + result.stderr)
    evidence = {
        "baseline": revision,
        "mode": "registered callbacks; native SWT widgets; no OS input injection",
        "sources": {str(path.relative_to(root)) if path.is_relative_to(root) else path.name:
                    fingerprint(path) for path in sources},
        "artifacts": {str(path): fingerprint(path) for path in artifacts},
        "java": subprocess.run(["java", "-version"], capture_output=True, text=True,
                               check=True).stderr,
        "stdout": result.stdout,
        "stderr": result.stderr,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n")
    print(result.stdout, end="")


if __name__ == "__main__":
    main()
