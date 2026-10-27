"""Assemble snapshot findings and record source drift without overwriting old evidence."""
import hashlib
import json
import subprocess
from pathlib import Path

evidence = Path(__file__).resolve().parent
project = next(p for p in evidence.parents if (p / "novadraw-scene").is_dir())
repo = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"],
                                    cwd=project, text=True).strip())
baseline = json.loads((evidence / "baseline.json").read_text())
comments = []
for name in ["group_1", "group_1_supplement", "group_2", "group_3",
             "group_4", "group_5", "group_6"]:
    for line in (evidence / "group" / f"{name}.jsonl").read_text().splitlines():
        if line.strip():
            comments.append(json.loads(line))
for line in (evidence / "platform.jsonl").read_text().splitlines():
    if line.strip():
        item = json.loads(line)
        # Correct the helper name in the aggregate; keep original reviewer evidence intact.
        item["rationale"] = item["rationale"].replace(
            "Runtime::dispatch_wheel_changed", "Runtime::dispatch_scroll/dispatch_zoom")
        comments.append(item)
assert len(comments) == 17
# Apply the final cross-group decision only to derived aggregates.
comments[2]["severity"] = "P2"
comments[2]["confidence"] = 6
comments[2]["rationale"] += " 最终复核：需结合业务场景确认平台实际事件序列，降为待验证P2；后续整改状态独立记录。"
comments[14]["confidence"] = 10
for item in comments:
    path = repo / item["file"]
    assert 1 <= item["start_line"] <= item["end_line"] <= len(path.read_text().splitlines())
    assert item["title"] and item["rationale"]
(evidence / "comments.jsonl").write_text(
    "".join(json.dumps(c, ensure_ascii=False) + "\n" for c in comments))

# Five representative open snapshot findings, not the count of all semantic differences.
selected = [comments[i] for i in [3, 4, 6, 9, 12]]
for item in selected:
    item["rationale"] = (
        "【2026-09-16原审计快照；行号按原版本，后续整改状态另见状态页】"
        + item["rationale"])
(evidence / "final_comments.json").write_text(
    json.dumps(selected, ensure_ascii=False, indent=2) + "\n")

drift = []
for item in baseline["files"]:
    path = project / item["path"]
    current = hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None
    if current != item["sha256"]:
        drift.append({"path": item["path"], "baseline_sha256": item["sha256"],
                      "current_sha256": current})
record = {
    "original_date": baseline["date"],
    "delivery_date": "2026-09-17",
    "original_head": baseline["head"],
    "delivery_head": subprocess.check_output(["git", "rev-parse", "HEAD"],
                                              cwd=project, text=True).strip(),
    "snapshot_findings": len(comments),
    "final_severity": {"P1": 16, "P2": 1},
    "remediation_reported_closed": ["F01", "F02", "F03", "F16", "F17"],
    "remediation_reported_pending": 12,
    "selected_html_findings": len(selected),
    "inventory_files": len(baseline["files"]),
    "inventory_lines": sum(f["lines"] for f in baseline["files"]),
    "source_drift": drift,
    "docs_check": {"command": "cargo xtask docs", "exit_code": 0,
                   "result": "PASS verification/suites.toml: 27 commands, 2 profiles, 13 suites"},
    "note": "Original probe/test logs are historical. Current source has changed; no claim of current full regression.",
}
(evidence / "delivery-checks.json").write_text(
    json.dumps(record, ensure_ascii=False, indent=2) + "\n")
print(json.dumps({"findings": len(comments), "source_drift_files": len(drift)}))
