# S08 设计与检查记录

类型：`verification-record`

日期：2026-10-29

状态：设计草案已形成，待评审；没有 S08 实现或行为验收结论。

## 本次交付

颜色文本修复已提交为 `9000486939056d7839fdfca304bbda00dd163bd7`，
标题为 `fix(core): 拒绝非法颜色文本并消除 UTF-8 解析崩溃`。
该提交包含颜色实现、三项外部回归及既有验证记录，归 ADR-020。

随后形成 [ADR-056](../../adr/adr-056-detached-input-invariants.md)、
[专题设计](../../design/architecture/detached-input-invariants.md)和
[验证矩阵](../plans/p2-s08-detached-input-invariants.md)，并回写 Backlog、
审计执行清单和所属索引。本批设计未提交 Git。

## 契约复核

这是本轮自查记录，不代表独立 reviewer 或用户接受。

- 公共 Graphics opacity 输入已受检，缺口定位到样式原始值及内部重复入口；
  未将已有公共拒绝能力误记为缺失。
- Opacity 和字体都有可复用值，选择调整领域归属与样式保存类型，未预定新标量 wrapper。
- Border builder 与直接构造的默认 inset 不同，草案统一为自动四侧宽度，并明确
  显式 inset 的更新语义。
- 有限输入仍可能在横纵边距合计、描边外扩和文字加 padding 时溢出；
  草案分别规定配置、组合与 owner 准备检查。
- 内置 Detached Figure 的原始 bounds／stroke 字段和圆角负值截断也属于相同根因，
  已纳入 S08-C；其同类入口清单仍为实施前必做项。
- 外部实现仍可能产生非法回调输出，草案保留统一准入，不宣称构造能保证任意 trait
  实现的私有状态；检查与发布使用同一候选值。

主要评审取舍为 Opacity 的唯一路径迁移、Border 的构造返回值、Compound 的静态／
owner 指标准备边界，以及 Detached Figure 原始数值字段和 Bounded 更新面收口。
成本假设已记录，尚无本批产物或性能测量。

## 文档检查

`git diff --check` 通过。新文档本地链接和所用锚点单独核对。
`cargo xtask docs` 未通过，失败为既有长文本报告第 243 行引用缺失：
`target/verification/long-text/scroll-resize-shared-20261028/hot-comparison.json`。
本次未补造采样证据；全仓文档门禁保留未通过。

交付前按好好说话技能复核中文，保留类型、API、单位、失败边界和未验收状态。
机械扫描检查编号标题、翻案句、元话语、中文半角标点和破折号；
ADR、专题、矩阵与本记录均为零命中，无豁免。只使用语句和链接检查，
不以中文检查代替设计评审或行为验证。

## 后续

依 [CLAUDE.md](../../../CLAUDE.md) 的“重大架构变更需用户确认后再实现”，
本批先交付具体草案供评审。获准后按 S08-A～D 单链实施，验证矩阵仍全为待执行，
完成证据闭合后再关闭 R03／TC-03／API-F04／API-10。
