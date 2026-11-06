# 富文本正文节点与段落布局复用验证

类型：`verification`

能力归属：Editor 发布正文和装饰；richtext 复用段落布局与高度索引；
应用证明正文贡献等价；Core 与 Vello 继续承担记录、damage 和提交。
日期：2026-11-06。

状态：两个优先项已实施，定向回归和匹配编辑采样通过；
点击到 GPU 完成未明显改善，屏幕呈现延迟、可见正文绘制和 giant 验收保持开放。
协议见[正文反馈与段落布局复用](../../design/editor/richtext-incremental-feedback.md)，
问题基线见[点击呈现诊断](richtext-click-presentation-diagnosis-2026-11-06.md)。

## 实际改动

Editor 增加默认关闭的 `EditSessionPlan::reuse_feedback`。
Plan 对前后控制器和模型证明正文等价后，Viewer 在只读正文上准备候选几何。
正文 FigureId 与准备资产保持不变。没有 selection 和 preedit 时，Viewer 也保留
已有 caret FigureId，在发布阶段更新 bounds 和显隐。可恢复拒绝保留旧控制器；
未知状态和 panic 清理会话。plain 适配与外部控制器使用同一协议；
现有 plain Plan 默认不声明复用。

richtext 的文档候选 fork 两条容量的缓存条目，共享不可变旧快照，不共享淘汰操作。
环境匹配后，仅复用 Arc 身份未变的段落布局；文字、span、样式、对齐或间距变化
会改变段落 Arc，重新排版该段。宽度、字体 generation、引擎身份、布局方向变化
不复用段落布局。新快照重新准入位置，同顺序段落只更新变化高度的索引路径。
准备失败不发布部分缓存；旧快照仍可查询。元数据扫描仍为 O(段落数)。

正文仍不展开为 Figure/EditPart 子树，绘制仍提交全部段落。
selection/preedit 的装饰组合切换仍可能替换装饰节点并触发保守完整修复。
没有修改 Core 的完整 damage 保护或渲染主循环。

## 匹配方法

探针直接编译当前应用的 `editor.rs`、`embedded.rs`、`native.rs` 和 `performance.rs`。
两个 release 二进制使用同一引擎源码和依赖：Parley 0.12.0、Vello 0.11.0、
WGPU 30.0.1；默认 rustc/cargo 1.99.1。基线适配器关闭节点复用，并在文档改变时
创建无历史布局缓存的正文 Figure，重现整改前的行为。它不是旧 commit 的复跑；
两组共享本次段落资产表示和当前并行 Core 改动。

最终四组编辑回放与两组 GPU 回放串行执行，关闭 tracing。其他工作区任务可能仍
影响机器负载，结果不作为跨机器预算承诺。中间的并行试采样保留在本地产物目录，
下表只用 `final-*` 数据。

CPU 编辑每组 216 次：首、中、尾各 24 次插入、替换、删除；定位和选区设置排除。
两组分别为 mixed 20,772 字符与 large 274,772 字符。输入耗时包含候选克隆、文档
编辑和反馈布局；帧耗时再包含 CPU 命令记录，不含 GPU。
表中 P50 取排序后的 `len / 2` 项，前后使用同一规则。

## 编辑成本

单位 ms。每行 72 个样本，合并首、中、尾的同类操作。

| 负载 / 操作 | 输入与反馈 P50 前 → 后 | 输入与反馈 P95 前 → 后 | CPU 帧 P50 前 → 后 |
|---|---:|---:|---:|
| mixed 插入 | 8.367 → 0.636 | 9.648 → 0.748 | 10.207 → 2.306 |
| mixed 替换 | 8.268 → 0.615 | 9.737 → 0.771 | 10.159 → 2.291 |
| mixed 删除 | 8.258 → 0.609 | 11.752 → 0.781 | 10.162 → 2.394 |
| large 插入 | 106.983 → 1.520 | 129.568 → 2.102 | 131.778 → 25.753 |
| large 替换 | 106.172 → 1.506 | 156.847 → 2.289 | 131.673 → 25.946 |
| large 删除 | 106.701 → 1.539 | 134.958 → 1.851 | 132.965 → 25.800 |

插入的输入与反馈 P50 分别下降约 92.4% 和 98.6%。
large 的 CPU 帧仍约 26 ms，主要剩余成本包括全文命令记录。
这些结果适用于多个硬段落的 large；giant 将长文放在同一硬段落，
一次编辑仍需重排该长段，不能沿用 large 的改善比例。

## 点击与像素

GPU 回放使用 1120 × 760 逻辑窗口、2240 × 1520 离屏纹理和真实 Vello 管线。
首、中、尾分别在两个相邻字位间切换，共 24 次 press/release。
每帧串行等待 GPU 完成；这是合成输入到 GPU 完成，不含 OS 输入排队、
WindowServer 和显示扫描。重复点击同一字位可以正常返回 Idle，
不能强迫空更新产生帧来采样。

| 阶段 | P50 前 → 后，ms | P95 前 → 后，ms |
|---|---:|---:|
| 输入及反馈 | 0.538 → 0.332 | 0.616 → 0.422 |
| CPU 帧准备 | 2.007 → 2.138 | 2.349 → 2.443 |
| 后端同步提交 | 39.409 → 40.233 | 45.427 → 48.524 |
| 提交后 GPU 完成等待 | 21.083 → 20.273 | 24.559 → 23.395 |
| 输入到 GPU 完成 | 64.132 → 63.423 | 67.861 → 69.372 |

两组均为 24/24 排版快照复用，4,488 条命令。基线为 24 个 Full 帧，优化后为
24 个 Partial 帧；正文和 caret 节点保留已经生效，但缩小 damage 没有减少正文
命令量，后端编码成本没有明显下降。不能据此宣布点击卡顿已解决。

分别在每组最后一个点击后直接导出 retained texture，再对同一状态完整重绘。
基线有 12 个像素的 RGB 通道差值为 1；优化组有 110 个像素的通道差值为 1，
其余像素一致，均没有大于 1 的通道差值。按实测完整重绘自身的 1 级色值差异
检查，局部帧未见残影。两张优化组图像另做了视觉检查，正文、Emoji 和 caret
可见。此处是各组同一状态的 retained/full 对照，不声明两次独立回放的视口
逐像素相同，也不关闭长文黑帧或全部 DPI 组合。

## 回归与门禁

- richtext 库 10 项通过：段落排版调用计数、未变布局 Arc 复用、插入/替换/删除
  与完整重排的尺寸、下游 caret 和跨段 selection 一致；旧位置失效、旧快照存活、
  宽度/方向/字体/引擎失效、缓存容量和失败预算。
- 独立外部编辑消费者 12 项通过：正文和 caret 节点保持、候选拒绝、不允许旧正文
  写入、geometry panic 清理、输入归属与模型历史。
- richtext controller 3 项通过：Emoji 字素删除、composition 回滚与提交、
  span 保留、候选隔离和 NoChange。
- mixed 应用 Headless 通过：同版几何、命中、滚动、resize/zoom、IME 回放、
  stale lease、取消及一项 Command 的 undo/redo。
- 两个库的 scoped Clippy `--no-deps -D warnings` 通过；格式和设计文档检查通过。

plain 原始回归中两项要求 `damage.union()` 存在，但当前 Core 在删除旧投影时
允许 Full 修复，Full 没有 union。这两项现改为明确接受 Full；若返回局部范围，
仍检查原来的 viewport 或旧新覆盖。没有放宽 caret、裁剪和正文几何断言。
最终 root plain 回归 10 项通过，Native package check 通过；
本切片合计 35 项定向自动化测试通过。

中途 root Cargo 解析被并行新增的 `masonry_imaging` 依赖阻断，先用独立 manifest
验证同一份源码；随后 root 的两个库 check 已通过。并行配置不属于本切片修改。

## 本地产物与后续范围

本地产物位于 `target/verification/text-richtext-incremental/`，不纳入 Git：

- `harness/` 保存 manifest、lock、基线适配器和前后 release 探针；
- `final-before/after-mixed-edit/` 与 `final-before/after-large-edit/`
  保存 `session-samples.json`、`session-summary.json` 和 metadata；
- `final-before/after-mixed-gpu/` 保存点击 samples/summary、
  `retained-click.png` 与 `full-reference.png`。

目录名中的 `before/after` 在磁盘上分别展开为 `before` 和 `after`，
例如 `final-after-large-edit/`。删除 target 后需重建探针。

下一项性能工作是让未变正文的记录和编码成本不再随全文长度增长，
并按真实墨迹、设备覆盖及旧新修复范围验证可见输出。
Core 的[局部录制方案](../../design/architecture/figure-drawing-recording.md)
仍需遵守自己的独立复核门禁，本切片未提前接入它。
首次会话准备、giant 段内增量、人工 IME、Bidi/DPI 组合、屏幕呈现延迟与 FR-02
整体验收继续开放。
