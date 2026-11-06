# 第三方依赖

能力归属：Core 文字适配器，以及其他 owner 的 UI／后端依赖管理。

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

## UI 与共享 GPU

Vello 后端和独立 Canvas 使用 Vello 0.11／WGPU 30。
egui／eframe 0.37、egui-winit／egui-wgpu 0.37 与 winit 0.30 使用
crates.io 官方发布版，不使用其他项目的定制分支或 Git patch。
`novadraw-ui` 直接维护迁入的工作台源码，不依赖任何 craft crate。
可选 Masonry/Xilem 宿主保留独立 Vello 0.8／WGPU 28，不与共享链路交换 GPU 对象。
迁移范围及验证见
[工作台记录](../doc/verification/reviews/ui-workbench-migration-2026-11-06.md)。
当前官方依赖适配见
[官方 crate 切换记录](../doc/verification/reviews/ui-official-crates-2026-11-06.md)。
可选 Masonry/Xilem 使用 linebender 官方仓库的固定提交；现有
`xilem_masonry`、`masonry_imaging` 拆分包尚未发布，不能仅替换版本号为 crates.io 包。
