"""Merge review candidates, apply independent decisions, and validate anchors."""
import json
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
PROJECT = HERE.parents[2]
REPO = PROJECT.parents[2]
candidates = []
for group in range(1, 5):
    source = HERE / f"group/group_{group}.jsonl"
    for index, line in enumerate(source.read_text().splitlines(), 1):
        if not line.strip():
            continue
        item = json.loads(line)
        item["audit_id"] = f"G{group}-{index:02}"
        candidates.append(item)

(HERE / "comments.jsonl").write_text(
    "".join(json.dumps(item, ensure_ascii=False) + "\n" for item in candidates)
)

reviewed = []
for item in candidates:
    item = dict(item)
    item["review_status"] = "retained"
    if item["audit_id"] == "G1-02":
        item["severity"] = "P2"
        item["review_note"] = "Static O(N²) work confirmed; no measured end-to-end regression."
    elif item["audit_id"] == "G2-01":
        item["confidence"] = 7
        item["review_note"] = "Confirmed borrow lifetime; focused DOM blur path not replayed."
    elif item["audit_id"] == "G2-05":
        item["confidence"] = 6
        item["review_status"] = "needs_platform_evidence"
        item["review_note"] = "Need evidence of DOM value restored after ClearValue."
    elif item["audit_id"] == "G2-06":
        item["review_status"] = "withdrawn"
        item["review_note"] = "Winit Windows get_agnostic_mods filters detected AltGr modifiers."
    elif item["audit_id"] == "G1-01":
        item["rationale"] = item["rationale"].replace(
            "未运行 probe。",
            "主 agent 最小 probe 已复现：首帧 child=10x10、root_valid=false；显式 revalidate 后 child=100x100。"
        )
    elif item["audit_id"] == "G1-04":
        item["rationale"] += " 主 agent probe 已确认首次 submission 的单个 Figure paint_calls=2。"
    reviewed.append(item)

(HERE / "reviewed-candidates.json").write_text(
    json.dumps(reviewed, ensure_ascii=False, indent=2) + "\n"
)

selected_ids = {"G1-01", "G4-01", "G2-02", "G3-01", "G2-01"}
selected = [item for item in reviewed if item["audit_id"] in selected_ids]
selected.sort(key=lambda item: (
    {"P0": 0, "P1": 1, "P2": 2}[item["severity"]], -item["confidence"], item["audit_id"]
))
for item in selected:
    source = REPO / item["file"]
    lines = source.read_text().splitlines()
    if item["audit_id"] == "G3-01":
        command_line = next(i + 1 for i, text in enumerate(lines) if "policy.command(host, request, &self.model)?" in text)
        item["start_line"] = command_line - 2
        item["end_line"] = command_line + 1
        item["anchor_note"] = "Policy body unchanged; line shifted by concurrent caret presentation edits."
    assert 1 <= item["start_line"] <= item["end_line"] <= len(lines), item
    item["code_snippet"] = "\n".join(lines[item["start_line"] - 1:item["end_line"]])
assert len(selected) == 5
(HERE / "final_comments.json").write_text(
    json.dumps(selected, ensure_ascii=False, indent=2) + "\n"
)
print(f"{len(candidates)} raw candidates; {len(selected)} priority findings; anchors valid")
for item in reviewed:
    print(item["audit_id"], item["severity"], item["confidence"], item["review_status"])
