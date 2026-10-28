# P2-F02 Figure Capability 实现记录

类型：`implementation-verification`

日期：2026-10-08

状态：`complete`

## 1. 完成范围

- Figure attach 时构建 owned typed descriptor registry；duplicate 在 topology 发布前拒绝；
- `CapabilityKey<C>` 支持标准和外部 descriptor 的 typed query；
- 同步与 deferred `FigureCapabilityUpdate` 共享 prepare、revision、invalidation、
  damage 和 notification 提交原语；
- Preparation、Input、Lifecycle、Accessibility、Container、Layer、Freeform、Scale、
  Border、Clickable、Connection 与 Decoration 已迁移；
- PointList、ScalablePolygon、TextFlow、Label、Viewport 与 Image 保持 concrete
  model/type、typed update 或闭合 facade，不再伪装为开放 behavior capability；
- `Figure` 上旧领域 accessor 和伪 behavior trait 已删除。

## 2. Animation Binding

`PresentationBinding<V>` 使用相同 attach-time registry：

```text
committed Figure value
-> typed AnimationChannel<V>
-> sampled V
-> immutable FigurePresentation
-> Runtime presentation snapshot
-> fixed recursive renderer
```

- 外部 Figure 可登记自定义 binding，不修改 `ChannelBinding`、Runtime match 或 renderer；
- opacity/transform 是所有 Figure 的结构通道；领域 content 使用 binding；
- Connection route/dash 已迁移为标准 binding；
- 同一 typed family 的多个 channel 可组合并共享一次 prepare；不同 family 在
  admission 阶段按 interruption policy 仲裁；
- Replace/Ignore、cancel、dispose、visibility、damage 与 fault 复用统一 Animation 语义；
- renderer 只识别 optional content presentation，不识别 Connection、route 或 dash。

## 3. 工作量基线

- attach 成本：每个 Figure 一次 registry 构建，按已登记 descriptor 数量线性；
- query/update：单次 `HashMap<TypeId, descriptor>` 查找，无全树扫描；
- 无 active animation：snapshot 保持 O(1) `None` 快路径；
- active presentation：只遍历 active track 推导的 subject，不扫描 FigureTree；
- frame 录制：每个 active content subject 构造一次 immutable presentation；
- render traversal：仍为单次递归遍历，深度保护和 self/children/border 顺序不变。

没有引入全局 registry、Singleton、unsafe trait-object cast、逐帧 capability 全树扫描或
source-state animation mutation。

## 4. 契约覆盖

`p2_figure_capability_contract` 覆盖：

1. 外部标准 Input 与自定义 capability；
2. 同步/deferred update、prepare rejection 与 revision；
3. duplicate、foreign、stale、absent；
4. 标准 capability discovery；
5. 外部 `PresentationBinding<V>` 的 typed sample 与 frame recording；
6. binding absence、同 Figure content 冲突、Replace 与 dispose retire。

`p2_animation_transition_contract` 继续覆盖 Connection route interpolation、crossfade、
dash flow、route + dash family 组合与 pulse。P2-F02 suite 同时包含
layer/freeform/event/viewport/connection/border/text 回归。

## 5. 验证结果

- `cargo check -p novadraw --lib`：通过；
- `cargo clippy -p novadraw --lib -- -D warnings`：通过；
- `cargo xtask verify core.p2-f02-figure-capability`：通过；
- `cargo xtask docs`：通过；
- `cargo xtask check --quick`：通过；
- `cargo xtask check --full`：fmt、Facade、第三方类型、Native/Web 与 workspace Clippy
  通过；`workspace.test` 被并行中的 Builder `Result` 迁移阻断，旧 `cfg(test)` 调用尚未
  解包 `set_contents/add_child_to`，与本 delta 的定向及 workspace compile 结果无冲突；
- `git diff --check`：通过。

## 6. 后续

P2-F02 关闭临时概念审计 TC-02 与 TC-05。后续 TC-13/TC-14 已完成 root/prelude
allowlist、compile-fail snapshot 与热路径日志清理；临时概念演进下一批次转入 TC-04
typed property identity。
