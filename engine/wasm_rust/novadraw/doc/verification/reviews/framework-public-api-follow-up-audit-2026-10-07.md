# 框架公共 API 命名、语义与职责复审

类型：`verification`

日期：2026-10-07

状态：发现项待整改

## 1. 审计信息

- 代码基线：`163f13a33de7` 加 2026-10-07 审计时工作区；
- 审计范围：`novadraw`、`novadraw-editor`、`novadraw-inspector`、
  `novadraw-backend-vello`、`novadraw-platform-winit` 与
  `novadraw-platform-web` 的公开 API；
- 重点：命名一致性、失败语义、状态所有权、职责边界、扩展闭合性及入门调用面；
- 审计方式：静态 API 与文档一致性复核，未执行运行期、性能或平台验证；
- 工作区说明：审计包含尚未提交的 Animation Behavior / Trigger 扩展，因此相关结论
  是进入稳定公开面前的阻断项，不表示该 API 已发布。

本报告是
[Core 公开 API 语义与命名审计](core-public-api-audit-2026-09-22.md)和
[公共 API 统一迁移完成记录](public-api-unification-completion-2026-10-06.md)
之后的横向复审。它记录当前快照，不替代 `design/`、ADR、parity ledger 或 roadmap。

## 2. 总体结论

此前对基础值、Runtime 可变借用命名、帧准备、第三方类型和 crate 分层的整改方向仍然
成立，但当前公开面尚不能视为完全收口。主要风险集中在：

1. `Graphics` 与 `NdCanvas` 同时承担公共绘制入口；
2. Editor Viewer 暴露过宽的可变状态逃逸口；
3. `Figure` 基础 trait 内置具体领域 capability，形成扩展闭集；
4. Figure 构造、树接纳和查询没有统一合法性与失败语义；
5. Animation Trigger 重新引入字符串属性身份；
6. root/prelude、资源管理、Inspector 与 CommandStack 的错误和职责边界仍不一致。

Vello backend 与 Winit/Web adapter 未发现同等级的 backend-specific 类型反向穿透 Core
问题。具体 backend 在自身 crate 暴露平台类型属于允许边界。

## 3. 发现项

### API-F01：`Graphics` 与 `NdCanvas` 形成竞争性公共入口

优先级：P0

状态：已整改（2026-10-08，见
[TC-13 / TC-14 完成记录](tc13-tc14-api-surface-hot-path-completion-2026-10-08.md)）

证据：

- `novadraw/src/lib.rs` 与 `novadraw/src/prelude.rs` 仍导出 `NdCanvas`；
- `novadraw/src/graphics/context.rs` 已提供 `Graphics` 和受限的 `PaintContext`；
- `novadraw/src/render/context.rs` 的 `NdCanvas` 仍公开 `clear_commands`、
  `damage_mut`、`commands` 和多组 `to_submission_*`；
- 同一类型还保留旧式绘制命名，与 `Graphics` 的结构化绘制方法并存。

影响：

- 普通调用者无法判断规范绘制入口；
- Figure 或应用绘制代码可以越过 Runtime 修改 damage、命令和 submission；
- 旧入口与新入口持续产生命名、校验和错误语义漂移。

目标：

- `Graphics` / `PaintContext` 是公开绘图语言；
- owned command recording 由 `CommandRecorder` 承担；
- `NdCanvas` 降为内部 recording primitive，或至少从 root/prelude 和普通 Figure
  编写路径移除；
- 不保留长期 deprecated 同义绘制入口。

### API-F02：Viewer 可变逃逸破坏 Editor 状态所有权

优先级：P0

状态：部分整改（Runtime 逃逸已关闭，model_mut 待独立事务收口）

证据：

- `novadraw-editor/src/viewer/mod.rs` 公开
  `model_mut() -> Result<&mut A, ViewerError>`；
- 原实现公开 `runtime_mut() -> &mut Runtime`；
- Viewer 同时承担模型投影、选择、输入仲裁和视觉状态同步。

影响：

- 调用者可绕过 CommandStack 修改模型；
- 调用者可绕过 Viewer 的视觉归属与同步事务修改 Runtime；
- Editor 的命令历史、EditPart 投影和选择状态不再构成可维护的不变量。

目标：

- 外部模型变化通过显式同步事务或 adapter notification 进入；
- 渲染驱动只暴露窄化 `ViewerDrive` / `RenderDrive` 能力；
- 不从 Viewer 返回完整 `&mut Runtime`；
- 测试辅助入口不得成为稳定应用 API。

整改证据：

- `GraphicalViewer::runtime_mut` 已删除；渲染、资源、viewport resize、backend session、
  Runtime gesture/focus 通过具名 forwarding API 驱动；
- router、字体、组件更新与 Part 内部 visual 均通过 Viewer 所有权边界进入；
- Native node editor 与 Web direct-edit host 已迁移，不再取得完整 mutable Runtime。
- `model_mut` 仍供现有 CommandStack/adapter 流程使用，不在本批伪装为已关闭。

### API-F03：`Figure` trait 同时承担基础协议与能力注册表

优先级：P0

状态：待独立设计

证据：

- `novadraw/src/figure/mod.rs` 的 `Figure` trait 同时覆盖生命周期、绘制、测量、命中、
  child policy、事件与多种领域行为；
- trait 内置 connection、point-list、scalable-polygon、text-flow、label 和 clickable
  等 capability accessor；
- 同时存在 `PaintContext` 绘制入口与接受 `NdCanvas` 的兼容绘制入口。

影响：

- 新增领域能力需要修改基础 trait；
- 第三方 Figure 的扩展能力无法在不改 Core 的情况下自然加入；
- 所有 Figure 被迫感知与自身无关的领域协议。

目标：

- `Figure` 仅保留所有 Figure 必须遵守的最小生命周期协议；
- 绘制、测量、命中和交互按稳定 capability 分层；
- 领域能力使用可扩展的类型化注册/查询机制，不在基础 trait 中追加已知 accessor；
- capability 重构必须先定义身份、借用、失败和 downcast 边界，不直接机械拆 trait。

### API-F04：Figure 构造与树接纳不能保证合法状态

优先级：P0

状态：部分整改（树接纳已统一，detached 构造器收口待后续批次）

证据：

- Rectangle、Ellipse、RoundedRectangle 公开 `bounds`，并保留多个标量构造和
  无返回值的 `set_bounds`；
- `with_stroke` 等入口不能报告非法宽度；
- Polyline 的部分配置静默 clamp，且查询仍使用非惯用的 `get_points`；
- `FigureTree` 接纳节点时复制初始 bounds/insets，但未形成统一的 finite、
  non-negative 和领域约束验证；
- Triangle 已使用受检构造，导致同类 Figure 的失败模型不一致。

影响：

- 非法值可在 detached 阶段进入对象，再延迟到绘制或后端失败；
- 同类 Figure 对相同错误分别采用接受、截断或返回错误；
- public field 可绕过 setter 与后续不变量。

目标：

- 几何 Figure 使用领域值构造，如 `new(Rectangle)`；
- fallible 配置使用 `try_new` 或返回 `Result` 的规范入口；
- public field 改为受控查询与 mutation；
- Builder/Runtime admission 执行统一的最后防线校验；
- clamp 仅用于合同明确允许的归一化，不用于隐藏非法输入。

整改证据：

- `FigureTree` 在分配 ID 和发布 topology 前统一拒绝非有限或负尺寸 initial bounds；
- `FigureTreeBuilder::set_contents` 改为 `Result<FigureId, GraphMutationError>`，与
  `add_child`/`insert_child` 使用相同接纳失败模型；
- detached Figure 的公开字段和标量构造器尚未统一，本发现项因此保持部分整改。

### API-F05：Animation Trigger 使用字符串标识属性

优先级：P0，提交前阻断

状态：已整改（2026-10-08，见
[TC-04 完成记录](tc04-typed-property-identity-completion-2026-10-08.md)）

证据：

- `PropertyKey<V>` 关联 namespace/name/value type；
- `TypedPropertyChange<V>` 是唯一公开构造入口；
- `AnimationTrigger::{property_changed,state_changed}` 只接受 typed key；
- fact、coalescing 与 lifecycle 映射使用 `ErasedPropertyKey`。

影响：

- 属性名称无法获得编译期重构和命名空间保护；
- trigger 与 target value 类型无法静态关联；
- 不同 Figure capability 的同名属性可能碰撞；
- 文档合同与实现公开面直接矛盾。

目标：

- 使用命名空间化 `PropertyKey`、枚举或 typed token；
- key 必须表达属性归属，并能与允许的动画值类型建立约束；
- 字符串只可作为诊断显示，不作为行为匹配身份。

### API-F06：树查询混淆合法缺失与无效身份

优先级：P1

状态：待整改

证据：

- `FigureTree::parent_id` 返回 `Option<FigureId>`，`None` 同时可能表示 root 和 unknown；
- `is_visible`、`is_enabled` 等查询把 unknown Figure 折叠为 `false`；
- 相邻查询又使用 `Option` 或 `TreeQueryError`，没有统一规则。

影响：

- namespace 错误、悬空身份和合法 `false` 无法区分；
- 调用者可能把状态损坏当成正常业务分支。

目标：

- `Option` 只表达合法缺失；
- identity/namespace/attached 失败返回 `TreeQueryError`；
- 布尔查询采用 `Result<bool, TreeQueryError>`；
- `parent_id` 采用 `Result<Option<FigureId>, TreeQueryError>`。

### API-F07：crate root 与 prelude 的精选边界仍不稳定

优先级：P1

状态：待整改

证据：

- root/prelude 已只保留 Runtime、树、基础 geometry、常用 Figure/Layout 与 Graphics；
- `NdCanvas`、backend、路由、资源与扩展协议已迁移到命名模块；
- rustdoc 分层符号快照已覆盖 root、prelude、领域模块与 `advanced`。

影响：

- 模块归属被 root 重导出掩盖；
- 专业协议容易被误认为长期稳定的入门 API；
- 后续内部整理会扩大 breaking-change 面。

目标：

- root/prelude 只保留 Runtime、FigureId、基础 geometry、常用 Figure/Layout 和规范
  Graphics 入口；
- 专业路由、资源、submission 与扩展协议从领域模块访问；
- 增加 root/prelude public API allowlist 或 rustdoc/public-api 快照门禁。

### API-F08：状态查询和操作错误被 `bool` 折叠

优先级：P1

状态：待整改

证据：

- `CommandStack::{can_undo, can_redo}` 接受 `&mut self`，会捕获 panic、改变 fault
  状态并返回 `false`；
- Animation `set_mode() -> bool` 将“值未变化”和“服务故障”折叠；
- `remove_behavior() -> Result<bool, _>` 的实际合同没有形成稳定的
  `Ok(false) == 幂等未变化` 语义。

影响：

- 名为 `can_*` 的查询具有状态变化副作用；
- 调用者无法区分正常不可用、幂等未变化和内部 fault；
- 相同 `bool`/`Result<bool>` 形状在不同服务中表达不同含义。

目标：

- 纯查询使用 `&self` 且无副作用；
- 可能触发 fault 处理的能力检查使用显式 `check_* -> Result<_, _>`；
- `Ok(false)` 仅表示成功执行但状态未变化；
- fault、panic 隔离和非法身份必须保留结构化错误。

### API-F09：Core Runtime 承担平台文件读取与过宽资源便利入口

优先级：P1

状态：待整改

证据：

- `novadraw/src/runtime/runtime/frame_resource.rs` 公开 `complete_image_file`；
- image bytes 解码、资源状态更新和错误映射在同一 Runtime 方法中耦合；
- image/font 以及 anchor/router 注册同时存在 panic 与 `try_*` 变体。

影响：

- 平台无关 Core 引入文件系统职责；
- 读取、解码和资源提交失败被压缩，难以诊断与恢复；
- panic convenience 与结构化失败入口形成两套规范。

目标：

- Core 只接收 bytes、decoded resource 或 provider 的异步完成结果；
- 文件系统和 URL 加载归 platform/application resource provider；
- 公共注册统一返回结构化错误，删除无必要的 panic alias；
- 资源失败保持读取、解码、revision 与提交阶段信息。

### API-F10：Inspector 静默吞掉同步故障

优先级：P2

状态：待整改

证据：

- `novadraw-inspector/src/lib.rs` 的 `events()` 在 Mutex poisoned 时返回空集合；
- `clear_events()` 在相同情况下静默无操作；
- `attach` 返回 `ListenerId`，生命周期解除仍完全由调用者配对管理。

影响：

- “没有事件”和“Inspector 已故障”不可区分；
- 测试与诊断工具可能给出错误的健康结论；
- listener 解除容易遗漏。

目标：

- 查询和清理返回显式 Inspector error，或暴露可查询 fault 状态；
- 评估使用 RAII subscription guard 管理 attach/detach；
- Inspector 保持只读，不因错误处理引入 Runtime mutation 权限。

## 4. 与既有完成记录的关系

2026-10-06 的完成记录证明一组已批准迁移切片通过了当时门禁，不表示所有公共 API
问题已经关闭。本次复审中的关系如下：

| 本次发现 | 与既有记录的关系 |
|---|---|
| API-F01 | Graphics 目标已设计，但 `NdCanvas` 公共旧入口尚未退出 |
| API-F03 | 完成记录已明确 Figure capability 全面改造不在当轮范围 |
| API-F07 | 2026-10-08 已由 TC-13 收窄 root/prelude，并建立分层 rustdoc snapshot |
| API-F02、F04、F06、F08-F10 | 属于此前切片未覆盖或未完全统一的职责/失败语义 |
| API-F05 | 2026-10-08 已由 ADR-028 / TC-04 完成 typed property identity 整改 |

因此不得修改旧完成记录为“未完成”；应以本报告作为后续增量审计入口。

## 5. 分阶段整改顺序

### Phase A：冻结和阻断扩散

1. 建立 root/prelude 导出 allowlist 或 public API snapshot；
2. 在 Animation Behavior 提交前完成 typed `PropertyKey`；
3. 禁止新增 `NdCanvas`、`runtime_mut` 和字符串属性身份消费者。

退出条件：新增代码不能继续依赖待移除入口，API diff 可自动审查。

### Phase B：收口权威入口

1. 完成 `Graphics` / `PaintContext` / `CommandRecorder` 职责分离；
2. 移除 Viewer 的完整 Runtime 和模型可变逃逸；
3. 用窄化 drive/synchronization facade 迁移真实消费者。

退出条件：绘制、Editor mutation 和 Runtime drive 各只有一条规范调用路径。

### Phase C：统一合法性与失败语义

1. 统一 Figure 构造和 admission validation；
2. 统一树查询的 `Result<Option<T>, E>` 与 `Result<bool, E>` 规则；
3. 整理 CommandStack、Animation、资源和 Inspector 错误合同；
4. 将平台文件加载移出 Core。

退出条件：非法状态不能进入树，错误不会被 `false`、空集合或 panic alias 隐藏。

### Phase D：Figure capability 开放化

1. 先形成独立设计或 ADR；
2. 定义 capability 身份、借用、生命周期、错误与第三方注册协议；
3. 迁移内置 Figure 和外部测试 Figure；
4. 删除基础 `Figure` trait 中的具体领域 accessor。

退出条件：新增第三方 capability 不要求修改基础 `Figure` trait 或 Core 已知类型列表。

## 6. 验证要求

每个整改切片至少需要：

- 编译期公共面探针：确认旧入口不可访问、目标入口可组合；
- 失败语义测试：unknown identity、非法值、fault 与幂等未变化可区分；
- 外部 Figure / Layout / Router / Editor consumer 测试；
- `cargo xtask docs` 检查索引、链接与公共面探针；
- 按变更范围运行对应 suite，最终交付边界再运行完整门禁。

本次仅记录静态审计结论，没有运行上述验证，也不将发现项写成已完成状态。
