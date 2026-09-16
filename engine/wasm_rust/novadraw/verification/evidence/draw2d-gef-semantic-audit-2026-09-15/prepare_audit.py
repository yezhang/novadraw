"""Record the full-file audit scope; never modify application sources."""
from pathlib import Path
import hashlib
import json
import subprocess

OUT = Path(__file__).resolve().parent
PROJECT = OUT.parents[4]
REPO = Path(subprocess.check_output(
    ["git", "rev-parse", "--show-toplevel"], cwd=PROJECT, text=True
).strip())
PREFIX = PROJECT.relative_to(REPO)
REFERENCE = Path("/Users/bytedance/Documents/code/GitHub/gef-classic")


def git(*args, cwd=REPO):
    return subprocess.check_output(["git", *args], cwd=cwd, text=True)


def group_for(path):
    s = path.as_posix()
    if s.startswith("novadraw-editor/"):
        return 5 if any(x in s for x in ("/model/", "/command/", "/part/", "g1_", "g2_", "g5_connection_projection")) else 6
    if "/connection/" in s or "/m9_" in s:
        return 4
    if any(x in s for x in ("/layout/", "/container/", "/runtime/update/", "/m5_", "/m8_", "/d2_", "/freeform", "/scroll")):
        return 2
    if any(s.startswith(x) for x in ("novadraw-render/", "novadraw-geometry/", "novadraw-math/", "novadraw-core/")):
        return 3
    if any(x in s for x in ("/figure/", "/runtime/resource", "render_recursive", "/m1_", "/m3_", "/m10_", "/d4_resource")):
        return 3
    return 1


paths = []
for name in ("novadraw", "novadraw-core", "novadraw-math", "novadraw-geometry",
             "novadraw-render", "novadraw-scene", "novadraw-editor", "novadraw-apps"):
    for folder in ("src", "tests"):
        paths.extend(sorted((PROJECT / name / folder).glob("**/*.rs")))
paths.extend(sorted((PROJECT / "apps/native/node-editor-demo/src").glob("**/*.rs")))
paths.extend(sorted((PROJECT / "apps/web/web-validation/src").glob("**/*.rs")))
groups = {i: [] for i in range(1, 7)}
manifest = []
for path in paths:
    rel = path.relative_to(PROJECT)
    data = path.read_bytes()
    group = group_for(rel)
    groups[group].append(str(PREFIX / rel))
    manifest.append({"path": str(PREFIX / rel), "group": group,
                     "sha256": hashlib.sha256(data).hexdigest(),
                     "lines": len(data.splitlines())})

shared = [str(PREFIX / p) for p in (
    "novadraw-scene/src/lib.rs", "novadraw-scene/src/figure/mod.rs",
    "novadraw-scene/src/graph/mod.rs", "novadraw-scene/src/runtime/runtime.rs",
    "novadraw-editor/src/lib.rs", "novadraw-editor/src/viewer/mod.rs")]
titles = {
    1: "Figure tree, identity, lifecycle, coordinates, events and platform input",
    2: "Layout, validation, update, damage, viewport, freeform and zoom",
    3: "Graphics, rendering, geometry, text, resources and reusable figures",
    4: "Connection, anchor, router, locator and self-loop",
    5: "GEF model, command history, EditPart lifecycle and projection",
    6: "GEF viewer, selection, request, policy, tool and interactive editing",
}
scope = ("scope: full_file\n\n用户指定评审范围: 当前 Novadraw 全量核心语义，"
         "包括未提交实现；不限于 diff。清单是检查范围，不代表逐行验证声明。\n\n")
(OUT / "review_files.md").write_text(
    scope + "\n".join(f"- {m['path']}" for m in manifest) + "\n")
(OUT / "review_groups.md").write_text(
    "# Semantic review groups\n\n分组依据：业务功能与调用链。公共边界重复共享；"
    "测试文件用于验证证据，未逐个重写测试。\n\n" +
    "\n\n".join(f"## Group {i}: {titles[i]}\n\n" +
                "\n".join(f"- {p}" for p in files) +
                "\n\nShared boundaries:\n" +
                "\n".join(f"- {p}" for p in shared)
                for i, files in groups.items()) + "\n")
(OUT / "baseline.json").write_text(json.dumps({
    "project": str(PROJECT), "repo": str(REPO), "head": git("rev-parse", "HEAD").strip(),
    "status": git("status", "--short", "--", str(PREFIX)),
    "diff_stat": git("diff", "--stat", "HEAD", "--", str(PREFIX)),
    "reference": str(REFERENCE),
    "reference_head": git("rev-parse", "HEAD", cwd=REFERENCE).strip(),
    "reference_source_diff": git("diff", "--stat", "HEAD", "--",
                                 "org.eclipse.draw2d/src", "org.eclipse.gef/src",
                                 cwd=REFERENCE),
    "files": manifest,
}, ensure_ascii=False, indent=2) + "\n")
print(f"Scope: {len(paths)} Rust source/test files; {sum(m['lines'] for m in manifest)} lines")
print("Groups:", {i: len(p) for i, p in groups.items()})
