"""Bounded build configuration for native text-dependency verification."""
import os
from pathlib import Path
import subprocess


def environment(target: Path, linker: str):
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target)
    env["CARGO_INCREMENTAL"] = "0"
    env["CARGO_BUILD_JOBS"] = "1"
    env["CARGO_PROFILE_DEV_DEBUG"] = "0"
    env["CARGO_PROFILE_TEST_DEBUG"] = "0"
    env["CC"] = "/Library/Developer/CommandLineTools/usr/bin/clang"
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = env["CC"]
    if linker == "rust-lld":
        rustc = Path(subprocess.check_output(
            ["rustup", "which", "--toolchain", "1.96.0", "rustc"], text=True,
        ).strip())
        lld = rustc.parent.parent / "lib/rustlib/aarch64-apple-darwin/bin/rust-lld"
        if not lld.is_file():
            raise RuntimeError(f"Rust Darwin linker missing: {lld}")
        env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = str(lld)
        # The legacy spelling is stable; `darwin-lld` requires unstable flags.
        env["RUSTFLAGS"] = (env.get("RUSTFLAGS", "") + " -C linker-flavor=ld64.lld").strip()
        env.setdefault("SDKROOT", "/Applications/Xcode.app/Contents/Developer/Platforms/"
                       "MacOSX.platform/Developer/SDKs/MacOSX15.0.sdk")
    return env
