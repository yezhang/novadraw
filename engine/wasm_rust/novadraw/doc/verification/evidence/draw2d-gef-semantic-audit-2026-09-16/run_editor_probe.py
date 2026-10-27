"""Compile audit-only public API examples against the verified workspace build."""
import hashlib
import json
import subprocess
from pathlib import Path

project = Path(__file__).resolve().parents[5]
out = Path(__file__).resolve().parent
source = out / "editor_probe.rs"
deps = project / "target/debug/deps"
binary = project / "target/verification/semantic-audit-2026-09-16/editor-probe"
binary.parent.mkdir(parents=True, exist_ok=True)
command = ["rustc", "--edition=2024", str(source), "-L", f"dependency={deps}"]
libraries = {}
for name in ["novadraw_editor", "novadraw_scene", "novadraw_geometry"]:
    library = max(deps.glob(f"lib{name}-*.rlib"), key=lambda p: p.stat().st_mtime_ns)
    command.extend(["--extern", f"{name}={library}"])
    libraries[name] = {"path": str(library.relative_to(project)),
                       "sha256": hashlib.sha256(library.read_bytes()).hexdigest()}
command.extend(["-o", str(binary)])
build = subprocess.run(command, cwd=project, text=True, capture_output=True)
(out / "editor-probe-build.log").write_text(build.stdout + build.stderr)
record = {"source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
          "libraries": libraries, "build_exit_code": build.returncode}
print(build.stderr, end="")
if build.returncode == 0:
    run = subprocess.run([str(binary)], cwd=project, text=True, capture_output=True)
    (out / "editor-probe.log").write_text(run.stdout + run.stderr)
    record["exit_code"] = run.returncode
    print(run.stdout + run.stderr, end="")
(out / "editor-probe.json").write_text(json.dumps(record, indent=2) + "\n")
raise SystemExit(record.get("exit_code", build.returncode))
