"""Check new report links, evidence JSON, source anchors, and whitespace."""
import hashlib
import json
import pathlib
import re
import subprocess
from urllib.parse import unquote

HERE = pathlib.Path(__file__).resolve().parent
PROJECT = HERE.parents[2]
documents = [
    PROJECT / "doc/verification/reviews/goal-design-code-audit-2026-09-30.md",
    PROJECT / "doc/roadmap/goal-alignment-adjustment-plan-2026-09-30.md",
]
errors = []
link_count = 0
for document in documents:
    for target in re.findall(r"\]\(([^)]+)\)", document.read_text()):
        if "://" in target or target.startswith("#"):
            continue
        path, _, fragment = unquote(target).partition("#")
        resolved = (document.parent / path).resolve()
        link_count += 1
        if not resolved.exists():
            errors.append(f"{document.name}: missing {target}")
        elif fragment.startswith("L"):
            numbers = [int(n) for n in re.findall(r"\d+", fragment)]
            if not numbers or max(numbers) > len(resolved.read_text().splitlines()):
                errors.append(f"{document.name}: out-of-range {target}")
for path in HERE.rglob("*.json"):
    json.loads(path.read_text())
for path in HERE.rglob("*.jsonl"):
    for line in path.read_text().splitlines():
        if line.strip():
            json.loads(line)
for path in [*documents, *HERE.rglob("*.md"), *HERE.rglob("*.py"), *HERE.rglob("*.rs")]:
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if line.rstrip() != line:
            errors.append(f"{path.name}:{number}: trailing whitespace")
baseline = json.loads((HERE / "baseline.json").read_text())
changed = []
for name, digest in baseline["fingerprints"].items():
    path = PROJECT / name
    if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        changed.append(name)
result = {
    "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=PROJECT, text=True).strip(),
    "links_checked": link_count,
    "errors": errors,
    "changed_since_capture": changed,
    "intentional_changes": ["doc/roadmap/00-index.md"],
    "note": "Generated dist may change externally; new audit files are outside baseline inventory.",
}
(HERE / "delivery-checks.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
print(json.dumps(result, ensure_ascii=False, indent=2))
raise SystemExit(bool(errors))
