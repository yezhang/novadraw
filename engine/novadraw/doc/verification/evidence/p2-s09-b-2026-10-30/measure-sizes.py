"""Run after creating a detached 27dc2274 worktree at target/verification/s09-b-baseline."""
import gzip
import hashlib
import json
import pathlib
import shutil
import subprocess

evidence = pathlib.Path(__file__).resolve().parent
root = evidence.parents[3]
baseline = root / "target/verification/s09-b-baseline/engine/novadraw"
work = root / "target/verification/s09-b-cost"
work.mkdir(exist_ok=True)
rows = []
source = (evidence / "cost-probe.rs").read_bytes()
for side, checkout in [("before", baseline), ("after", root)]:
    probe = work / side
    probe.mkdir(exist_ok=True)
    (probe / "lib.rs").write_bytes(source)
    (probe / "Cargo.toml").write_text(f'''[package]
name = "novadraw-service-failure-consumer"
version = "0.1.0"
edition = "2024"
[workspace]
[lib]
path = "lib.rs"
crate-type = ["cdylib"]
[features]
linked-inspector = ["dep:novadraw-inspector"]
probe-inspector = ["linked-inspector"]
[dependencies]
novadraw = {{ path = "{checkout / 'crates/novadraw'}", default-features = false }}
novadraw-inspector = {{ path = "{checkout / 'crates/novadraw-inspector'}", optional = true }}
[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
panic = "abort"
strip = true
''')
    shutil.copy2(root / "tests/consumers/service-failure/Cargo.lock", probe / "Cargo.lock")
    for variant, features in [("off", []), ("unused", ["linked-inspector"]), ("inspector", ["probe-inspector"])]:
        for platform in ["native", "wasm"]:
            command = ["cargo", "build", "--manifest-path", str(probe / "Cargo.toml"),
                       "--release", "--lib", "--offline", "--locked",
                       "--target-dir", str(work / "target")]
            if features:
                command += ["--features", ",".join(features)]
            if platform == "wasm":
                command += ["--target", "wasm32-unknown-unknown"]
            name = f"{side}-{variant}-{platform}"
            with (evidence / f"size-{name}.log").open("w") as log:
                log.write(" ".join(command) + "\n")
                log.flush()
                subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT, check=True)
            artifact = (work / "target/release/libnovadraw_service_failure_consumer.dylib"
                        if platform == "native" else
                        work / "target/wasm32-unknown-unknown/release/novadraw_service_failure_consumer.wasm")
            data = artifact.read_bytes()
            shutil.copy2(artifact, work / (name + artifact.suffix))
            rows.append(dict(side=side, variant=variant, platform=platform,
                             bytes=len(data), gzip_bytes=len(gzip.compress(data, mtime=0)),
                             sha256=hashlib.sha256(data).hexdigest(), command=command))
receipt = dict(baseline="27dc2274", rustc=subprocess.check_output(["rustc", "-Vv"], text=True),
               source_sha256=hashlib.sha256(source).hexdigest(), rows=rows)
(evidence / "sizes.json").write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps(receipt, indent=2))
