# egui Docking 延期方案

类型：`proposal`

日期：2026-10-29

状态：`deferred`。当前继续使用固定三栏；本方案不进入 P2 backlog，也不代表已经批准实施。

## 当前决定

`gef-editor-demo` 保持固定应用外壳：左侧 Palette / Outline、中央 Canvas、右侧
Inspector，文档工具栏和状态栏位于外围。当前不引入 docking 依赖，不改变 Core、
Editor、Viewer、CommandStack 或画布宿主边界。

现有样例的面板集合和默认工作流都较稳定，还没有多文档、可交换工作区或大量辅助面板。
此时引入 docking 会提前增加布局持久化、焦点仲裁、拖放、浮动窗口和恢复失败处理，
但不能提高引擎能力的验证覆盖。是否采用 docking 应由真实工作流触发，不能只以
“更像成熟工具”为理由。

本方案只负责未来产品外壳的候选设计，不定义 Editor 运行时契约。

## 重新评审的触发条件

满足下列任一条件后，可以重新评审；未满足时继续使用固定三栏：

- 出现多个需要频繁交换、并排或隐藏的辅助面板，固定左右栏已经妨碍主要工作流；
- 产品需要多文档标签、用户自定义工作区、保存布局或恢复布局；
- 用户明确需要把 Outline、Inspector、Assets、History 等面板浮动或重新停靠；
- 真实可用性验证证明固定三栏是效率瓶颈，而不是单纯的视觉偏好。

进入实施前仍需单独批准范围、默认布局、不可移动面板和持久化版本。

## 候选依赖与能力边界

当前 `gef-editor-demo` 使用 egui 0.35。与之直接兼容的候选版本是：

```toml
egui_dock = { version = "0.20", features = ["serde"] }
```

截至 2026-10-29，`egui_dock` 0.21.1 已使用 egui 0.36。真正实施时必须重新核对
egui、eframe 和 `egui_dock` 的版本矩阵，不能机械复制上述版本。

`egui_dock` 提供标签页、二叉分割树、拖动、尺寸调整、程序化布局和可序列化的
`DockState`。它的浮动 Surface 使用 `egui::Window`，不是独立操作系统窗口。
工作区版本迁移、面板注册、布局恢复、损坏数据回退、屏幕边界校正和产品级约束仍由
应用负责。

参考：

- [`egui_dock` 0.20 文档](https://docs.rs/egui_dock/0.20.0/egui_dock/)
- [`DockState` API](https://docs.rs/egui_dock/0.20.0/egui_dock/dock_state/struct.DockState.html)
- [`TabViewer` API](https://docs.rs/egui_dock/0.20.0/egui_dock/widgets/tab_viewer/trait.TabViewer.html)

## 状态所有权

Docking 只属于应用 shell：

- `DockState<PanelRef>` 保存面板身份和布局；`PanelRef` 使用稳定 key，不保存活的
  Viewer、Runtime、GPU 资源或业务模型；
- 面板 renderer 由应用 registry 提供，内置面板与后续扩展面板使用同一注册机制；
- Canvas session、Viewer、EditorDomain 和 CommandStack 继续由现有应用 session
  持有，标签移动、UI 重绘和布局恢复不得重建它们；
- 持久化只保存带版本的布局、活动标签和浮动几何；加载时丢弃未知面板，拒绝重复
  Canvas，校正屏幕外窗口，失败时恢复默认布局；
- 文档工具栏、工具轨、上下文栏和状态栏不进入 docking tree。

Docking 不进入 `novadraw`、`novadraw-editor` 或平台 adapter 的公共 API。若多个产品
以后出现相同需求，再以真实消费者证据决定是否提取独立应用层组件。

## 初始产品约束

首个候选布局仍以 Canvas 为唯一主工作区：

- Canvas 不可关闭、不可浮动，并且默认占据中央最大区域；
- Layers / Assets 可在左侧共享标签组；
- Inspector / Outline / History 可在右侧共享或拆分；
- 用户可以隐藏和恢复辅助面板，但必须能一键恢复默认工作区；
- Canvas 输入只在对应标签可见且获得焦点时启用，面板拖动不得泄漏 pointer capture、
  IME lease 或编辑手势。

实施前必须先确认候选版本能否直接表达 Canvas 的固定约束。若不能，应先评估小范围
上游扩展；不得靠每帧把布局强行改回去维持表面效果。

## 重新启动后的实施顺序

1. 建立独立接入探针，验证 Canvas 固定约束、焦点、拖放和浮动 Surface。
2. 定义稳定 `PanelRef` 与开放 panel registry，建立默认布局。
3. 接入现有 Palette、Outline、Inspector，不改变业务动作和编辑会话所有权。
4. 增加带版本的 serde 布局保存、损坏回退和默认布局重置。
5. 验证键盘导航、跨显示器 DPI、窗口缩放、失焦取消和可访问名称。
6. 比较启用前后的 Native 产物体积、初始化时间和典型拖动帧耗时。

只有上述探针和设计通过后，才把实施切片加入
[P2 backlog](../../roadmap/p2-delta-backlog.md)。

## 验收门禁

- 默认布局、用户布局和重置布局都有确定结果；
- 未知、重复、旧版本或损坏的布局不会破坏启动；
- 标签移动和布局恢复不重建 Viewer、Runtime、CommandStack 或文档历史；
- Dock 拖动与 Canvas 编辑手势互斥，失焦会清理 capture 和临时反馈；
- 浮动 Surface 在窗口缩放、DPI 变化和显示器移除后仍可恢复；
- 键盘焦点、快捷键、关闭确认和未保存状态有明确归属；
- 依赖和 serde feature 的包体积成本有同配置前后数据。

## 保留替代方案

- 固定三栏：触发条件未满足时的默认方案，也是当前决定；
- `egui_tiles`：只有出现多子节点或网格布局的刚性需求时再比较，不因灵活性描述直接替换；
- 原生多窗口：属于平台宿主能力，不用 `egui::Window` 浮动 Surface 冒充。
