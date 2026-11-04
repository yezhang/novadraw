"""Regression fixtures for document role and navigation checks."""

import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from check_design_docs import anchors, check, local_links, section


class DesignDocTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.design = self.root / "doc/design"
        self.design.mkdir(parents=True)
        self.page = self.design / "contract.md"
        self.page.write_text(
            "# 合同\n\n类型：`normative-design`\n\n能力归属：Core。\n"
            "规范效力：已接受。\n示例性质：协议示意。\n\n"
            "## 低 miter\n\n[修订](revision.md#规则)\n\n```rust,ignore\nfn role();\n```\n"
        )
        (self.design / "revision.md").write_text(
            "# 修订\n\n类型：`normative-design`\n\n能力归属：Core。\n\n## 规则\n\n有限非负。\n"
        )
        (self.design / "00-index.md").write_text(
            "# 入口\n\n[合同](contract.md) [修订](revision.md)\n"
        )
        self.rules = {
            "protocol_examples": ["doc/design/contract.md"],
            "supersessions": [{
                "path": "doc/design/contract.md", "section": "低 miter",
                "link": "revision.md", "reject": ["至少为 1"],
            }],
        }

    def test_valid_fixture_and_scoped_supersession(self):
        self.assertEqual(check(self.root, self.rules)[0], [])
        with self.page.open("a") as stream:
            stream.write("\n## 历史\n\n旧规则至少为 1。\n")
        self.assertEqual(check(self.root, self.rules)[0], [])
        source = self.page.read_text().replace("[修订]", "至少为 1。[修订]")
        self.page.write_text(source)
        self.assertTrue(any("retired clause" in error for error in check(self.root, self.rules)[0]))

    def test_missing_owner_and_unclassified_example(self):
        self.page.write_text(self.page.read_text().replace("能力归属：Core。", "").replace("rust,ignore", "rust"))
        errors = check(self.root, self.rules)[0]
        self.assertTrue(any("ability owner" in error for error in errors))
        self.assertTrue(any("unclassified Rust" in error for error in errors))

    def test_missing_anchor_and_unreachable_page(self):
        self.page.write_text(self.page.read_text().replace("#规则", "#不存在"))
        (self.design / "orphan.md").write_text("# 孤立\n\n能力归属：Core。\n")
        errors = check(self.root, self.rules)[0]
        self.assertTrue(any("missing anchor" in error for error in errors))
        self.assertTrue(any("unreachable" in error for error in errors))

    def test_unicode_duplicate_headings_and_fences(self):
        source = "# UTF-8 与 identity\n## UTF-8 与 identity\n```text\n## 非标题\n```\n"
        self.assertEqual(anchors(source), {"utf-8-与-identity", "utf-8-与-identity-1"})
        self.assertEqual(anchors("## 3. 执行顺序"), {"3-执行顺序"})
        self.assertEqual(anchors('## `值` / 权限\n<a id="custom"></a>'), {"值--权限", "custom"})

    def test_fragment_only_and_web_links(self):
        links = list(local_links(self.page, "[当前](#低-miter) [web](https://example.com)"))
        self.assertEqual(links, [(1, self.page.resolve(), "低-miter")])

    def test_historical_anchor_is_exempt_but_path_is_not(self):
        archive = self.root / "doc/archive"
        archive.mkdir()
        (archive / "old.md").write_text("# 原文")
        self.page.write_text(self.page.read_text() + "\n[旧稿](../archive/old.md#旧锚点)\n")
        self.assertEqual(check(self.root, self.rules)[0], [])
        (archive / "old.md").unlink()
        self.assertTrue(any("missing path" in error for error in check(self.root, self.rules)[0]))

    def test_baseline_roles_and_manifest_integrity(self):
        evidence = self.root / "doc/verification/evidence"
        evidence.mkdir(parents=True)
        original = evidence / "original.txt"
        original.write_bytes(b"frozen bytes\n")
        manifest = evidence / "manifest.json"
        manifest.write_text(json.dumps({"original.txt": {"sha256": hashlib.sha256(original.read_bytes()).hexdigest()}}))
        review = self.root / "review.md"
        review.write_text("完整被审版本缺失。[草案](doc/verification/evidence/original.txt)")
        rules = copy.deepcopy(self.rules)
        rules["manifests"] = ["doc/verification/evidence/manifest.json"]
        rules["baselines"] = [{
            "review": "review.md", "missing": True, "required_links": ["original.txt"],
        }]
        self.assertEqual(check(self.root, rules)[0], [])
        review.write_text("[修订](current.md)")
        original.write_bytes(b"changed")
        errors = check(self.root, rules)[0]
        self.assertTrue(any("baseline limitation" in error for error in errors))
        self.assertTrue(any("baseline role" in error for error in errors))
        self.assertTrue(any("baseline bytes changed" in error for error in errors))

    def test_partial_acceptance_requires_scope_and_topics(self):
        rules = copy.deepcopy(self.rules)
        rules["accepted_scope"] = [{"path": "doc/design/contract.md", "links": ["revision.md"]}]
        self.assertTrue(any("partial acceptance" in error for error in check(self.root, rules)[0]))
        self.page.write_text(self.page.read_text().replace("规范效力：", "效力范围：候选未生效。\n规范效力："))
        self.assertEqual(check(self.root, rules)[0], [])
        self.assertIn("revision.md", section(self.page.read_text(), "低 miter"))


if __name__ == "__main__":
    unittest.main()
