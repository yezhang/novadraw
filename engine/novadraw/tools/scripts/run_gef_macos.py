#!/usr/bin/env python3
"""Build and launch a GEF example through macOS LaunchServices."""

import argparse
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys


def main() -> None:
    if sys.platform != "darwin":
        raise SystemExit("This launcher requires macOS; use the documented Cargo entry on other platforms.")

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--demo", choices=("shapes", "logic"), default="shapes")
    args = parser.parse_args()
    package, display_name = {
        "shapes": ("gef-editor-demo", "Novadraw Shapes"),
        "logic": ("gef-logic-demo", "Novadraw Logic"),
    }[args.demo]
    project = Path(__file__).resolve().parents[2]
    build_dir = project / "target" / "egui"
    bundle = build_dir / f"{display_name}.app"
    executable_dir = bundle / "Contents" / "MacOS"
    expected_executable = executable_dir / package
    running = subprocess.run(
        ["pgrep", "-x", package], capture_output=True, text=True, check=False,
    )
    for pid in running.stdout.splitlines():
        if not pid.isdigit():
            continue
        command = subprocess.run(
            ["ps", "-p", pid, "-o", "comm="], capture_output=True, text=True, check=False,
        ).stdout.strip()
        if command and Path(command).resolve() == expected_executable.resolve():
            print(f"{display_name} is already running. Save and close it before rebuilding.", flush=True)
            subprocess.run(["open", "-W", str(bundle)], check=True)
            return

    build_env = os.environ.copy()
    build_env["CARGO_TARGET_DIR"] = str(build_dir)
    subprocess.run(
        [
            "cargo", "build", "--release", "--locked",
            "--manifest-path", str(project / "examples/native" / package / "Cargo.toml"),
            "--bin", package,
        ],
        cwd=project,
        env=build_env,
        check=True,
    )

    executable_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(build_dir / "release" / package, executable_dir / package)
    metadata = {
        "CFBundleIdentifier": f"org.novadraw.{package}",
        "CFBundleName": display_name,
        "CFBundleDisplayName": display_name,
        "CFBundleExecutable": package,
        "CFBundlePackageType": "APPL",
        "CFBundleVersion": "0.1.0",
        "CFBundleShortVersionString": "0.1.0",
        "LSUIElement": False,
        "NSHighResolutionCapable": True,
    }
    with (bundle / "Contents" / "Info.plist").open("wb") as output:
        plistlib.dump(metadata, output)

    # Launch the exact bundle URL, rather than selecting another installed app
    # by name/identifier. Keep the tracked command alive until this app closes.
    subprocess.run(["open", "-W", str(bundle)], check=True)


if __name__ == "__main__":
    main()
