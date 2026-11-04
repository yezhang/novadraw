"""Check current design navigation and explicitly registered document contracts."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import unicodedata
from urllib.parse import unquote, urlsplit


def prose(source: str) -> str:
    """Keep line numbers, but exclude fenced examples from metadata/link checks."""
    result = []
    fence = None
    for line in source.splitlines():
        match = re.match(r"\s*(`{3,}|~{3,})", line)
        if match:
            token = match.group(1)
            if fence is None:
                fence = token
            elif token[0] == fence[0] and len(token) >= len(fence):
                fence = None
            result.append("")
        else:
            result.append(line if fence is None else "")
    return "\n".join(result)


def anchors(source: str) -> set[str]:
    counts: dict[str, int] = {}
    result = set(re.findall(r'\b(?:id|name)=["\']([^"\']+)["\']', prose(source)))
    for line in prose(source).splitlines():
        match = re.match(r" {0,3}#{1,6}\s+(.+?)\s*#*\s*$", line)
        if not match:
            continue
        title = re.sub(r"\[([^]]+)\]\([^)]*\)", r"\1", match.group(1))
        title = re.sub(r"<[^>]*>", "", title).lower()
        slug = "".join(
            char for char in title
            if char in "_- " or unicodedata.category(char)[0] not in "PS"
        ).replace(" ", "-")
        number = counts.get(slug, 0)
        counts[slug] = number + 1
        result.add(slug if not number else f"{slug}-{number}")
    return result


def local_links(path: Path, source: str):
    for number, line in enumerate(prose(source).splitlines(), 1):
        for match in re.finditer(r"\]\(([^)]*)\)", line):
            raw = match.group(1).strip().split()
            if not raw:
                continue
            target = urlsplit(raw[0].strip("<>"))
            if target.scheme or target.netloc:
                continue
            resolved = (path.parent / unquote(target.path)).resolve() if target.path else path.resolve()
            yield number, resolved, unquote(target.fragment)


def section(source: str, title: str) -> str:
    lines = prose(source).splitlines()
    start = next((i for i, line in enumerate(lines) if line.lstrip("# ").strip() == title), None)
    if start is None:
        raise ValueError(f"missing section {title!r}")
    depth = len(lines[start]) - len(lines[start].lstrip("#"))
    end = next(
        (i for i in range(start + 1, len(lines))
         if re.match(rf"^#{{1,{depth}}}\s", lines[i])),
        len(lines),
    )
    return "\n".join(lines[start:end])


def check(root: Path, rules: dict) -> tuple[list[str], list[str], int]:
    root = root.resolve()
    designs = set((root / "doc/design").rglob("*.md"))
    sources = {path: path.read_text() for path in designs}
    retained_adrs = {
        match.group(1)
        for path in (root / "doc/adr").glob("adr-*.md")
        if (match := re.match(r"adr-(\d{3})-", path.name))
    }
    errors, warnings = [], []
    for path, source in sources.items():
        relative = str(path.relative_to(root))
        if path.name != "00-index.md" and "能力归属：" not in "\n".join(source.splitlines()[:25]):
            errors.append(f"{relative}: missing ability owner in first 25 lines")
        for number, target, fragment in local_links(path, source):
            if not target.exists():
                errors.append(f"{relative}:{number}: missing path {target}")
            elif fragment and target.suffix == ".md":
                # Historical files retain their baseline's navigation.
                if root / "doc/archive" in target.parents or root / "doc/verification/evidence" in target.parents:
                    continue
                if fragment not in anchors(target.read_text()):
                    errors.append(f"{relative}:{number}: missing anchor #{fragment} in {target.relative_to(root)}")
        for paragraph in prose(source).split("\n\n"):
            numbers = set(re.findall(r"ADR-(\d{3})", paragraph)) - retained_adrs
            if numbers and "](" not in paragraph:
                if not any(word in paragraph for word in ("原编号", "历史", "批准", "归并")):
                    warnings.append(f"{relative}: ADR reference without a topic link: {paragraph[:100]}")

    visited = set()
    pending = [root / "doc/design/00-index.md"]
    while pending:
        path = pending.pop()
        if path in visited or path not in designs:
            continue
        visited.add(path)
        pending.extend(target for _, target, _ in local_links(path, sources[path]) if target in designs)
    errors.extend(f"{path.relative_to(root)}: unreachable from design index" for path in sorted(designs - visited))

    for relative in rules.get("protocol_examples", []):
        source = (root / relative).read_text()
        if "示例性质：协议示意" not in "\n".join(source.splitlines()[:25]):
            errors.append(f"{relative}: missing protocol example classification")
        if "规范效力：" not in "\n".join(source.splitlines()[:25]):
            errors.append(f"{relative}: missing explicit normative effect")
        if re.search(r"^```rust\s*$", source, re.MULTILINE):
            errors.append(f"{relative}: unclassified Rust example; use rust,ignore for protocol sketches")

    for entry in rules.get("accepted_scope", []):
        source = (root / entry["path"]).read_text()
        if "效力范围：" not in "\n".join(source.splitlines()[:25]):
            errors.append(f"{entry['path']}: missing partial acceptance scope")
        for target in entry["links"]:
            if target not in source:
                errors.append(f"{entry['path']}: missing accepted topic link {target}")

    for entry in rules.get("supersessions", []):
        try:
            body = section((root / entry["path"]).read_text(), entry["section"])
            if entry["link"] not in body:
                errors.append(f"{entry['path']}: superseding topic missing in {entry['section']}")
            for pattern in entry.get("reject", []):
                if re.search(pattern, body):
                    errors.append(f"{entry['path']}: retired clause remains in {entry['section']}")
        except ValueError as error:
            errors.append(f"{entry['path']}: {error}")

    for entry in rules.get("baselines", []):
        source = (root / entry["review"]).read_text()
        for target in entry["required_links"]:
            if target not in source:
                errors.append(f"{entry['review']}: missing baseline role link {target}")
        if entry.get("missing") and "完整被审版本缺失" not in source:
            errors.append(f"{entry['review']}: missing baseline limitation")

    for relative in rules.get("manifests", []):
        path = root / relative
        for filename, entry in json.loads(path.read_text()).items():
            target = path.parent / filename
            if not target.is_file() or hashlib.sha256(target.read_bytes()).hexdigest() != entry["sha256"]:
                errors.append(f"{relative}: baseline bytes changed: {filename}")
    return errors, warnings, len(designs)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    rules = json.loads((args.root / "tools/scripts/design_doc_rules.json").read_text())
    errors, warnings, count = check(args.root, rules)
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    print(f"Design checks: {count} Markdown files, {len(errors)} errors, "
          f"{len(warnings)} ADR reference candidates for manual review")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
