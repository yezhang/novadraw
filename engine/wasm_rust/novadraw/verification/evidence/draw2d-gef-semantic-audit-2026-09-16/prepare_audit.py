"""Record the working-tree audit scope without changing product sources."""
import hashlib
import json
import subprocess
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[5]
OUT = Path(__file__).resolve().parent
REFERENCE = Path("/Users/bytedance/Documents/code/GitHub/gef-classic")
GROUPS = {
    1: "Figure topology, identity, lifecycle, search, events, focus, notifications",
    2: "Layout, validation, UpdateManager, damage, viewport, freeform, zoom",
    3: "Geometry, Graphics, recursive paint, backend, resources, text, widgets",
    4: "Connection Figure, Anchor, Router, Locator and runtime atomicity",
    5: "Model, CommandStack, EditPart, registries and projection",
    6: "Domain, Tool, Request, EditPolicy, targeting, feedback and auto-expose",
}


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True)


def groups(path):
    name = path.as_posix()
    if name.startswith(("novadraw-editor/src/model", "novadraw-editor/src/command")):
        return [5]
    if name.startswith(("novadraw-editor/src/part", "novadraw-editor/src/viewer")):
        return [5, 6]
    if name.startswith("novadraw-editor"):
        return [5] if "/tests/g1" in name or "/tests/g2" in name or "projection" in name else [6]
    if name.startswith("novadraw-scene/src/connection") or "/tests/m9" in name:
        return [4]
    if name.startswith(("novadraw-scene/src/layout", "novadraw-scene/src/container",
                        "novadraw-scene/src/runtime/update")):
        return [2]
    if name.startswith(("novadraw-render", "novadraw-geometry", "novadraw-math",
                        "novadraw-core", "novadraw-scene/src/figure")):
        return [3]
    if "render_recursive" in name or "/runtime/resource" in name:
        return [3]
    if "/tests/m5" in name or "/tests/m8" in name or "/tests/d2" in name:
        return [2]
    if "/tests/m10" in name:
        return [3]
    if name in ("novadraw-scene/src/graph/mod.rs", "novadraw-scene/src/runtime/runtime.rs",
                "novadraw-scene/src/lib.rs", "novadraw-editor/src/lib.rs"):
        return list(GROUPS)
    return [1]


files = []
for base in sorted(PROJECT.glob("novadraw*")):
    if base.is_dir():
        for path in sorted(base.rglob("*.rs")):
            if "target" in path.parts:
                continue
            relative = path.relative_to(PROJECT)
            data = path.read_bytes()
            files.append({"path": relative.as_posix(), "groups": groups(relative),
                          "sha256": hashlib.sha256(data).hexdigest(),
                          "lines": len(data.splitlines())})
baseline = {
    "date": "2026-09-16",
    "project": str(PROJECT),
    "repo": git(PROJECT, "rev-parse", "--show-toplevel").strip(),
    "head": git(PROJECT, "rev-parse", "HEAD").strip(),
    "scope": "Current working tree, including untracked Rust source; not diff-only",
    "status": git(PROJECT, "status", "--short", "--", "."),
    "reference_head": git(REFERENCE, "rev-parse", "HEAD").strip(),
    "reference_source_diff": git(
        REFERENCE, "diff", "HEAD", "--", "org.eclipse.draw2d/src",
        "org.eclipse.gef/src", "org.eclipse.draw2d.doc.isv/guide-src",
        "org.eclipse.gef.doc.isv/guide-src"),
    "inventory_note": "Inventory defines scope; it is not a claim of line-by-line review.",
    "files": files,
}
(OUT / "baseline.json").write_text(json.dumps(baseline, ensure_ascii=False, indent=2) + "\n")
(OUT / "review_files.md").write_text(
    "scope: full_file\n\n用户指定评审范围: 当前 Novadraw 全项目核心语义，含未跟踪源码。\n"
    "diff_files.md 只记录初始变更，不能限制本次审计。\n\n" +
    "\n".join(f"- {x['path']} ({x['lines']} lines)" for x in files) + "\n")
(OUT / "review_groups.md").write_text("\n\n".join(
    f"## Group {number}: {title}\n\n按语义与调用链分组；共享 Runtime/Graph/接口跨组。\n\n" +
    "\n".join(f"- {x['path']}" for x in files if number in x["groups"])
    for number, title in GROUPS.items()) + "\n")
print(json.dumps({"head": baseline["head"], "reference_head": baseline["reference_head"],
                  "inventory_files": len(files), "inventory_lines": sum(x["lines"] for x in files),
                  "reference_sources_clean": not baseline["reference_source_diff"]}, indent=2))
