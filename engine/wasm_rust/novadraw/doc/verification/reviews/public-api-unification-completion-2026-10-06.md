# 公共 API 统一迁移完成记录

类型：`verification`

日期：2026-10-06

状态：已完成

## 1. 范围

本记录对应
[公共 API 统一设计与迁移提案](../../design/architecture/public-api-experience-proposal.md)
中 P2-G02 之外获准实施的跨领域候选。迁移保持 Runtime、FigureTree、Editor Viewer
和 backend 的既有状态所有权，不借改名引入第二套 mutation 或 submission 协议。

本轮明确不包含 Figure capability 全面改造和 Editor direct-edit session facade；
两者仍需独立契约与真实消费者。

## 2. 已闭合切片

| 切片 | 完成结果 | 提交 |
|---|---|---|
| 帧准备 | `prepare_submission` 直接返回完整 `FramePreparation`，调用者显式处理 Idle、Suspended、AwaitingCompletion 与 Error | `cf0376e` |
| 角度单位 | Graphics degree 入口与 Affine2D radian 入口名称显式化，数值语义不变 | `5bc26b2` |
| 尺寸与 inset | Figure、测量、Border 与 Runtime 公共面统一使用 `Dimension` / `Insets` | `26a64c1` |
| 图像源区域 | `ImageRegion` 绑定 image 与受检 source-pixel rectangle，destination 保持逻辑坐标 | `c0a4ef6` |
| 图元构造 | Triangle 使用 `new(Rectangle)`；无效 stroke 在 detached 配置边界拒绝 | `a89a538` |
| Runtime 可变借用 | `FigureEditor`、`ContainerEditor` 及全部同类短生命周期 facade 改为 `FigureMut`、`ContainerMut` 等 `*Mut<'_>`；`Editor` 专指编辑器框架 | `52a3d35` |
| Editor / Inspector | Editor 开放筛选后的领域模块；Inspector 使用精确的 `capture_tree` | `1552fce` |
| Core 导出 | root/prelude 移除专业查询、路由、资源和 submission 组装协议，领域模块仍公开扩展入口 | `ae9d356` |
| Vello | Native `new` 返回 `Result<_, VelloInitializationError>`；`size` 改为 `pixel_size`，宿主显式处理初始化失败 | `6863bb8` |

`ViewportHandle`、`ScaleHandle` 等 identity/read-only 类型继续保留 `Handle`；
短生命周期 Runtime 可变借用统一使用 `*Mut`。文档使用
“scoped mutable facade”，不再把 Core facade 称作 editor。ADR-019 文件路径为稳定历史
链接，标题与正文已同步新术语。

## 3. 兼容与失败语义

- 项目仍为 `0.1.0`，旧同义入口直接删除，不保留 deprecated 转发壳。
- 名称迁移不改变坐标域、角度数值、mutation 原子性、damage、通知或帧确认顺序。
- 非法几何、stroke 和 image source 在构造或录制边界结构化拒绝，不延迟到后端静默失败。
- Vello surface 初始化错误交还宿主；示例宿主退出，验证/基准宿主在应用边界显式 panic。
- Core root 缩减不将专业协议私有化；外部 Figure、Layout、Router 和 backend 仍从领域模块组合。

## 4. 自动验证

代码切片期间已通过：

- `cargo test -p novadraw`：355 个 unit test、全部 integration test 与 doctest；
- `cargo check --workspace --all-targets`；
- `cargo test -p novadraw-backend-vello --features native`：22 个 unit test 与 doctest；
- Core facade、ImageRegion、Triangle、Editor 领域模块与 Inspector capture 的定向契约测试。
- `cargo xtask docs`：文档 metadata、链接、42 个 suite 与 17 个 compile-fail
  公共面探针通过；
- `cargo xtask check --quick`：通过；
- `cargo xtask check --full`：通过，包括 workspace Clippy、全量 unit/integration/doctest、
  Native/Web Vello 和公共依赖边界。

## 5. 剩余边界

- Figure capability accessor 的闭集问题仍按“后续定向”处理，不以本次 root 收口冒充完成。
- Editor direct-edit session facade 不在本轮创建，避免复制 Viewer/Tool 的编辑状态。
- Animation 设计中的 Runtime 可变借用预留为 `AnimationMut<'_>`，不得重新引入
  `AnimationEditor` 与 `novadraw-editor` 概念冲突。
