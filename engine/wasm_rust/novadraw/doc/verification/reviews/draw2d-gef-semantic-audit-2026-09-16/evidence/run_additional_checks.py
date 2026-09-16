"""Run manifest commands not covered by workspace.quality; preserve exit status."""
import json
import subprocess
import time
import tomllib
from pathlib import Path

project = Path(__file__).resolve().parents[5]
out = Path(__file__).resolve().parent
manifest = tomllib.loads((project / "verification/suites.toml").read_text())
selected = ["verify.update", "verify.event", "verify.scroll-pane",
            "replay.editor-g3", "replay.editor-g4", "replay.editor-g5.2",
            "replay.editor-g5.3", "replay.editor-g5.4", "replay.editor-g5.5", "web.build"]
results = []
for command_id in selected:
    spec = manifest["commands"][command_id]
    command = [spec["program"], *spec.get("args", [])]
    log_path = out / f"{command_id}.log"
    started = time.monotonic()
    print(f"Starting {command_id}", flush=True)
    with log_path.open("w") as log:
        completed = subprocess.run(command, cwd=project, stdout=log, stderr=subprocess.STDOUT)
    result = {"command_id": command_id, "exit_code": completed.returncode,
              "seconds": round(time.monotonic() - started, 2), "log": log_path.name}
    results.append(result)
    (out / "additional-checks.json").write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps(result), flush=True)
raise SystemExit(any(result["exit_code"] for result in results))
