"""Capture reproducible audit inventory and independent public-import probes."""
import hashlib
import json
import pathlib
import subprocess
from datetime import datetime, timezone

HERE = pathlib.Path(__file__).resolve().parent
PROJECT = HERE.parents[2]
REPO = pathlib.Path(
    subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], cwd=PROJECT, text=True
    ).strip()
)


def git(*args):
    return subprocess.check_output(["git", *args], cwd=PROJECT, text=True)


def capture():
    paths = []
    for directory in (
        "doc/design", "doc/adr", "doc/roadmap", "doc/parity", "doc/strategy",
        "novadraw", "novadraw-editor", "novadraw-inspector",
        "novadraw-backend-vello", "novadraw-platform-winit", "novadraw-platform-web",
        "benchmarks", "scripts", "tools/xtask", "book/src",
        "examples/native/node-editor-demo", "examples/web/web-validation",
    ):
        paths.extend(
            p for p in (PROJECT / directory).rglob("*")
            if p.is_file() and p.suffix in {".md", ".rs", ".toml", ".sh", ".html", ".css"}
        )
    paths.extend(PROJECT / p for p in (
        "AGENTS.md", "CLAUDE.md", "Cargo.toml", "Cargo.lock", "verification/suites.toml"
    ))
    baseline = {
        "captured_at": datetime.now(timezone.utc).isoformat(),
        "head": git("rev-parse", "HEAD").strip(),
        "repo_root": str(REPO),
        "project_root": str(PROJECT),
        "status": git("status", "--short"),
        "diff_stat": git("diff", "--stat", "HEAD", "--", "."),
        "scope_note": "Inventory is candidate scope, not a claim of line-by-line review.",
        "fingerprints": {
            str(p.relative_to(PROJECT)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(set(paths))
        },
    }
    (HERE / "baseline.json").write_text(json.dumps(baseline, ensure_ascii=False, indent=2) + "\n")
    print(f"Captured {len(baseline['fingerprints'])} source/document fingerprints")


def probe():
    deps = PROJECT / "target/debug/deps"
    libraries = sorted(deps.glob("libnovadraw-*.rlib"), key=lambda p: p.stat().st_mtime)
    if not libraries:
        raise SystemExit("Build novadraw before running import probes")
    library = libraries[-1]
    out = PROJECT / "target/verification/goal-alignment-imports"
    out.mkdir(parents=True, exist_ok=True)
    results = []
    for name in ("FigureNode", "UpdateManager", "RenderCommand", "PendingMutations", "EventDispatcher"):
        source = out / f"probe_{name.lower()}.rs"
        source.write_text(f"use novadraw::{name};\nfn main() {{ let _: Option<{name}> = None; }}\n")
        command = [
            "rustc", "--edition=2024", "--emit=metadata", str(source),
            "--extern", f"novadraw={library}", "-L", f"dependency={deps}",
            "--out-dir", str(out),
        ]
        result = subprocess.run(command, capture_output=True, text=True, cwd=PROJECT)
        results.append({
            "import": name, "command": command, "exit_code": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr,
        })
    (HERE / "public-import-probes.json").write_text(
        json.dumps(results, ensure_ascii=False, indent=2) + "\n"
    )
    for result in results:
        print(f"{result['import']}: {'importable' if result['exit_code'] == 0 else 'rejected'}")


if __name__ == "__main__":
    import sys
    if sys.argv[1:] == ["probe"]:
        probe()
    else:
        capture()
