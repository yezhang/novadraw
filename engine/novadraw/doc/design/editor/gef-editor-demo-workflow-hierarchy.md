# GEF Shapes 样例工作流层级

类型：`normative-design`

日期：2026-10-29

状态：固定三栏下的 P0、P1、P2 已实现并通过自动测试与原生窗口复核；
docking 继续延期。

## 目标

`gef-editor-demo` 使用成熟图形设计工具的操作层级组织 UI，同时保持它是 Core /
Editor 的消费样例，不把产品面板、文档外观或工作区偏好反向放进引擎契约。

界面固定为：

1. 文档栏：文件、保存状态、撤销和重做；
2. 左侧工具轨：选择、框选、矩形、椭圆和连接；
3. 画布上方上下文栏：删除、对齐、层级、分组和当前工具选项；
4. 左侧栏：Layers / Assets；
5. 中央画布：唯一主工作区；
6. 右侧 Inspector：对象摘要、Transform 和 Appearance；
7. 状态栏：工具提示、选择摘要、画布锁定、网格、吸附和缩放。

使用频率越高、与当前选择越相关的操作，越靠近画布。文档级操作位于外围。

## P0：壳层与工具

- 文档栏不再承载编辑工具；保留文件路径、文件操作和撤销/重做。
- Select、Marquee、Rectangle、Ellipse 和 Connection 统一进入竖向工具轨。
- Solid / Dashed 是 Connection 的上下文选项，不再表现为两种并列主工具。
- Delete、Align、层级和分组属于选择上下文；无有效选择时禁用。
- 状态栏显示当前工具提示、选择摘要、缩放和视图开关，不重复显示 `Tool: ...`。

## P1：Layers、属性提交与网格

- 左侧栏使用 Layers / Assets 两个固定标签。Assets 保留模板点击和拖放；
  Layers 提供搜索、选择、可见性和锁定。
- 可见性、锁定和对象名称属于 Shapes 文档模型，通过应用 Command 进入撤销历史并保存。
- Transform 不保留 Apply / Reset 表单。字段编辑期间保留草稿；拖动结束、Enter 或失焦后
  自动提交一个几何 Command。非法值保留草稿并显示错误，不进入历史。
- 网格显示和吸附属于应用 `Session` 的工作区策略，不写入业务 JSON，不进入撤销历史。
  创建、移动和缩放的反馈与提交使用同一吸附修正。
- 状态栏提供 Grid、Snap、Zoom out、100%、Zoom in 和 Fit。

## P2：层级、分组、外观与折叠

- 图形顺序继续以文档 `order` 为唯一 Z-order；Bring forward / Send backward 修改该顺序，
  Viewer 按模型通知更新 Figure 顺序。
- Layer Group 是文档内的组织关系，保存稳定 group identity 和成员。点击组选择全部成员，
  因而复用现有多选移动、对齐和删除能力；它不伪装成新的 Core 容器契约。
- Group / Ungroup、层级调整和 Appearance 修改都通过可撤销 Command。
- Appearance 首批提供 fill、stroke 和 opacity；Shape 的 Figure style 由模型投影更新。
- 左右侧栏可以折叠和恢复，但位置固定。停靠、浮动和工作区布局恢复继续遵循
  [egui Docking 延期方案](egui-docking-deferred-plan.md)。

## 图标体系

- 通用 UI 图标统一使用 Phosphor Regular，通过 `egui-phosphor 0.13` 接入。
  该版本与项目现有的 egui 0.35 对齐，没有引入第二套 egui。
- 文件、撤销重做、工具轨、图层控制、层级、分组、缩放、网格和吸附使用
  Phosphor 语义图标。图标按钮保留 tooltip 和可访问名称。
- 六个对齐按钮继续使用样例内的几何绘制。它们表达的是编辑器领域语义，
  通用图标集合没有同等准确的替代项。
- release 二进制从 17,276,896 字节增加到 17,805,440 字节，增量为
  528,544 字节，约 3.1%。其中 Phosphor Regular 字体原始大小为 488,636 字节。

## 状态所有权

| 状态 | owner | 说明 |
|---|---|---|
| 几何、名称、可见性、锁定、Z-order、Group、Appearance | `Diagram` | 保存、撤销、模型通知 |
| 选择、Tool、手势、CommandStack、Viewport | `Session` / Editor | UI 不复制编辑状态 |
| Grid、Snap 工作区策略 | `Session` | 不属于业务文档 |
| Layers / Assets 标签、筛选、侧栏折叠 | `EditorUi` | 纯应用 shell 状态 |
| Inspector 草稿 | `AppState` | 只在合法提交后进入模型 |

内置面板和后续扩展面板不获得 Core 特权入口；本切片不新增 docking manager。

## 验收

- 工具模式只在工具轨出现，Connection 线型只在上下文位置出现；
- 隐藏对象不绘制、不命中，锁定对象可从 Layers 选择但不能移动、缩放或删除；
- 名称、外观、可见性、锁定、顺序和 Group 可以保存、重开、撤销和重做；
- Transform 一次完成交互只产生一个历史记录；
- Grid 与 Snap 不改变文档 dirty/savepoint，吸附反馈与最终几何一致；
- 缩放、Fit、滚动和侧栏折叠不重建 Viewer、Runtime 或 CommandStack；
- 默认、最窄和较宽窗口中，工具轨、上下文栏、画布和侧栏不重叠。

## 实施结果

2026-10-29 已完成：

- 文档栏、竖向工具轨、选择上下文栏和增强状态栏；
- Layers / Assets、名称、可见性、锁定和 Transform 自动提交；
- Session 级 Grid / Snap、缩放步进、100% 和 Fit；
- Z-order、Group / Ungroup、Appearance 与固定侧栏折叠；
- 文档版本 2，并保留版本 1 的读取兼容；
- 29 项 demo 单元与契约测试；
- macOS release bundle 默认窗口和 900×620 最小窗口截图复核。

Clippy 对本次 Editor API 与 demo 代码通过。全依赖严格 `-D warnings` 仍会先命中
`novadraw` 既有的 `chunks_exact_to_as_chunks` 和 `drain_collect` 告警；
本次未扩大范围修改这些文件。
