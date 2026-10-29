#!/usr/bin/env python3
"""Launch this checkout's official Draw2D samples with macOS LaunchServices."""

import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys


def main() -> None:
    if sys.platform != "darwin":
        raise SystemExit("This launcher requires macOS.")
    project = Path(__file__).resolve().parents[2]
    build_dir = project / "target" / "official-native"
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(build_dir)
    subprocess.run(
        ["cargo", "build", "--locked", "-p", "draw2d-official-examples",
         "--bin", "draw2d-official-examples"],
        cwd=project, env=env, check=True,
    )
    bundle = build_dir / "Draw2D Official Examples.app"
    executable_dir = bundle / "Contents" / "MacOS"
    executable_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(build_dir / "debug" / "draw2d-official-examples",
                 executable_dir / "draw2d-official-examples")
    with (bundle / "Contents" / "Info.plist").open("wb") as output:
        plistlib.dump({
            "CFBundleIdentifier": "org.novadraw.draw2d-official-examples",
            "CFBundleName": "Draw2D Official Examples",
            "CFBundleExecutable": "draw2d-official-examples",
            "CFBundlePackageType": "APPL",
            "CFBundleVersion": "0.1.0",
            "LSUIElement": False,
            "NSHighResolutionCapable": True,
        }, output)
    subprocess.run(["open", "-W", str(bundle)], check=True)


if __name__ == "__main__":
    main()
