# ADR-032：Viewer 编辑会话与宿主驱动边界

类型：`architecture-decision`

## 状态

已接受并实现，2026-10-09。

本决策拟关闭临时概念审计 TC-07，并关闭 TC-10 中 CommandStack 探测语义与
Viewer 宿主驱动相关的部分。TC-10 的 Animation、Inspector 和其余错误分类另行处理。

## 背景

`GraphicalViewer` 同时拥有应用模型 adapter、EditPart/Figure 投影、选择、输入状态和
Runtime。Runtime 的完整可变逃逸已经删除，但 Viewer 仍公开：

```rust,ignore
fn model_mut(&mut self) -> Result<&mut A, ViewerError>;
```

这允许普通调用者绕过 CommandStack、revision 通知和投影刷新。EditorDomain 自身也按
以下分离步骤执行命令：

```text
CommandStack mutates model
-> caller remembers to call Viewer::refresh
```

因此，命令成功、命令可恢复失败和模型状态未知并没有统一的关闭动作。成功后遗漏
`refresh` 会让 history、模型和视图处于不同前缀；可恢复错误若违反“不修改模型”合同，
当前也无法在命令边界立即发现。

Viewer 还直接公开 frame、surface、backend session、time、focus 和资源 forwarding
方法。单个方法本身是窄的，但宿主循环仍长期持有完整 `&mut GraphicalViewer`，驱动面
与编辑面没有类型边界。

CommandStack 的 `can_undo/can_redo(&mut self) -> bool` 同时具有三种语义：

- 查询是否存在 history entry；
- 调用扩展 Command 的 `can_undo/can_redo`；
- 捕获 panic 后把 CommandStack 改为 faulted，并返回 `false`。

这使 UI 查询能够静默改变编辑状态，也把“无 history”“扩展拒绝”和“扩展 fault”
折叠为同一个布尔值。

GEF Classic 将 CommandStack 放在 EditDomain，并由模型 listener 驱动 EditPart 刷新。
其 Java 模型对象仍可被任意持有者修改，正确性依赖应用遵守通知约定。Novadraw 保留
“Domain 拥有 CommandStack、模型通知驱动投影”的语义，但利用 Rust 借用边界关闭完整
可变引用逃逸，不复制 GEF 的约定式可变性。

## 决策

### 1. Viewer 不再公开模型可变引用

`GraphicalViewer::model()` 继续提供只读 adapter 访问。`model_mut()` 直接删除，不保留
deprecated 别名。

模型变化只允许从两个入口进入：

```text
Editor command
    -> crate-private ViewerEditSession
    -> CommandStack operation
    -> session conclusion
    -> notification validation and projection synchronization

External model update
    -> GraphicalViewer::synchronize_model(callback)
    -> callback receives a non-escaping short mutable borrow
    -> notification validation and projection synchronization
```

对于由共享存储、消息队列或 adapter 自身观察机制完成的外部变化，调用方使用
`GraphicalViewer::synchronize()`，只消费通知并刷新投影。

`synchronize_model` 的 callback 不返回借用，且在返回前必须完成模型 revision 与事件
发布。该入口不写入 CommandStack，因此只用于加载、远端同步、测试输入或明确不属于
本地 undo history 的变化。普通编辑意图必须通过 EditorDomain 和 Command。

`refresh` 改为内部名称；公开同步语义统一为：

```rust,ignore
fn synchronize(&mut self) -> Result<bool, ViewerError>;

fn synchronize_model(
    &mut self,
    update: impl FnOnce(&mut A),
) -> Result<bool, ViewerError>;
```

`Ok(false)` 只表示同步成功且没有新投影 revision。revision gap、stale event、root
变化、扩展 panic 或部分投影失败继续使 Viewer faulted。

### 2. ViewerEditSession 关闭命令模型事务

`ViewerEditSession<'_, A, F>` 是 crate-private 短生命周期对象，只能由
`EditorDomain` 发起。创建 session 前，Viewer 先同步已有外部通知，使命令总是建立在
已投影的稳定 revision 前缀上。

session 只向 CommandStack operation 暴露短期 `&mut A`，并且必须以以下一种结论关闭：

| 结论 | 适用情况 | 关闭动作 |
|---|---|---|
| `commit` | execute/undo/redo 成功 | 校验并 drain 通知，刷新投影 |
| `reject` | 明确拒绝或可恢复错误 | 同步并要求没有 revision 变化；否则 fault |
| `fault` | panic、unknown-state 或其他不可恢复错误 | 不投影未知模型，CommandStack 与 Viewer 都保持 faulted |

session 未显式关闭即 Drop 时，Viewer 进入 faulted。Drop 不 panic，也不尝试猜测模型
状态或自动提交。

EditorDomain 使用单一内部 helper 执行 execute、undo 和 redo。Tool release、direct
text accept 和显式 `execute_request` 不再各自拼接
`stack operation -> viewer refresh`。

可恢复命令错误必须保持模型、revision 和待发布事件不变。`reject` 若观察到任何合法
revision 前进或非法通知，返回结构化 Viewer 错误并 fault Viewer。这把 Command 的
“失败不改变模型”合同变成可执行门禁。

### 3. ViewerDrive 限定宿主持有的能力

`GraphicalViewer::drive()` 返回短借用 `ViewerDrive<'_, A, F>`。宿主循环通过该 facade
访问：

- `prepare_submission` / `complete_submission`；
- `reset_backend_session`；
- `resize_logical_viewport` / `request_full_redraw`；
- `advance_time` / `next_wake_deadline`；
- `cancel_gestures` / `release_focus`；
- packaged font 等宿主初始化所需的资源注册。

这些方法从 `GraphicalViewer` 的直接公共表面移除，避免 Viewer 与 drive 两个长期同义
入口。`ViewerDrive` 不暴露：

- Runtime、FigureTree 或模型可变引用；
- Part/visual/overlay registry mutation；
- CommandStack 或 EditorDomain；
- Router、EditPolicy、selection 和 viewport 编辑策略；
- 任意可保存到 drive 生命周期之外的内部引用。

输入事件继续由 `EditorDomain` 仲裁并调用 Viewer 的领域操作；`ViewerDrive` 不建立
第二套 Tool/selection 输入协议。Part visual、Router 和 root layer 配置仍使用对应的
Viewer 领域 API，不归入宿主驱动。

### 4. CommandStack 拆分纯查询与扩展检查

删除公开 `can_undo/can_redo(&mut self) -> bool`，改为：

```rust,ignore
fn has_undo(&self) -> bool;
fn has_redo(&self) -> bool;

fn check_undo(&mut self) -> Result<bool, CommandStackError>;
fn check_redo(&mut self) -> Result<bool, CommandStackError>;
```

`has_*` 只查询 history 与 fault 状态，不调用扩展代码，不改变状态。

`check_*` 调用顶部 Command 的 capability 检查。`Ok(false)` 只表示扩展明确拒绝；
扩展 panic 返回 `CommandStackError::Panicked` 并使 stack faulted。实际 `undo/redo`
仍在操作点重新检查，不能依赖较早的 UI 探测结果。

### 5. 本批次直接 breaking

项目仍处于 `0.1.0`，workspace 内已知调用方一次迁移。不得保留：

- `GraphicalViewer::model_mut`；
- 公开 `GraphicalViewer::refresh`；
- Viewer 与 ViewerDrive 上重复的宿主驱动方法；
- 会静默 fault 的 `can_undo/can_redo -> bool`；
- 测试专用的生产级完整模型或 Runtime 可变引用。

现有测试中的模型故障注入通过 `synchronize_model` 的受控 callback 或局部测试 adapter
完成。API surface 门禁负责证明 `model_mut` 和重复 drive 方法没有重新出现。

## 错误处理

- session 开始前同步失败：命令不执行；
- 命令成功但通知无效：history 已提交，Viewer faulted，Host 必须重建模型与 Editor；
- 可恢复命令错误却改变模型：Viewer faulted，并报告“拒绝路径发生模型变化”；
- 命令 unknown-state 或扩展 panic：不尝试投影未知状态，stack 与 Viewer 同时 faulted；
- 外部同步 callback panic：Viewer faulted，panic 继续传播；
- `check_undo/check_redo` panic：返回结构化 CommandStackError，不折叠为 `false`；
- backend、surface 和 frame 失败继续使用既有 Render/Runtime 错误，不包装为 ViewerError。

## 扩展性与稳定性

- 新 Command 只依赖 ModelAdapter，不依赖 Viewer 或 Runtime；
- 新宿主只依赖 ViewerDrive，不取得 Editor 内部所有权；
- 共享存储 adapter 可先在外部提交，再调用 `synchronize()`；
- Viewer 自有 adapter 可通过 `synchronize_model` 执行无 history 的显式同步；
- 未来多 Viewer 共享同一模型时，仍以每个 Viewer 独立 drain/snapshot 合同为准，不把
  CommandStack 移入 Viewer；
- 本决策不引入跨应用副作用的自动回滚，也不替代 TC-08 的结构 prepared publication。

## 验证

1. compile/API surface：Viewer 不公开 `model_mut`、`refresh` 或完整 Runtime mutation；
2. session：成功 command 自动同步，无调用方手工 refresh；
3. session：可恢复失败且模型未变时保留 history 与可用状态；
4. session：可恢复失败却发布 revision 时 Viewer faulted；
5. session：unknown-state 与 panic 同时 fault CommandStack 和 Viewer；
6. external：`synchronize_model` 发布的合法批次立即投影；
7. external：共享 adapter 通知可由 `synchronize()` 独立消费；
8. drive：Native/Web host 只经 ViewerDrive 完成 frame、surface、time 与恢复操作；
9. query：`has_*` 不调用扩展，`check_*` 显式返回拒绝或 panic；
10. G1、G2、G4、G5、P2-E02 定向 suite、API surface、docs 与 quick gate 通过；
11. 最终 full gate 只执行一次，并单独记录既有无关阻塞。

## 日期

2026-10-09
