# Shapes 编辑器

类型：`implementation-guide`

```sh
cargo run -p gef-editor-demo
cargo test -p gef-editor-demo --lib
```

从仓库根目录执行。demo 使用根 workspace 的锁文件，默认二进制为
`gef-editor-demo`；体积基线需显式指定 `--bin ui-shell-baseline`。
当前应用外壳使用 egui/eframe 0.35；Novadraw 通过共享的 WGPU Device/Queue
渲染到应用持有的 RGBA8 纹理，再由 egui 中央面板合成。Viewer、EditorDomain、
命令历史和模型不归 UI 框架所有。构建使用系统默认 Rust 工具链。
此前的 Xilem/Masonry 迁移作为独立 adapter 记录保留，见
[迁移记录](../../../doc/verification/reviews/masonry-upstream-migration-2026-10-28.md)；
功能、失败路径和成本见[Shapes 验证记录](../../../doc/verification/reviews/shapes-editor-implementation-2026-10-28.md)。

macOS 的标准 bundle 启动路径：

```sh
python3 tools/scripts/run_gef_macos.py
```

脚本构建 release、在 `target/egui` 生成 `.app`，通过 LaunchServices 打开
该 bundle 并等待关闭。egui release 窗口已完成真实显示核验；完整原生交互、
窗口外 capture、IME 和系统 AT 仍分别待验收。

## 使用

- 启动文档预置 6 个矩形/椭圆节点和 6 条有向连接，包含实线、虚线、目标箭头、
  Chopbox/Ellipse Anchor 与多障碍路由。该拓扑用于直接展示官方 Shapes 样例职责，
  并默认选中第一个节点以展示 Handle、Layers 和 Inspector 投影。初始选择不产生历史，
  也不复制 Java 序列化文件。节点按三列两行等尺寸网格排列，默认连接只连接同排或同列
  节点，不产生多余折点；保存后仍使用本样例的版本化 JSON 格式。
- 左侧竖向工具轨提供 Select、Marquee、Rectangle、Ellipse 和 Connection。
  Connection 的 Solid / Dashed 在线型上下文中切换，不占用两个主工具入口。
- 在 Assets 中点击 Rectangle / Ellipse 模板，然后在画布点击或拖出尺寸；
  也可直接把模板拖入画布。
  点击采用默认尺寸，拖动采用规范化矩形。Escape 取消；成功后回到选择工具。
- Select / move 支持选择、拖动和尺寸 handle。Shift 追加选择，Control / Command 切换。
  拖动反馈使用透明选择轮廓和 8 个随动 handle；已提交位置的交互 handle 在手势期间
  隐藏，释放或取消后按最终选择几何重建。Marquee 选择完全包含的节点，沿用同样的
  追加和切换规则。
- 滚轮滚动画布，中键拖动平移。画布的灰色边框为 1 个逻辑像素。
  内容超出视口后仍可滚动访问，拖动靠近视口边缘会自动展露。
- Solid / Dashed 先点击源节点，再点击目标节点。拒绝自连接和同方向重复边，
  拒绝规则不因线型改变。选中连接后拖动端点 handle 重连。Escape 取消未完成操作。
  矩形/椭圆分别使用 Chopbox / Ellipse Anchor，路径随障碍物和端点几何更新。
- Delete 删除选择的节点或连接，节点关联的边一起删除；Undo 恢复身份、关系和节点顺序。
  画布获得焦点时支持 Delete / Backspace、Control 或 Command + Z、
  Shift + Z 和 Control 或 Command + Y。
- 画布上方的选择上下文栏提供删除、两组对齐、前后层级和分组操作。
  对齐包含左／水平居中／右，以及顶／垂直居中／底。
  至少选中两个 Shape 时启用，以最后选中的主节点为参照，主节点保持不动；
  连接不参与对齐，也不能作为参照。对齐保留节点尺寸，整组操作只产生一个撤销记录，
  重复执行已完成的对齐不增加历史。画布锁定或拖动手势进行中时禁用。
- Layers 与画布共享选择，支持搜索、可见性和锁定。隐藏节点不绘制、不命中；
  锁定节点仍可从 Layers 选择，但不能移动、缩放、删除或修改 Inspector。
  名称、可见性和锁定均进入撤销历史并写入文档。
- Inspector 的 Transform 输入 x、y、width、height，拖动结束、Enter 或失焦时
  自动提交一次命令；宽高至少为 24，所有值须为有限数。Appearance 提供 fill、
  stroke 和 opacity 色板，修改同样可撤销。
- 状态栏提供 Grid、Snap、缩小、100%、放大和 Fit。网格与吸附属于 Session 工作区，
  不写入 JSON，也不改变 dirty/savepoint；创建、移动和缩放共用同一吸附策略。
- 图形层级以文档 `order` 为唯一顺序。Group 是保存稳定 identity 和成员的应用组织关系，
  点击组会复用 Editor 多选；Group / Ungroup 和前后层级调整均可撤销。
- 左右侧栏可折叠但位置固定。停靠、浮动和布局恢复仍按
  [docking 延期方案](../../../doc/design/editor/egui-docking-deferred-plan.md)后置。
- 路径输入框指定 JSON 文件。Save 保存当前文件；Save as 使用输入的新路径。
  保存成功才标记保存点。Open 拒绝丢弃未保存修改，Discard & open 明确放弃修改。
  文件读取、解析或写入失败会保留当前文档和历史。

模型、序列化、文件服务、图层、分组、外观和侧栏属于应用；创建、框选、请求派发、
命令历史和连接投影属于 Editor。egui 只生成应用命令和画布局部输入；`Session`
独立持有 Viewer、EditorDomain 与工作区网格策略，UI 重绘不会重建编辑会话。
main 只组合应用入口，初始窗口为 1180×820。

Shapes 的功能实现与自动验证已覆盖 SH-01～SH-10；
原生完整交互、跨显示器 DPI、窗口外 capture、完整 IME 和系统 AT 保留独立待验收范围，
当前状态见[实施队列](../../../doc/roadmap/p2-delta-backlog.md)。
`ui-shell-baseline` 是同配置 egui/WGPU 体积测量基线，不是用户样例。
