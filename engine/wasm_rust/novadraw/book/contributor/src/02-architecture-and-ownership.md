# 2. 静态架构、身份与所有权

> **本章解决的问题**：修改某个模块时，如何避免制造第二份状态、反向依赖或跨层类型泄漏。

## 2.1 Crate 边界

Novadraw 按责任而不是部署形式拆分：

```text
应用
├── novadraw-editor
├── novadraw-inspector
├── novadraw-platform-winit / web
├── novadraw-backend-vello
└── novadraw
```

`novadraw` 是平台无关 Core。Editor、Inspector、后端和平台适配器可以依赖 Core，
Core 不反向依赖它们。应用在 composition root 组装所需 crate。

修改公开边界前先核验：

- [`ADR-021：公开 Facade 与 Feature 边界`](../../../doc/adr/adr-021-public-facade-and-feature-boundary.md)
- [`ADR-022：第三方类型与渲染依赖边界`](../../../doc/adr/adr-022-third-party-type-and-render-dependency-boundary.md)
- [`ADR-023：Crate 收口与扩展边界`](../../../doc/adr/adr-023-crate-consolidation-and-extension-boundaries.md)

## 2.2 权威状态只能有一个所有者

典型状态所有权：

| 状态 | 所有者 |
|---|---|
| 业务节点、连接和属性 | 应用模型 |
| Figure 拓扑和通用节点状态 | `FigureTree` |
| 失效、重绘、交互与提交会话 | `Runtime` |
| Model 到 Figure 的投影关系 | `GraphicalViewer` |
| 选择和活动编辑会话 | Viewer / `EditorDomain` |
| GPU 专有资源 | 渲染后端 |
| 窗口、系统事件和文本输入客户端 | 平台宿主 |

新增字段前先判断它是否复制了其他 owner 的事实。缓存和派生状态必须能够从权威状态
重建，并有明确失效条件。

## 2.3 三类身份不能混用

图形编辑链存在三个主要身份域：

```text
ModelId -> EditPartId -> FigureId
```

- `ModelId` 属于应用模型，可以持久化；
- `EditPartId` 属于单个 Viewer；
- `FigureId` 属于单个 Runtime。

命令和持久化数据不能保存 `EditPartId` 或 `FigureId`。运行时身份必须检查 namespace
与 generation，不能把 stale 或 foreign identity 当作合法缺失。

## 2.4 生命周期决定可用入口

Figure 经历四个阶段：

| 阶段 | 合法入口 |
|---|---|
| Detached | 构造器与 `with_*` |
| Build | `FigureTreeBuilder` |
| Attached | Runtime scoped facade |
| Drive | Runtime、Host 与 Backend |

挂载后不再向调用者暴露长期 `&mut Figure`。一次修改应由 Runtime 统一完成校验、
revision、invalidation、damage 和 notification。

## 2.5 扩展性判断

只有存在真实替换需求和独立消费者时才引入 trait。判断顺序：

1. 它是否只是具体 Figure 的数据？
2. 是否可以使用结构体、泛型函数或 typed component update？
3. 是否存在不知道具体类型的独立消费者？
4. 多个实现是否共享相同语义、生命周期和失败合同？

只有第 3、4 项同时成立时，才适合形成横切 Capability 或行为 trait。

## 2.6 评审问题

每项结构变化至少回答：

- 新状态由谁拥有，谁只持有 ID 或快照？
- 新依赖是否仍然指向内层？
- 失败发生在 prepare、validate、commit 还是 publication？
- 是否可能发布半完成结构？
- 扩展者能否在不修改中心枚举或 match 的情况下增加实现？
- public API 是否暴露了第三方后端类型或完整可变对象？
