# Novadraw 贡献者指南

本书面向需要修改 Novadraw 自身的开发者，回答三个仓库级问题：

1. 一项行为由哪个 crate、对象和生命周期阶段负责？
2. 修改内部协议时应核验哪些规范和核心原理？
3. 如何用设计文档、定向测试和分层门禁证明修改可以合并？

如果只需要使用公开 API 构建应用，或者通过公开 trait、typed update、Capability、
Policy 等扩展点增加能力，请阅读
<a href="../../user/html/index.html">《Novadraw 应用开发与扩展指南》</a>。
在进入源码和贡献流程前，需要系统建立框架心智模型时，请先阅读
<a href="../../internals/html/index.html">《深入理解 Novadraw》</a>。

## 本书与规范文档的关系

本书是当前仓库的贡献操作入口，不是架构原理书，也不是架构唯一事实源。发生冲突时，
按以下优先级核验：

1. 已接受的架构决策记录；
2. `doc/design/` 中的规范性设计；
3. `doc/parity/` 中的 Draw2D/GEF 语义账本；
4. `verification/suites.toml` 中的验证清单；
5. 当前实现和测试；
6. 本书的教学解释。

贡献者不应根据本书中的摘要跳过规范核验。书中会说明应该去哪里确认事实，但不会复制
整份 ADR、路线图或审计报告。

## 何时进入贡献者路径

以下工作属于项目贡献：

- 修改 Core 的 Figure、Runtime、布局、事件、连接或渲染协议；
- 新增一种需要 Core 统一发现的横切 Capability；
- 修改 Editor 的 Tool、Request、Policy、Command 或 Viewer 投影协议；
- 修改公开 API、crate 边界、错误模型或生命周期；
- 修改平台输入、资源交接或渲染后端合同；
- 修复必须触及上述内部协议的缺陷。

以下工作通常仍属于应用开发：

- 组合已有 Figure、布局器、视口和连接；
- 实现已经公开的 Figure、LayoutManager、Anchor、Router 或编辑策略；
- 通过 Runtime facade 更新已挂载对象；
- 在应用 crate 中适配业务模型和平台生命周期。

## 推荐阅读路径

### 修复局部缺陷

先读贡献流程，再按问题所属模块阅读对应章节，最后阅读验证与评审。

### 新增框架能力

按顺序阅读架构边界、Core Runtime、目标子系统和验证章节。涉及公共边界时，先形成
可评审的契约，再实现代码。

### 修改 Editor

先掌握 Core 的身份、坐标、输入和 Runtime 提交边界，再进入 Editor 的模型投影和编辑
协议。Editor 不拥有第二份业务事实。

## 贡献完成标准

一项修改完成时，贡献者应能回答：

- 权威状态由谁拥有？
- 修改从哪个公开或内部入口进入？
- prepare、validate、commit 和 publication 的边界在哪里？
- 失败前后哪些状态必须保持不变？
- 是否改变公开 API、语义账本或验证清单？
- 哪个最小 suite 能证明行为，最终需要运行哪一级门禁？
