# 第三方依赖

能力归属：Core 文字适配器的依赖管理。

默认文字引擎使用 crates.io 发布的 **Parley 0.12.0**。
Parley Engine、Fontique 和 Harfrust 由上游依赖图引入，实际版本及校验和见
根目录 `Cargo.lock`。本目录不再维护文字依赖源码或补丁。

workspace 的 Parley 依赖必须启用上游 `complex-scripts` feature，
使中文／日文词边界及泰文等复杂文字断行使用 ICU4X 词典数据。
Parley 0.12.0 默认未启用此能力；仅注册字体不能补齐分词模型。
回归入口：`cargo test -p novadraw --lib render::text::tests::cjk_word_navigation_uses_dictionary_boundaries -- --exact`。

2026-11-05 退役了 Parley 0.7.0、Fontique 0.7.0 和 Harfrust 0.3.2 的
三份源码副本、35 份补丁及其专用验证工具。字体选择、塑形、断行、对齐和
文字交互由上游接口承担；Core 通用阶段协议与外部引擎预算合同保留。
默认 Parley 适配器未实现第三方内部可拒绝预算或精确留存容量登记。

替代验证入口为 `cargo test -p novadraw --test p2_parley_public_api_contract`，
Core 阶段合同入口仍为 `cargo test -p novadraw --test p2_text_stages`。
实施与验证见[迁移记录](../doc/verification/reviews/parley-upstream-migration-2026-11-05.md)。
旧补丁顺序和验证范围见[已提交基线的说明快照](../doc/archive/dependency-patches-2026-11-05.md.txt)；
快照中的旧路径和命令只用于历史追溯。
