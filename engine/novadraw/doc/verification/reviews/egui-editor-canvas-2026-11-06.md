# egui Editor Canvas 实施与验证

类型：`verification`
能力归属：其他——`novadraw-ui` Canvas／输入适配及工作台消费者。
日期：2026-11-06

组件已实现，定向输入、UI 回归与编译检查通过。Native 能绘出图形，但六图形连接
场景出现 Core 坐标投影预算错误，完整替代验收保持未通过。
本轮不修改 Core 投影实现，也不增加预算掩盖该错误。

## 实施边界

契约见 [egui Editor Canvas](../../design/egui-editor-canvas.md)。
`canvas` feature 提供 `CanvasSurface`，持有共享 device／queue 的 Vello renderer、
有界纹理和 egui 注册；稳定尺寸复用纹理，Drop 注销注册。
`editor-canvas` 提供 `EditorCanvas<A, F>`，持有一份 Viewer／Domain；
`EditorCanvasInput` 供已有会话封装和无 GPU 消费者使用，两者共用输入适配。

Canvas 接收焦点范围内的键盘，按 egui 控件身份接收 pointer，捕获对应按钮的区域外
释放。中键平移、滚轮、缩放、禁用／失焦／隐藏取消均接到现有 Editor 协议。
Composer 增加有序步骤接口，仍使用同一 composition 状态机；文本输入绑定当前批次
的 lease，完成后的剩余事件不会继续写入新会话。
caret／候选位置消费 Editor 的效果，不每帧提取全文草稿。
剪贴板及官方字符删除事件按需读取快照。

显式失焦遵循会话策略；输入或绘制错误强制取消草稿，避免借错误清理触发提交。
Viewer 驱动 prepare／submit／complete；Idle 不调用 backend submit，
Runtime deadline 和 auto-expose 安排唤醒。零尺寸暂停；DPI／纹理限制先校验。
GPU 重建使用新的 RenderState 并重置 backend session，最终合成仍由 eframe 负责。

工作台已由静态纹理展示改为可复用 Editor Canvas，复用 Shapes 模型／Factory，
演示选择、移动、创建、图层选择和同一份 Undo／Redo。
Masonry 适配仍保留给现有消费者；本轮没有删除其 package 或官方 Git 依赖。
普通工作台默认 feature 不依赖 Core、Editor 或 Vello。

## 自动验证

环境：macOS；系统默认 Rust／Cargo；官方 egui／eframe 0.37、WGPU 30、Vello 0.11。

| 检查 | 结果与范围 |
|---|---|
| `cargo test -p novadraw-workbench-app --bin novadraw-workbench-app` | 9 项通过：自定义模型／Factory／文本 Policy 消费者；选择、区域外拖动、统一历史、错误取消、中键、滚轮、隐藏、禁用、失焦、IME 与 lease |
| `cargo test -p novadraw-ui --features serde,native-windows,testing,editor-canvas` | 504 项单测、4 项外部面板测试、5 项 doctest 通过；2 项原有 doctest ignored |
| 定向 Clippy | UI `--lib`（四个 feature）和工作台 `--bin novadraw-workbench-app` 均通过 `-D warnings` |
| Web `cargo check -p novadraw-ui --lib --features serde,editor-canvas --target wasm32-unknown-unknown` | 通过；Vello 后端原有 4 条 dead-code warning 保留，不等于浏览器运行通过 |
| `cargo check --workspace` | 通过；保留第三方 `block` future-incompatibility 提示 |
| UI 默认 feature `cargo check -p novadraw-ui --lib --no-default-features` | 通过，普通工作台仍可独立消费 |
| 包级 fmt | UI 与工作台通过 |
| workspace fmt | 未通过：并行 Editor `src/viewer/failure.rs:781` 格式差异；未改写该文件 |
| `cargo xtask docs` | 被并行新增的 `doc/design/architecture/backend-local-damage-selection.md` 缺少类型标记阻塞；本轮文档单独校验本地链接 |

输入测试在独立应用包中通过公开 API 消费 UI，不增加 Core／Editor 特权路径。
文本 Policy 检查 preedit 不修改模型、commit echo 只插入一次、IME 确认 Enter
不同时提交会话、接受后只产生一条命令、失焦策略和 lease 结束后的剩余事件。
这些测试证明标准化事件行为，不证明真实操作系统输入法。

## Native 证据与未完成项

运行：

```bash
cargo run -p novadraw-workbench-app -- --smoke-screenshot target/verification/workbench-editor-canvas.png
```

程序正常退出，生成 2048 × 1280 截图。视检可见停靠面板、六个图形和连接，
但顶部同时显示：

```text
Canvas preparation: Error(Validation(Layout(Coordinate(Projection(Budget(
PreparationBudgetError {
  stage: "coordinate projection consumer",
  used: 2000144,
  limit: 2000000,
  allocation: true
}
))))))
```

截图是本地产物，不纳入 Git。退出码为 0 只证明截图流程完成；
该图不能用于宣称 Native 完整验收通过。恢复条件是 Core 投影路径完成预算整改，
在默认预算下重新运行六图形连接场景并完成真实交互验证。

尚未验证设备丢失后的实际恢复、跨原生窗口 Canvas capture、真实系统输入法、
浏览器交互和 Windows／Linux 原生行为。当前没有 release 成本采样或提速结论；
纹理尺寸复用和 Idle 不提交只是实现属性，不代表完整性能验收。
未进行 workspace 全量 Clippy／test，也未关闭 P2 的 Core 或平台门禁。
