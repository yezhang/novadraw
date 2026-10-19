#!/usr/bin/env python3
"""Capture lexical event/platform evidence for every official entry and inventoried helper."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess

BASELINE = "4463d9d0ce13c19d10fbe769d29f28b7345a8cba"
MARKERS = {
    "registration": re.compile(r"\badd\w*Listener\s*\("),
    "callback": re.compile(
        r"\b(?:mouse(?:Pressed|Released|DoubleClicked|Dragged|Moved|Entered|Exited|Hover)"
        r"|key(?:Pressed|Released)|focus(?:Gained|Lost)|widget(?:Selected|DefaultSelected)"
        r"|control(?:Resized|Moved)|shell(?:Closed|Activated|Deactivated))\s*\("
    ),
    "platform": re.compile(r"^\s*import org\.eclipse\.swt\."),
    "host": re.compile(
        r"\.(?:open|pack|setSize|setBounds|dispose|readAndDispatch|sleep|setFocus"
        r"|setBackgroundMode|setOrientation|setCaret)\s*\("
    ),
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    root = args.reference_root.resolve()
    revision = subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != BASELINE:
        raise RuntimeError(f"Expected {BASELINE}, got {revision}")
    workspace = Path(__file__).resolve().parents[3]
    inventory = workspace / "examples/native/draw2d-official-examples/source-contracts.tsv"
    rows = [line.split("\t") for line in inventory.read_text().splitlines()
            if line and not line.startswith("#")]
    files = sorted({file for row in rows for field in row[1:3]
                    for file in field.split(",") if file != "-"})
    base = root / "org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples"
    output = [f"# baseline={BASELINE}",
              "# Lexical source evidence; not runtime order or semantic completeness.",
              "# path\tline\tkind\texact-source"]
    for file in files:
        path = base / file
        content = path.read_bytes()
        output.append(f"{file}\t0\tsha256\t{hashlib.sha256(content).hexdigest()}")
        for number, line in enumerate(content.decode().splitlines(), 1):
            # Preserve exact token content; indentation is not part of the evidence.
            text = line.strip().replace("\t", " ")
            for kind, marker in MARKERS.items():
                if marker.search(line):
                    output.append(f"{file}\t{number}\t{kind}\t{text}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(output) + "\n")
    print(f"Captured {len(files)} source files, {len(output) - 3 - len(files)} evidence rows")


if __name__ == "__main__":
    main()
