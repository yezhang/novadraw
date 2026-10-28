# GEF 样例接入的 Scene 输出实施记录

类型：`implementation-verification`

日期：2026-10-28。契约：[ADR-053](../../adr/adr-053-embedded-vello-scene-target.md)。
授权来源：用户要求“好的，记录这个方案，然后开始实施”。
本记录只关闭 Scene backend 的首个前置切片，不关闭完整 GEF 样例或 UI 接入。

## 已实施的边界

在具体 backend 新增
[`scene::VelloSceneRenderer`](../../../crates/novadraw-backend-vello/src/scene.rs)与
scoped `SceneBackend`。消费公开 `RenderSubmission`，输出 caller-owned 的逻辑坐标 Scene，
复用原 SceneEncoder、资源/session gate 和抗锯齿预处理。
未新增第三方依赖，未修改 Core/Editor 或渲染主循环。

目标仅完整替换；失败、Retry、Skipped 不替换旧目标。
显式背景绘制在画布局部，默认透明。设备空间编码经逆 DPI 返回逻辑坐标，
宿主最终施加同一 DPI 和平移。
`Presented` 表示 Scene 目标发布，不代表窗口 GPU/present 完成。
完整 UI、任意额外宿主 transform、capture、IME 和 AT 尚未认证。

独立消费者在
[`tests/consumers/vello-scene`](../../../tests/consumers/vello-scene/README.md)，
使用自己的 Cargo workspace、lockfile 和公开依赖，没有私有实现访问。
integration test 同样只从公开 Runtime/RenderBackend 构造提交。
本项目已迁移到 `engine/novadraw`，公开 crate 路径为 `crates/`。

## 验证结果

| 范围 | 结果与限制 |
|---|---|
| Scene 契约 | 4 项通过，包含显式运行的原生 GPU 项 |
| 失败保持 | partial damage、非法 DPI、逆 DPI/extent 溢出、空尺寸、旧 session、缺 snapshot、资源缺失 Retry、OFF 预算耗尽均不替换旧 Scene |
| 重复合成 | 同输入再次提交替换目标，不追加多份旧帧 |
| GPU 对照 | DPI 1/1.5/2 × 透明/蓝色背景 × 初始/移动后两代，共 12 帧，Scene 与纹理后端逐像素一致 |
| Host composition | 8 个物理像素平移、canvas 边界 clip、canvas 外透明、第二代旧位置擦除 |
| 既有 backend 单元回归 | 39 项通过；原有 release cost 项按配置 ignored |
| 既有纹理 GPU 契约 | 显式运行 1 项通过，target 校验、full replacement、session reset 和旧像素擦除 |
| Clippy | backend `--lib --test scene_contract` 与独立消费程序均严格通过 |
| 格式/文档 | workspace fmt check 与文档门禁通过；未运行无关 workspace 全量测试 |

测试入口：

```bash
cargo check -p novadraw-backend-vello --lib
cargo test -p novadraw-backend-vello --test scene_contract -- --include-ignored
cargo test -p novadraw-backend-vello --lib
cargo test -p novadraw-backend-vello --test texture_contract -- --ignored
cargo clippy -p novadraw-backend-vello --lib --test scene_contract -- -D warnings
cargo clippy --manifest-path tests/consumers/vello-scene/Cargo.toml --features scene -- -D warnings
cargo fmt --all --check
cargo xtask docs
git diff --check
```

没有窗口人工操作证据，没有编译/运行 Java，也没有将规划文档作为实现证据。
本次验证是本地工作区快照；仓库正有其他迁移/长文本改动。
工作期间检测到并发提交 `93182988` 已包含 Scene 输出源码；
本会话没有执行 git commit，最终验证补记仍有工作区改动。

## 有限成本测量

环境为本地 macOS、`rustc 1.94.1 (e408947bf 2026-03-25)`，原生 GPU 可用。
测量程序建立 100 个矩形子节点，DPI=2，准备一次 submission，1310 条命令，
Scene 输出为 505 paths。计时只覆盖编码与完整目标替换，不包含 Runtime 准备、
模型编辑、排版、GPU、窗口提交或 Xilem rebuild。

同一 standalone lockfile、profile、target，分别启用/禁用 `scene` feature。
profile 为 opt-level=s、LTO、单 codegen unit、panic=abort、strip=true。

| 最小消费程序 | 原始字节 | gzip 字节 |
|---|---:|---:|
| Runtime 准备，无 backend 编码 | 2,075,088 | 1,004,136 |
| Runtime + Scene backend | 2,854,112 | 1,377,672 |
| 增量 | 779,024 | 373,536 |

这是最小消费应用保留 Scene lowering 的产物增量，不是给完整 UI 添加一个方法的
边际成本，更不是 WASM 下载大小。
当前仅保留一组本地消费测量，不能据此推断不同机器的尾延迟。

| 编码时间 | 微秒 |
|---|---:|
| 本进程首次编码 | 176.500 |
| 1000 次同源 warm p50 | 105.209 |
| warm p95 | 142.042 |
| warm p99 | 310.625 |

首次值只有一次观测；warm 百分位取排序后索引 500/950/990。
首测在构建进程同时活动时取得，调度争用可能影响尾延迟。
没有 RSS/GPU memory 或完整 UI 的启动测量。
原子替换保留旧目标与候选，短期内存峰值仍需后续 Widget 消费场景测量。
消费构建缓存已移至 `/tmp/novadraw-gef-scene-consumer-target-20261028`，
源码只保留消费者及 lockfile。

## 后续切片

当前公开 Scene 类型属于 Vello 0.10；本地 Masonry/Xilem 基线仍为 Vello 0.7。
不能直接互传。下一步固定兼容依赖，再实现 Masonry 自定义 Widget 与 Xilem View，
验证稳定 session/rebuild、输入仲裁、文本 bridge、尺寸/DPI、capture 和失效清理。
本次不修改第三方参考仓库、不创建占位 UI crate。

Shapes 的 CreationTool、MarqueeTool、模板创建和 UI 状态接入仍需实施。
GEF 账本中 `interaction.create` / `interaction.selection` 的 partial 状态保持原样；
完整样例 70 项能力和 6 项支持检查的完成比例未变化。
执行状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
