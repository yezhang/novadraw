# Parley 上游迁移与补丁退役

类型：`implementation-verification`

能力归属：Core 默认文字适配器与第三方依赖。Editor、backend 和平台不新增能力。
日期：2026-11-05。状态：迁移实施完成；并发 GPU 精确像素门禁未关闭。
`api_semantics`：`text.flow`、`text.interaction`、`builtin.figures`；
验收依据为[Draw2D 语义账本](../../parity/draw2d/api-coverage.md)中的既有文字合同，
本次不新增 Core 或 Editor 公开能力。

默认适配器使用 crates.io 发布的 Parley 0.12.0，字体选择、塑形、断行、
对齐和文字交互使用上游接口。不另建断行引擎，也不把第三方内部计数器复制到 Core。

旧 Parley、Harfrust、Fontique 补丁主要服务于尚未启用的默认阶段实现和
补丁专用验证。迁移后删除这些源码、补丁与专用入口；历史报告保留原始证据。
Core 的通用 `analyze/reflow`、预算与资产合同保留，外部引擎仍按原合同接入。
默认 Parley 适配器不提供可中断的内部预算，也不声称已完成阶段预算门禁。

验证覆盖字体、混排、宽度断行、对齐、内联对象、光标和选区，以及 Native／Wasm
编译。版本迁移不预设性能收益，性能结论必须来自同条件采样。

## 已完成的替代

- 根 workspace 固定 `parley = "=0.12.0"`，不再指向本地源码。
  当前解析的 Parley Engine、Fontique、Harfrust 均为 0.12.0。
  可选 Masonry 自身仍依赖 Parley 0.8.0，不修改其内部文字栈。
- 方向使用 `set_base_direction`，删除隐藏方向字符、临时字符串与偏移修正。
  字体列表使用 `FontFamily`，内联对象显式声明 in-flow 与基线对齐；
  规范化坐标通过上游 `to_bits` 转成 Core 自有值。
- 行指标分别从内容区域和行框映射；对齐使用新版 `align`。
  不换行但存在容器宽度时，通过公共逐行断行和 `set_prior_line_width`
  保留显式换行并设置对齐行框。该宽度覆盖接口在上游标记为 `doc(hidden)`，
  因此单独验证多行居中、显式 RTL 和空文本，不依赖私有数据结构。
- 删除 35 份补丁、三个依赖源码副本、三个补丁消费者和 84 个专用 Python 工具；
  删除只服务于这些副本的 rustfmt 配置与旧操作准入合同测试。
- 新增 `p2_parley_public_api_contract`：复制后重排与新建布局比较覆盖中文、
  阿拉伯文、组合字符、连字、样式和内联对象；旧布局在上下文复用及销毁后仍可读。
  Core 阶段协议、预算、资产和独立外部消费者继续保留。

## 已观察到的验证

Core 单元测试首轮为 483 通过、4 忽略；公开合同首轮为 42 通过，
含上游公开 API 2 项、Core 阶段 20 项、图文协议 12 项和共享内容 8 项。
随后新增并通过“不换行窄容器、多行居中、显式 RTL 与空文本”回归，
当前上游接口合同测试共 3 项通过。
workspace check 已通过。文档门禁通过 502 份 Markdown，设计检查无错误；
50 个既有 ADR 引用候选保留人工判断。
Wasm Core／Web editor＋resource 编译和 workspace Clippy 已通过；
独立阶段消费者 20 项、共享内容消费者 8 项通过。
完整 workspace 首轮在后端空字形试验的 GPU 对照停下；
单列这一失败后的补跑为 1,962 通过、29 忽略、1 项过滤。
这不是完整门禁通过。最终无跳过的 workspace 复验仍在同一 GPU 对照失败，
完整门禁未关闭，workspace Clippy 复验通过。
随后运行 `cargo test --workspace -- --test-threads=1`，183 个测试 harness
合计 1,963 通过、29 忽略、0 失败、0 过滤，原 GPU 精确像素测试通过。
这轮没有跳过测试或放宽像素判据；串行全量验证通过，默认并发的 GPU 波动仍未关闭。
最后的 workspace fmt、Clippy、差异空白检查与 502 份文档检查通过。

这次迁移未运行窗口拖拽／resize 的匹配性能采样，不据此宣称性能收益，
也不关闭 FR-02 或阶段文字的整体性能门禁。

## 历史证据

三份补丁报告保留已提交基线的原始字节，旧依赖说明保留同一基线；
路径、提交与 SHA-256 见[归档清单](../../archive/parley-patch-retirement-2026-11-05.json)。
2026-11-05 的 Fontique／Parley 前段试验范围仍保存在
[第二批验证记录](draw2d-text-followup-2026-11-05.md)，其旧工具已经退役。
原始 `target/verification/text-*` 样本不修改。

## GPU 对照波动

`empty_native_glyf_domain_preserves_gpu_pixels_without_fragmenting_runs`
曾在未手动修改字形位置、空字形候选删除后的 GPU 对照失败：
16,384 字节中有 12 个 RGB 通道不同，最大差值 30，alpha 通道相同。
已有旧 Parley 0.7.0 测试产物通过同名测试；
新 0.12.0 产物先后出现失败和通过，不能据此确定是版本迁移或候选删除的根因。

曾试验只删除整段全空 run，混合 run 保留全部字形，对照仍失败；
该生产改动已撤回。随后同一 scene 重复渲染检查未复现差异，
候选对照仍一度失败，之后又通过。当前保留原生产路径与逐字节相等判据，
只给失败断言补入配置诊断；没有增加预热来改变原测试顺序。
原顺序连续三轮完整 GPU 矩阵通过，但这不构成波动已修复的证明。
随后默认并发的 workspace 复验再次失败，配置为 scale `0.75`、
无 skew、平移 `(3.25, -2.75)`、`AaConfig::Area`。
因此不能将三次独立通过作为完整门禁关闭依据。

失败后补充重复渲染诊断，不改变成功路径的调用顺序和逐字节判据。
默认并发后端测试中，同一 full scene 再次渲染与首次输出不同，
同一 filtered scene 也出现差异：`full_repeat=false`、
`filtered_repeat=false`、`repeated_pair=false`。本轮差异位于
`(27, 14)`、`(28, 14)`，RGBA 分别为 `[15, 15, 15, 0]` 与
`[0, 0, 0, 0]`。后端结果为 42 通过、1 失败、1 忽略；
同产物单独执行候选测试通过。这证明同 scene 的 GPU 输出也会波动，
不能把失败只归因于候选删除；尚未定位 Vello／驱动中的具体原因。
串行 workspace 完整验证通过，说明本轮串行执行没有复现该问题，
不据此声称并发就是根因，也不把串行参数写入项目默认测试配置。

Vello 0.10.0 的 `fine.wgsl` 在写入 RGBA8 前以
`1 / max(alpha, 1e-6)` 将预乘颜色转换为直通颜色。
该过程可能放大极小覆盖率差异，使 alpha 量化为零后 RGB 仍有差异；
这是与观察吻合的解释，尚不是驱动或着色器根因的证明。
不通过清零透明像素、放宽判据、预热、跳过测试或新增依赖补丁关闭此问题。

可读取的原始日志与 SHA-256 清单保存在
`target/verification/text-parley-upstream-migration-2026-11-05/`。
过期的工具日志在本地清单中列为缺失，不补造原始数据；
之前已观察到的结果保留为本文的验证摘要。
