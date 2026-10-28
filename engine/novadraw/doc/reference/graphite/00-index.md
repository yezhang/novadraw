# Graphite 参考

类型：`reference-analysis`

本页记录 Graphite 作为外部图形工具的参考入口。它不定义 Novadraw 的 API
语义，也不是 Draw2D/GEF 对标来源。

## 仓库

- GitHub：<https://github.com/GraphiteEditor/Graphite/tree/master>
- 当前记录的 HEAD：`f970cb167e7467716976f089daac502dd9de6678`
- 项目自述：Graphite 是开源的 vector and raster graphics engine，目标是面向
  2D 内容创作，包含非破坏式编辑、图层合成和节点式生成设计。

## 与 Vello 的关系

在上述 HEAD 的 workspace `Cargo.toml` 中，Graphite 使用 Linebender 生态依赖：

- `vello_gpu = { version = "0.3", default-features = false, features = ["wgpu", "full_image_support"] }`
- `parley = { version = "0.9", default-features = false, features = ["std"] }`
- `skrifa = "0.42"`
- `kurbo = { version = "0.13", features = ["serde"] }`

同一文件还把 `vello_gpu`、`vello_common` 和 `vello_gpu_shaders` patch 到
`https://github.com/Keavon/vello.git` 的固定提交
`7306bb4f206d956a8989173237c406b1d7798b35`。

## 可参考范围

- Vello / WGPU 在完整图形编辑器中的集成方式。
- 向量与栅格混合编辑、节点图、非破坏式工作流等产品层组织方式。
- Linebender 生态中 `kurbo`、`parley`、`skrifa` 与 Vello GPU 路径的组合方式。

## 边界

- 不用 Graphite 推导 Novadraw 的 Draw2D/GEF 语义。
- 不把 Graphite 的产品架构直接升级为 Novadraw 的引擎契约。
- 如需采用其中某个机制，必须重新做源码证据摘录、成本评估和 Novadraw 契约映射。
