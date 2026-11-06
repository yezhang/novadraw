# UI 框架切回官方 crate

类型：`verification`
能力归属：其他——本地 UI、示例宿主与原生输入依赖。日期：2026-11-06。

用户要求本项目不引用其他项目的框架定制分支。egui 全族、eframe 和 winit
已改用 crates.io 官方发布版，删除了根 Cargo.toml 中的 12 项 Git patch。
本地工作台代码继续维护在 `novadraw-ui`，不依赖 craft crate。

## 当前依赖

共享 GPU 链路保持 Vello 0.11／WGPU 30；egui／eframe／egui-winit／egui-wgpu
使用官方 0.37.0，winit 使用官方 0.30.13。ecolor、emath、epaint、
默认字体、extras 和 kittest 随官方依赖图解析。不需要下游复制任何 patch。

可选 Masonry/Xilem 仍引用 linebender 官方仓库的固定提交，并非其他项目的定制分支。
所需 `xilem_masonry` 和 `masonry_imaging` 拆分包尚未发布到 crates.io。
官方发布的 xilem／masonry 0.4 使用另一套 API，不能用改版本号或包名别名保持
现有宿主协议。本轮未迁移这部分，也不声称仓库已完全没有 Git 依赖。

## 本地适配

官方 egui 的 `IMEOutput` 提供用途、编辑区、光标区和中断请求，没有定制分支的
owner、context generation 或 mutation receipt 字段。
composer 只发送官方字段；编辑目标身份保存在本地窗口记录中，
用于同一窗口 pass 的发布与撤回。Core／Editor 不增加 GUI 依赖。

移除定制原生重置确认事件和通过 Debug 字符串识别按字节删除事件的逻辑。
测试改为实际验证官方字符删除事件、描述符启停和焦点事件顺序，
没有用提前返回或隐藏测试模拟官方不提供的能力。
原生取消完成 fence、context generation 和按字节删除事件不在当前验收范围内。
真实 IME 的平台行为仍须单独验收。

Combo／Menu 的 RTL 判断使用官方 `Ui::layout().prefer_right_to_left()`，
不读取定制的全局 Style 字段。停靠、浮窗、事务、权限、工具栏、截图回调、
候选窗口定位及已有组合状态机继续使用同一套本地代码。

## 验证

| 检查 | 结果 |
|---|---|
| 官方 egui 下的 UI 单元测试 | 502 项通过 |
| UI 外部消费者和文档示例 | 4 项消费者、5 项文档示例通过；2 项既有 ignore 保留 |
| workspace 编译／Clippy | 通过；UI 全功能库 Clippy 也通过 |
| 官方依赖来源审计 | GUI 全族与 winit 全部为 crates.io registry；Git 来源仅为 linebender/xilem 官方仓库 |
| Wasm | UI `serde,native-windows`、Web 平台与 web-validation 编译通过 |
| Native 示例 | 官方 crate 构建、窗口启动和 2048 × 1280 截图复验通过 |

原生截图保存在本地 `target/verification/workbench-official.png`。
来源审计使用完整 `cargo metadata`，包含所有 workspace 包和传递依赖。
`egui-phosphor` 保留官方发布版本，仅使用其常量和字体字节，
其传递 egui 0.35 同样来自 crates.io，不参与共享 GPU 对象。

核心命令：

```bash
cargo test -p novadraw-ui --features serde,native-windows,testing
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo check -p novadraw-ui --features serde,native-windows --target wasm32-unknown-unknown -p novadraw-platform-web -p web-validation
```

本记录替代[首轮定制分支基线](ui-workbench-migration-2026-11-06.md)中的当前依赖结论；
首轮结果保留为历史证据。Core 投影预算失败与并行代码的格式状态
不在本轮调整范围内，不通过提高预算或忽略失败来关闭其门禁。
