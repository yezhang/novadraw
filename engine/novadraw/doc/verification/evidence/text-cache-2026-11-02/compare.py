import hashlib
import json
import math
from pathlib import Path
import statistics
import subprocess
import sys

OUTPUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/novadraw-text-cache-comparison")
OUTPUT.mkdir(exist_ok=False)
BINARIES = {
    "before": Path("/tmp/novadraw-text-cache-before"),
    "after": Path("/tmp/novadraw-text-cache-after"),
}
WARMUP = 8
SAMPLES = 32
SESSIONS = 3
results = {side: [] for side in BINARIES}
for session in range(SESSIONS):
    order = ("before", "after") if session % 2 == 0 else ("after", "before")
    for side in order:
        command = [str(BINARIES[side]), str(WARMUP + SAMPLES), "extended", "original"]
        completed = subprocess.run(command, capture_output=True, text=True, check=True)
        (OUTPUT / f"{session}-{side}.jsonl").write_text(completed.stdout)
        (OUTPUT / f"{session}-{side}.stderr").write_text(completed.stderr)
        rows = [json.loads(line) for line in completed.stdout.splitlines()]
        assert not any("failure" in row.get("status", "") for row in rows)
        measured = [row for row in rows if row.get("sample", -1) >= WARMUP]
        ok = [row for row in measured if row.get("status") == "ok"]
        assert len(ok) == SAMPLES
        extended = [row for row in measured if row.get("status") == "extended"]
        assert len(extended) == SAMPLES
        results[side].append(measured)
        print(session, side, "passed", flush=True)

def percentile(values, fraction):
    return sorted(values)[math.ceil(len(values) * fraction) - 1]

def metrics(rows):
    out = {}
    for row in rows:
        if row["status"] == "ok":
            for key in ("initial_ns", "append_e2e_ns", "stable_ns"):
                out.setdefault(key, []).append(row[key])
        elif row["status"] == "extended":
            for key in ("scroll_ns", "resize_ns", "record_ns"):
                if key in row:
                    out.setdefault(key, []).append(row[key])
        elif row["status"] == "work" and row.get("phase", "").endswith("/flow-paragraph-lookup"):
            out.setdefault(row["phase"], []).append(row["inclusive_ns"])
    return {
        key: {"samples": len(values), "p50_ns": statistics.median(values),
              "p95_ns": percentile(values, .95)}
        for key, values in out.items()
    }

for session in range(SESSIONS):
    def semantics(side):
        return [
            {key: value for key, value in row.items()
             if key not in {"construct_ns", "initial_ns", "dispatch_ns", "append_update_ns",
                            "append_e2e_ns", "stable_ns", "scroll_ns", "resize_ns",
                            "record_ns", "inclusive_ns", "exclusive_ns",
                            "contribution_ns", "styled_layout_ns", "source_geometry_ns", "build_ns"}}
            for row in results[side][session]
            if row["status"] in {"ok", "extended"} or row.get("phase") in {"initial", "append", "initial-cache", "append-cache"}
        ]
    assert semantics("before") == semantics("after"), "fixture or work counters changed"

summary = {
    "sessions": SESSIONS, "warmup_per_session": WARMUP, "samples_per_session": SAMPLES,
    "workload": "original TextFlowLarge, exact layout, append Z, scroll and width resize",
    "scope": "CPU only; diagnostics enabled; no GPU or present; external load uncontrolled",
    "toolchain": subprocess.check_output(["rustc", "-Vv"], text=True),
    "binaries": {side: {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                 for side, path in BINARIES.items()},
    "semantic_rows_and_work_counters_equal": True,
    "per_session": {side: [metrics(rows) for rows in sessions] for side, sessions in results.items()},
}
(OUTPUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
