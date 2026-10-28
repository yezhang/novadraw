# Shapes 编辑器

类型：`implementation-guide`

```sh
cargo +1.96.0 run -p gef-editor-demo
cargo +1.96.0 test -p gef-editor-demo --lib -- --test-threads=1
```

从仓库根目录执行。demo 使用根 workspace 的锁文件，默认二进制为
`gef-editor-demo`；体积基线需显式指定 `--bin ui-shell-baseline`。
UI 依赖固定的未修改上游 Git 提交，Cargo 管理源码缓存，无需本地 vendor。
需要安装 Rust 1.96。验证沿用上游输入 harness，绘制经独立的真实 GPU renderer；
不会调用上游 CPU 截图来替代 GPU 证据。
依赖迁移见[迁移记录](../../../doc/verification/reviews/masonry-upstream-migration-2026-10-28.md)；
功能、失败路径和成本见[Shapes 验证记录](../../../doc/verification/reviews/shapes-editor-implementation-2026-10-28.md)。

macOS 的标准 bundle 启动路径：

```sh
python3 tools/scripts/run_gef_macos.py
```

脚本构建 release、在 `target/masonry` 生成 `.app`，通过 LaunchServices 打开
该 bundle 并等待关闭。用户置前台后已确认画面显示，真实窗口截图已保存；
已登记 `gef-shapes-macos` Run action。自动前台激活和原生完整交互仍分别待验收。

## 使用

- 点击 Rectangle / Ellipse 模板，然后在画布点击或拖出尺寸；也可直接把模板拖入画布。
  点击采用默认尺寸，拖动采用规范化矩形。Escape 取消；成功后回到选择工具。
- Select / move 支持选择、拖动和尺寸 handle。Shift 追加选择，Control / Command 切换。
  Marquee 选择完全包含的节点，沿用同样的追加和切换规则。
- 滚轮滚动画布，中键拖动平移。画布的灰色边框为 1 个逻辑像素。
  内容超出视口后仍可滚动访问，拖动靠近视口边缘会自动展露。
- Solid / Dashed 先点击源节点，再点击目标节点。拒绝自连接和同方向重复边，
  拒绝规则不因线型改变。选中连接后拖动端点 handle 重连。Escape 取消未完成操作。
  矩形/椭圆分别使用 Chopbox / Ellipse Anchor，路径随障碍物和端点几何更新。
- Delete 删除选择的节点或连接，节点关联的边一起删除；Undo 恢复身份、关系和节点顺序。
  画布获得焦点时支持 Delete / Backspace、Control 或 Command + Z、
  Shift + Z 和 Control 或 Command + Y。
- Outline 与画布共享选择。Geometry 输入 x、y、width、height，Apply geometry
  通过命令修改；宽高至少为 24，所有值须为有限数。
- 路径输入框指定 JSON 文件。Save 保存当前文件；Save as 使用输入的新路径。
  保存成功才标记保存点。Open 拒绝丢弃未保存修改，Discard & open 明确放弃修改。
  文件读取、解析或写入失败会保留当前文档和历史。

模型、序列化、文件服务和侧栏属于应用；创建、框选、请求派发、命令历史和连接投影
属于 Editor。Xilem rebuild 消费一次性应用队列，保留 Masonry Widget 中的会话。
鼠标预览不触发外层 UI 重建，稳定模型、选择、工具、文档或错误变化才发布 action。
main 只组合应用入口，初始窗口为 1100×800。

Shapes 的功能实现与自动验证已覆盖 SH-01～SH-10；
原生完整交互、跨显示器 DPI、窗口外 capture、完整 IME 和系统 AT 保留独立待验收范围，
当前状态见[实施队列](../../../doc/roadmap/p2-delta-backlog.md)。
`ui-shell-baseline` 是同配置 Native UI 体积测量基线，不是用户样例。
