# Masonry 上游迁移与无 vendor 验证

类型：`implementation-verification`

2026-10-28。用户授权：“按这个思路来吧，目标是删除本地 vendor 目录仍然能够完成 gef 样例的验证。”
本次完成现有 Shapes 消费者的依赖迁移和验证，完整 GEF 官方样例仍按
[P2 队列](../../roadmap/p2-delta-backlog.md)推进。

## 结果与依赖边界

`vendor/` 已删除。Masonry、Xilem 及其相关包统一使用未修改的上游 Git 提交
`66c8893f12232b9d5cdf4296f773aa3dc07ee776`；Cargo 管理下载、缓存和锁文件，
构建不引用本地 Xilem 参考仓库。原补丁说明按原文保留在
[依赖记录](../evidence/masonry-vendor-baseline-2026-10-28.txt)。

UI 包要求 Rust 1.96。本次安装 `1.96.0`，没有改变全局默认工具链。
上游使用 imaging 0.0.1、Vello 0.8 / WGPU 28、Parley 0.8；
Core 使用自己的 Parley 0.7，现有 Vello 0.10 / WGPU 29 后端保持原有版本。
GEF demo 的常规依赖树只有上游 Vello 0.8 / WGPU 28，不再依赖 Novadraw Vello 后端。
根 workspace 保留 35 个默认成员，其中 UI 包数量为 0。

## 实施契约

[平台适配包](../../../crates/novadraw-platform-masonry/README.md)内的独立
`RenderBackend` 消费 Core 的 `RenderSubmission`，发布逻辑坐标 imaging recording。
Core/Editor 的公共 API、业务所有权和渲染主循环没有为本次迁移修改。

- Widget 接入上游的 `Painter`、`Length` 和 `UpdateCtx::scale_factor`。
- layout 通过公开延迟 mutation 请求一次动画；paint 只重放已发布 recording。
- Xilem View 采用 `&mut State`，保留 build/sync/action、会话身份与 teardown 路由。
- `host::run` 委托公开 `MasonryState`，DPI / exposure 变化请求一次树动画。
- 资源和 recording 共同暂存，失败、旧 session 或缺资源 Retry 均保留旧目标。
- 图像资源复用稳定的 Blob 身份和共享像素负载，不在每帧复制像素或进行 GPU readback。

支持默认 AA 的图元、路径、裁剪、完整描边、渐变、图像源矩形和 Core 默认文字轮廓。
imaging 0.0.1 无法完整表示逐操作 AA 和原生 glyph brush/dash，
因此明确不声明 `PerOperationAntialias` / `GlyphRuns`。相关命令会返回失败，
没有按默认绘制静默替代。现有 Vello 后端的相应能力仍可使用。
当前证据不能外推为该新目标已经覆盖 Vello 后端的全部能力。

当前 [接入契约](../../design/editor/masonry-xilem-canvas-integration.md)已同步这些边界。
[迁移前评估](xilem-masonry-upstream-reassessment-2026-10-28.md)保留原判断与时间范围。

## 自动验证

在删除 vendor 后执行；[原始日志和 GPU 截图](../evidence/masonry-upstream-2026-10-28/)
保留完整结果。7 项 GEF 消费测试和 4 项适配契约测试通过。
另有 1 项 release 成本采样默认 ignored，本次已显式执行并通过。

| 范围 | 核验结果 |
|---|---|
| 编辑与 history | Widget/headless 拖动一致，只有一个 history entry；创建、删除、undo/redo 通过 |
| View 与 lifecycle | 稳定 Widget/Editor/Runtime 身份；消息、teardown、禁用、失焦、capture 取消通过 |
| 工具栏 | 实际 action → message/rebuild → Undo，通过模型与资格检查和 GPU 合成 |
| GPU / DPI / resize | DPI 1、2 的拖动与撤销像素通过；1、1.5、2 的 resize/DPI 序列保留 source/history |
| 绘制目标 | 捕获裁剪变换与 restore、图像 revision/源矩形、缺资源 Retry、旧 session、AA 拒绝保留旧目标 |
| 宿主调度 | 嵌套 Widget 在 DPI 变化后读取新 scale，随后不持续 tick |
| 静态检查 | UI 两包严格 Clippy、workspace fmt、diff whitespace、文档引用门禁通过 |

新上游 TestHarness 的截图使用 CPU renderer。本次只复用其输入与布局，
独立 GPU harness 将公开 `VisualLayerPlan` 交给 `imaging_vello` 的真实 WGPU renderer。
测试禁止 `SKIP_RENDER_TESTS`，没有用 CPU 截图替代原 GPU 证据。
上游 `mouse_move` 已改为逻辑坐标输入，测试相应调整；GPU 像素断言仍使用物理坐标。

```sh
cargo +1.96.0 test --locked -p gef-editor-demo -p novadraw-platform-masonry --lib -- --test-threads=1
cargo +1.96.0 clippy -p gef-editor-demo -p novadraw-platform-masonry --lib --bin gef-editor-demo --bin ui-shell-baseline -- -D warnings
cargo +1.96.0 fmt --all -- --check
cargo +1.96.0 xtask docs
```

## 有限成本测量

macOS arm64、Rust 1.96.0，同一 release 配置和锁文件。UI shell 与 editor 都使用
同一个宿主桥；前者不实例化业务编辑会话。该测量只说明 Native 可执行文件增量。

| 产物 | 原始字节 | gzip-9 字节 |
|---|---:|---:|
| UI shell | 9,975,088 | 3,923,665 |
| Shapes editor | 14,298,864 | 5,776,155 |
| 增量 | 4,323,776 | 1,852,490 |

640×480、两图形、DPI=1；1020 次拖动/撤销，丢弃前 20 次，保留 1000 次。
不包含 GPU 光栅化、surface present 或窗口交付时间。

| CPU 路径 | p50 ms | p95 ms | max ms |
|---|---:|---:|---:|
| 四次 pointer event | 0.071 | 0.089 | 0.252 |
| recording 准备 | 0.043 | 0.054 | 0.126 |

旧 vendor 记录使用 Rust 1.94.1 和旧依赖，不能把两次差值全部归因于 imaging。
这些单机采样不替代正式性能门禁，也不证明 Web 体积、RSS 或 GPU memory 成本。

## 原生启动与剩余验收

标准 macOS 启动脚本已显式选择 UI 工具链；保留原 Run action。
最终应用经 LaunchServices 启动，进程路径与当前构建 bundle 的真实路径一致，
并在后续检查中保持运行。构建目录沿用已有 `target/masonry` 的缓存软链接。

本轮没有可用的桌面操作工具；System Events 查询未获得应用窗口。
因此记录“原生进程已启动，画面与交互未核验”，不以离屏截图补充原生首帧结论。
原生拖动、undo/redo、跨显示器 DPI、IME、系统 AT 与完整 GEF parity 的剩余范围
保持未完成。`block 0.1.6` 的上游 future-incompatibility 提示也保留，未修改第三方包。
