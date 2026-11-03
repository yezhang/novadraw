# TextFlowLarge 独立研究应用：实施与验证

类型：`implementation-verification`

日期：2026-11-03。能力归属：应用／宿主（`textflow-large-app`），位于 Core
Native examples。Core、Vello backend、Winit adapter 无新增能力。
`api_semantics`：消费既有 `text.flow`、`text.interaction`、
`viewport.scroll_zoom`、`notification.layout_update`。
状态：应用已实现，定向测试、无窗口及 Native 固定回放通过；人工交互未验收。

## 交付与测量边界

独立 package 直接调用官方示例库公开的 `text_flow_large_example`；
正文、字体注册和 Figure 树没有复制或迁移。原综合示例与 `textflow-perf`
继续使用同一工厂。研究应用持有 Runtime、操作队列、有界日志及平台宿主，
使用既有输入 adapter、frame submission 和 completion 协议。

应用提供重建、追加 Z、固定滚动、resize、稳定查询、自动回放、timing 切换与导出；
CLI 可选无窗口、完整请求诊断、GPU queue wait、首帧截图及回放后退出。
运行命令和字段口径见
[应用 README](../../../crates/novadraw/examples/native/textflow-large-app/README.md)。

每项操作稳定化成功、对应帧获后端确认后才进入下一项。
Native resize 等实际窗口尺寸确认，Skipped／Retry 则返回窗口循环后再尝试。
失败不以旧快照替代结果；预算失败后以新 Runtime 恢复。
每轮重建前导出上一轮，文件不覆盖，跨文件由 `run_id` 与 `operation_id` 关联。
日志仍有 8192 条／32 MiB 上限；超限通过 dropped 明确报告。

所有研究帧请求完整重绘。CPU IR 准备覆盖 `prepare_submission`，
backend submit 与 GPU queue wait 分开记录；可选 Core timing 给出内部阶段。
因此本报告不代表默认增量 damage 路径的性能，也不宣布长文本延迟门禁通过。
Native Presented 是后端确认，queue wait 是队列同步，
GPU retained target 截图也不是 WindowServer 合成或屏幕 scanout 证据。

## 行为与回放证据

四项定向测试通过：

- 原工厂正文、追加、纯滚动、宽度变化与 reset 恢复。
- preparation work 预算为零时保留失败、快照为 null，新 Runtime 恢复。
- 后端重试和终止失败保留在日志中，正确消费 completion。
- 有界缓冲报告 dropped，JSONL 可解析，多次导出不覆盖原文件。

原文 40 段、51,232 字节，首 source 为 `Normal text.`，正文有效宽度 586。
追加后 51,233 字节；向下滚动 100 不新增 styled layout 调用；
视口宽度 400 时正文有效宽度 386；重建后恢复原文和初始宽度。
固定回放每轮 13 项：重建、追加、滚动及回滚、稳定查询、
550 → 500 → 450 → 400 → 450 → 500 → 550 → 600。

| 路径 | 轮数／操作 | 成功帧 | 附加结果 |
|------|------|------|------|
| 无窗口、timing | 2／26 | 26，`cpu_recorded` | 每轮独立导出，零丢弃 |
| Native、timing、GPU wait | 2／26 | 26，`native_backend_presented` | 首帧另有 2 次 Skipped；26 次 wait 成功，零丢弃 |
| 无窗口、完整 requests | 首次场景／1 | 1 | 520 条完整请求 Debug，零丢弃 |

Native 使用 Apple M1 Pro／Metal，逻辑视口 600 × 500，DPI scale 2。
两轮尺寸序列与无窗口路径一致；操作、实际帧尺寸、提交确认和 queue wait 均可关联。
已通过 LaunchServices 打开构建产物，并确认确切 bundle 内的进程持续运行。
首个成功帧的 1200 × 1000 GPU retained target 已查看，长文本、不同字号、
横纵滚动条和裁剪区域可见。

验证中发现并修复了两项宿主问题：多轮诊断原本堆积到缓冲上限，
改为每轮落盘；Native 原本在同一回调内连续重试，
改为跨窗口事件推进，并在 occluded 状态等待恢复。
原失败及溢出记录仍留在本地目录，不能与最终验证记录混用。

## 自动检查

使用 Rust 1.96.0，workspace `cargo check`、workspace fmt、
应用 bin 的 Clippy（`-D warnings`）及四项定向测试通过。
文档类型、索引、链接和命令引用通过 xtask docs 检查。
macOS launcher 复用既有文本验证 CLT 构建配置；没有修改全局 Rust 或 Git 配置。

## 本地原始产物

以下路径相对于仓库根目录，均在忽略的 `target` 下：

- 无窗口最终：`target/verification/textflow-large-app/headless-final/`
  中 `textflow-1793698114881163000-98256-0.jsonl`、
  `textflow-1793698118975971000-98256-0.jsonl`。
- Native 最终：`target/verification/textflow-large-app/native-final/`
  中 `textflow-1793698148165621000-4363-0.jsonl`、
  `textflow-1793698153286288000-4363-0.jsonl`。
- 首帧 GPU 截图：同一 Native 目录下 `native-1793698143308364000.png`。
- 完整请求：`target/verification/textflow-large-app/requests-check/`
  中 `textflow-1793698253588132000-22225-0.jsonl`。

这些调试运行用于验证采集入口与口径，没有代表性样本量和匹配前后版本；
不能从其耗时推断优化收益。人工鼠标与键盘、跨显示器 DPI、IME、
系统 AT、WindowServer／屏幕呈现和其他平台原生交互仍未验收。
应用建设不改变 ADR-059 B1 暂停状态；FR-02 贡献复用合同仍是后续引擎研究。
