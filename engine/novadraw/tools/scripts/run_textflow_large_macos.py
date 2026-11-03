"""Build and present this checkout's TextFlowLarge research window."""
import argparse
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
from text_admission_build import environment


def main():
    if sys.platform != "darwin":
        raise SystemExit("This launcher requires macOS.")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain")
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--clt", action="store_true",
                        help="Use the existing ARM64 text-build CLT compiler configuration")
    parser.add_argument("app_args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[2]
    target = (args.target_dir or Path(os.environ.get(
        "CARGO_TARGET_DIR", project / "target" / "textflow-large-native"))).resolve()
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target)
    if args.clt:
        env = environment(target, "clang")
        env.setdefault("SDKROOT", subprocess.check_output(
            ["xcrun", "--sdk", "macosx", "--show-sdk-path"], text=True).strip())
    # Bound debug compilation, preserving explicit caller settings.
    for key, value in {"CARGO_INCREMENTAL": "0", "CARGO_BUILD_JOBS": "1",
                       "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0"}.items():
        env.setdefault(key, value)
    cargo = ["cargo", *([f"+{args.toolchain}"] if args.toolchain else [])]
    subprocess.run([*cargo, "build", "--locked", "-p", "textflow-large-app"],
                   cwd=project, env=env, check=True)
    bundle = target / "TextFlowLarge.app"
    executable_dir = bundle / "Contents" / "MacOS"
    executable_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(target / "debug" / "textflow-large-app", executable_dir / "textflow-large-app")
    with (bundle / "Contents" / "Info.plist").open("wb") as output:
        plistlib.dump({
            "CFBundleIdentifier": "org.novadraw.textflow-large-research",
            "CFBundleName": "TextFlowLarge",
            "CFBundleExecutable": "textflow-large-app",
            "CFBundlePackageType": "APPL",
            "CFBundleVersion": "0.1.0",
            "LSUIElement": False,
            "NSHighResolutionCapable": True,
        }, output)
    app_args = args.app_args[1:] if args.app_args[:1] == ["--"] else args.app_args
    # LaunchServices does not preserve cwd; always resolve the app's artifact directory.
    if "--output" in app_args:
        index = app_args.index("--output") + 1
        if index >= len(app_args):
            raise SystemExit("--output needs a directory")
        app_args[index] = str((project / app_args[index]).resolve())
    else:
        app_args.extend(["--output", str(project / "target/verification/textflow-large-app")])
    subprocess.run(["open", "-n", "-W", str(bundle), "--args", *app_args],
                   cwd=project, check=True)


if __name__ == "__main__":
    main()
