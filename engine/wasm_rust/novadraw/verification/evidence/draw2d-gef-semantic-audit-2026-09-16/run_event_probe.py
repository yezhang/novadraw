"""Recompile the existing public-API reproducer against this audit's build."""
import hashlib
import json
import subprocess
from pathlib import Path

project = Path(__file__).resolve().parents[5]
out = Path(__file__).resolve().parent
source = out.parents[1] / "draw2d-gef-semantic-audit-2026-09-15/evidence/event_probe.rs"
deps = project / "target/debug/deps"
library = max(deps.glob("libnovadraw_scene-*.rlib"), key=lambda p: p.stat().st_mtime_ns)
binary = project / "target/verification/semantic-audit-2026-09-16/event-probe"
binary.parent.mkdir(parents=True, exist_ok=True)
command = ["rustc", "--edition=2024", str(source), "-L", f"dependency={deps}",
           "--extern", f"novadraw_scene={library}", "-o", str(binary)]
build = subprocess.run(command, cwd=project, text=True, capture_output=True)
(out / "event-probe-build.log").write_text(build.stdout + build.stderr)
record = {"source": str(source.relative_to(project)),
          "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
          "library": str(library.relative_to(project)),
          "library_sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
          "build_exit_code": build.returncode}
if build.returncode == 0:
    run = subprocess.run([str(binary)], cwd=project, text=True, capture_output=True)
    (out / "event-probe.log").write_text(run.stdout + run.stderr)
    record["exit_code"] = run.returncode
    print(run.stdout, end="")
(out / "event-probe.json").write_text(json.dumps(record, indent=2) + "\n")
raise SystemExit(build.returncode)
