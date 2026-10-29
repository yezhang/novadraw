#!/usr/bin/env python3
"""Build and launch the Shapes editor through macOS LaunchServices."""

import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys


def main() -> None:
    if sys.platform != "darwin":
        raise SystemExit("This launcher requires macOS; use the documented Cargo entry on other platforms.")

    project = Path(__file__).resolve().parents[2]
    build_dir = project / "target" / "egui"
    build_env = os.environ.copy()
    build_env["CARGO_TARGET_DIR"] = str(build_dir)
    subprocess.run(
        [
            "cargo", "build", "--release", "--locked",
            "--manifest-path", str(project / "examples/native/gef-editor-demo/Cargo.toml"),
            "--bin", "gef-editor-demo",
        ],
        cwd=project,
        env=build_env,
        check=True,
    )

    bundle = build_dir / "Novadraw Shapes.app"
    executable_dir = bundle / "Contents" / "MacOS"
    executable_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(build_dir / "release" / "gef-editor-demo", executable_dir / "gef-editor-demo")
    metadata = {
        "CFBundleIdentifier": "org.novadraw.gef-editor-demo",
        "CFBundleName": "Novadraw Shapes",
        "CFBundleDisplayName": "Novadraw Shapes",
        "CFBundleExecutable": "gef-editor-demo",
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
