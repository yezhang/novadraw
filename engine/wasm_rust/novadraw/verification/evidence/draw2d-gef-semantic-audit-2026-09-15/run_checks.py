"""Run existing project gates once and retain their exact output."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import time

out = Path(__file__).resolve().parent
project = out.parents[4]
label = sys.argv[1] + "-" if len(sys.argv) > 1 else ""
baseline = json.loads((out / "baseline.json").read_text())
repo = Path(baseline["repo"])
before = {entry["path"]: hashlib.sha256((repo / entry["path"]).read_bytes()).hexdigest()
          for entry in baseline["files"]}
if label:
    (out / f"{label}source.patch").write_text(subprocess.check_output(
        ["git", "diff", "HEAD", "--", *before], cwd=repo, text=True))
commands = [
    ("fmt", ["cargo", "fmt", "--all", "--", "--check"]),
    ("check", ["cargo", "check", "--workspace"]),
    ("clippy", ["cargo", "clippy", "--workspace", "--", "-D", "warnings"]),
    ("test", ["cargo", "test", "--workspace"]),
]
results = []
for name, command in commands:
    start = time.monotonic()
    print(f"START {name}", flush=True)
    with (out / f"{label}{name}.log").open("w") as log:
        process = subprocess.run(command, cwd=project, stdout=log, stderr=subprocess.STDOUT)
    result = {"name": name, "command": command, "exit_code": process.returncode,
              "seconds": round(time.monotonic() - start, 2), "log": f"{label}{name}.log"}
    results.append(result)
    (out / f"{label}checks.json").write_text(json.dumps(results, indent=2) + "\n")
    print(f"END {name}: {process.returncode} ({result['seconds']}s)", flush=True)
after = {path: hashlib.sha256((repo / path).read_bytes()).hexdigest() for path in before}
(out / f"{label}source-fingerprints.json").write_text(json.dumps({
    "before": before, "after": after,
    "changed_during_checks": [path for path in before if before[path] != after[path]],
}, indent=2) + "\n")
