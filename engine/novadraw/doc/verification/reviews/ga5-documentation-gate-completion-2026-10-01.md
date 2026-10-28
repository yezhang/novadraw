# GA-5 文档与门禁完成记录

类型：`verification`

日期：2026-10-01

状态：`complete`

实现基线：`ff77242`

## 1. 文档漂移修正

- component update 与 Inspector 分离规范效力和实现状态；
- Inspector protocol package 改为按独立版本兼容或多消费者证据再拆，符合 ADR-023；
- Draw2D parity 的旧 crate 路径改为当前 `novadraw` 模块路径；
- FigureTree 与 Viewport/ScrollPane 私有 mutator 改为公开只读 handle + Runtime editor；
- 产品清单同步 P2-C02 ShortestPath 已完成事实，同时保留 M9 原始交付范围；
- Book 中把 suite ID 的执行方式统一为 `cargo xtask verify <suite-id>`；
- `basic-widgets` 与 Draw2D text/label 参考文档使用规范类型词汇。

历史 review 继续保存对应快照事实，没有批量改写为当前实现状态。

## 2. Docs gate

`cargo xtask docs` 现在验证：

1. `doc/` 文档必须声明受支持的类型；
2. `doc/` 与 `book/src/` 的本地 Markdown 链接必须存在；
3. 文档中的 `cargo xtask` command、profile、suite prefix 与 manual step 必须有效；
4. 设计索引的“核心设计”和“非规范提案”必须指向匹配类型；
5. 逐符号 facade doctest 必须通过，禁止 root 导入不能由其他失败符号掩盖；
6. 每个 contract/application suite 必须声明已存在于 Draw2D/GEF parity ledger 的
   `api_semantics`，重复、未知或空映射均失败。

当前覆盖 208 份 Markdown、26 个行为 suite、10 个独立 compile-fail facade probe。
suite 的 `documents` 与 `artifacts` 继续提供语义到证据的结构化落点。

## 3. 验证

```text
cargo test -p xtask: 5 passed
cargo clippy -p xtask -- -D warnings: PASS
cargo xtask docs: PASS
cargo xtask check --quick: PASS
git diff --check: PASS
```

## 4. 剩余边界

- Markdown gate 校验本地目标存在，不声称外部 URL 在线可达；
- public API probe 聚焦 facade 分层，行为语义仍由对应 contract suite 与人工评审负责；
- GA-2 的真实 present/input-to-present、GA-3 的 Windows/Linux 原生 runner 仍未验证。

GA-5 的文档路径、分类、命令、公开 facade 与 parity-to-suite 映射已闭合。
