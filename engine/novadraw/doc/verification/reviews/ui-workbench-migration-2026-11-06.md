# WGPU 30 与本地工作台迁移验证

类型：`verification`
能力归属：其他——Vello 后端、原生宿主、独立 Canvas 与 `novadraw-ui`。
日期：2026-11-06。

本记录保留首轮定制分支基线。用户随后要求使用官方 crate；
当前依赖、适配与验收以[官方 crate 切换记录](ui-official-crates-2026-11-06.md)为准。

## 交付范围

共享 GPU 链路从 Vello 0.10／WGPU 29 升为 Vello 0.11.0／WGPU 30.0.1。
现有 Shapes、Logic 和官方示例的 egui 宿主改用固定的 egui 0.37 分支，
对应原生 IME 使用配套 winit 分支。契约与提交号见
[工作台设计](../../design/ui-workbench.md)。

新增 [novadraw-ui](../../../crates/novadraw-ui/README.md)，直接维护 craft-libs
固定提交的工作台源码及测试，不依赖 craft-ui 或其他 craft crate。
停靠树、标签页、分割布局、浮窗、工具栏、面板、按钮、下拉控件、截图回调、
文字几何辅助和 IME composer 已迁入。字体服务、国际化、印刷与业务资产未迁入。
版权、许可证及本地差异见 [源码来源](../../../crates/novadraw-ui/SOURCE.md)。

新增独立包 [novadraw-workbench-app](../../../crates/novadraw-ui/examples/native/workbench-app/README.md)，
通过公共 `DockContent<P>` 接口组合工具区、Canvas、Layers、Inspector 和 Notes。
Canvas 绘制真实 Novadraw Figure，与 eframe 共享 WGPU 30 设备。
工具状态与 Notes 用于演示工作台组合，未实现图形编辑命令。

## 关键适配与修复

WGPU 30 将提交画面改为 `Queue::present`，映射缓冲区读取改为 `Result`。
生产截图路径传播读取错误；像素测试显式断言读取成功。
egui 0.37 纹理增量按每个纹理的增量序列上传，保留更新顺序；
上传和释放完成后显式清空增量，宿主关闭时退役未发送的数据。
无 GPU 的 UI 回放使用独立 headless 入口，明确丢弃纹理上传；
生产宿主继续消费完整输出，不关闭上游的增量检查。
可访问性角色和文字对齐调用迁到当前 API。默认字体显式启用，
图标字体兼容空回退列表；文档标题按工具栏声明的控件高度分配响应区域。

空 glyf GPU 对比发现两处全透明像素的 RGB 残差，例如 `[15,15,15,0]`；
隔离复现也只涉及 Alpha 为 0 的像素。Vello 0.10 和 0.11 的 fine shader
均在输出前取消颜色预乘，浮点残差可能在 Alpha 量化到零后保留。
该测试改为统一完全透明像素的表示，继续逐字节比较 Alpha 和所有非透明像素，
未加入数值误差容限，也未修改生产 shader。

浮窗 ID 改用随机 UUID 源，原生配置和环境会话缓存由 Context 持有，
不保留上游的三个全局缓存。外部消费者测试发现直接构造的布局缺少重复 ID 校验，
已补上 `DockError::DuplicateWindow`；反序列化沿用上游身份去重。
原生浮窗每帧重建的调试诊断通过事件返回，绘制路径不打印日志。

`egui-phosphor` 保留现有版本，仅使用其图标常量和字体字节；
字体用 egui 0.37 类型重新注册，其 egui 0.35 传递依赖不参与 GPU 共享。
可选 Masonry/Xilem 宿主仍使用独立 Vello 0.8／WGPU 28，
不能与本次共享链路交换设备、纹理或渲染对象。
根 Cargo.lock 按原规则不纳入 Git；固定分支提交由 Cargo.toml 保存。

## 已完成的验证

| 检查 | 结果 |
|---|---|
| Vello 后端 `native,web` 与独立 Canvas `gpu` 编译 | 通过 |
| 现有 egui panel/workbench、Shapes 和 Logic 宿主编译 | 通过 |
| 上游迁入单元测试，启用 `serde,native-windows,testing` | 502 项通过 |
| 外部消费者：应用自定义面板、整批回滚、隐藏恢复、重复浮窗身份 | 4 项通过 |
| 文档示例 | 5 项通过，2 项保留为 ignored |
| Vello 后端单元测试 | 43 项通过，1 项保留为 ignored |
| 共享宿主单元测试 | 20 项通过；新增 GPU overlay 退役测试另行通过 |
| Vello 外部纹理像素／重建／错误目标与失效 session | 1 项 GPU 测试通过 |
| 宿主背景、Canvas alpha 和移除后的清除 | 1 项 GPU 测试通过 |
| 独立 Canvas 叠放、裁剪、透明度及旧区域清除 | 1 项 GPU 测试通过 |
| Web 平台、web-validation 与 UI `serde` 的 Wasm 编译 | 通过 |
| UI 全功能库及独立工作台 app 的 Clippy `-D warnings` | 通过 |
| `cargo check --workspace` | 通过，含独立 Masonry 宿主 |
| 原生工作台截图 | 2048 × 1280，主题背景、面板和真实 Canvas 已复验 |
| workspace Clippy `-D warnings` | 通过 |
| workspace fmt | 被并行 Editor／richtext 文件阻挡；迁移相关包的格式检查通过 |
| workspace test | 未通过；升级兼容问题已修，相关应用仍有 Core 投影预算失败 |
| 官方示例库复验 | 99 项通过、7 项投影预算失败、8 项 ignored；纹理增量丢弃错误已清除 |
| Shapes 库复验 | 29 项通过、2 项投影预算失败；控件中心线精确断言通过 |
| 文档检查 | 523 份 Markdown 通过 |

主要命令：

```bash
cargo test -p novadraw-ui --features serde,native-windows,testing
cargo test -p novadraw-backend-vello --test texture_contract -- --include-ignored
cargo test -p novadraw-backend-vello --test host_background_pixels -- --include-ignored
cargo test -p winit-canvas --features gpu --test gpu_contract -- --include-ignored
cargo check -p novadraw-platform-web -p web-validation -p novadraw-ui --features novadraw-ui/serde --target wasm32-unknown-unknown
cargo run -p novadraw-workbench-app -- --smoke-screenshot target/verification/workbench-final.png
```

截图和构建输出保留在本地 `target/`，不纳入 Git。首份截图确认真实 Canvas 和
面板绘制，暴露了顶部透明背景导致文字难读的问题；样例已改为绘制主题背景，
最终截图 `target/verification/workbench-final.png` 已确认修复。

全量格式检查在 `novadraw-editor/src/viewer/failure.rs` 与
`novadraw-richtext/src/layout.rs` 的并行改动处失败；本次未改动这两份文件。
按迁移相关包限定的格式检查通过。工作区编译与 Clippy
包含当前并行组合，第三方 `block 0.1.6` 另有 future-incompatibility 提示，
未作为本次升级修改第三方源码。

全量测试首次编译还暴露了 `p2_tree_query_identity` 的重复 move：
并行 Core 的 `FreeformError` 不再实现 `Copy`。第一处断言改用 clone，
保留原有比较语义，没有修改 Core 错误协议。
当前未关闭的应用失败均报告 `PreparationBudgetError`，限额为 2,000,000，
涉及 `projection Core record`、`projection owner scope` 或 `input active path`。
本次没有调整限额、跳过测试或改变投影协议。

## 验收边界

上游 native-window 测试使用模拟宿主，证明输入转发、焦点及拖动协议；
真实原生截图证明本机窗口和共享 GPU 合成。
真实 IME 候选窗口、跨显示器拖动，以及 Windows/Linux 原生行为仍需平台验收。
本次没有性能加速结论，没有关闭 Core／Editor 并行工作的里程碑。
工作区中同时存在投影和富文本改动，验证结果描述当前组合状态，
不能据此声称这些并行任务全部完成。
