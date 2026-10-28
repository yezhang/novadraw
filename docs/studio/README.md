# Studio 产品设计入口

本目录临时承载未来 `<prefix> Studio` 中台产品的产品设计、AI-native 工作流和产品级
协议。`<prefix>` 只是品牌占位符，不表示产品名称已经确定。

Studio 与 Novadraw Framework 属于不同产品层：

- Studio 面向最终用户，拥有自己的产品模型、文档格式、交互、AI adapter、权限和发布；
- Novadraw Framework 提供 Figure、Runtime、Editor、平台适配和渲染等公开能力；
- Studio 只能通过框架公开 API 使用 Novadraw，不能依赖私有模块或获得产品专用后门；
- Studio 产品设计不能写入 `engine/novadraw/doc/`，也不能定义引擎完成状态；
- 只有被证明具有通用消费者价值的需求，才通过独立设计或 ADR 进入框架。

当前尚未创建独立产品仓库，因此本目录只保存产品设计文档，不放置正式产品代码。创建
Studio 仓库后，本目录内容应迁移到产品仓库，并由产品仓库独立版本化和维护。

## 文档

| 文档 | 状态 | 主题 |
|---|---|---|
| [AI-native 产品设计](ai-native-product-design.md) | proposal | Studio 的 DSL、定性空间推理、Agent 协作、审查闭环与验证路线 |

## 维护规则

- 明确区分产品事实、设计决策、待验证假设和未来设想；
- AI 输出始终是候选，不得作为领域事实、约束正确性或执行结果的最终裁判；
- 产品模型、文档历史和工作区临时状态必须分别建模；
- 产品级 DSL、持久化、迁移、权限、账号、同步和 AI provider 不下沉到引擎；
- 对框架的新诉求先写成产品侧缺口，再单独证明其通用性和公开契约。
