# 官方样例 Core 增量实施记录

类型：`implementation-verification`

日期：2026-10-27

用户已批准 [能力补充方案](../../design/architecture/official-example-capability-increment-2026-10-26.md)
中的方向上下文、低 miter、Cocoa 零宽映射和通用下划线。本页记录批准后的实施，
保留 [前一批消费者修正](official-example-capability-increment-2026-10-26.md)
与功能审计的历史快照。通用设备 hairline 不在本次范围内。

## 方向上下文

[ADR-048](../../adr/adr-048-inherited-layout-direction.md)已落地：
普通 FigureStyle 保存可继承的 LayoutDirection，Builder、子树描述和 scoped
Runtime mutation 共用原样式协议；属性通知使用类型化身份。
LayoutSceneQuery、EventSceneQuery 与 Flow 的冻结样式提供同一解析结果。
StyledTextRequest 保存方向并参与版本，方向修改使子树布局和文字准备失效。

镜像排版交换水平对齐与行内片段位置，保留片段内字形朝向。
glyph、inline box、caret、selection 和 hit testing 使用同一片段平移。
run 边界的 caret affinity 与视觉方向键跨片段移动已覆盖。
ScrollPane 的纵向滚动条随方向换侧，横向滚轮与方向键读取相同方向；
纵向 RangeModel 的共享语义保留。Runtime 输入回归验证 RTL 下横向滚轮增量、
ArrowRight 的反向滚动，以及恢复 LTR 后的同键正向滚动；纵向增量不随镜像反转。

BidiBlockExample 左右两棵树分别使用 LTR/RTL 布局上下文。
独立外部文字引擎消费者验证请求传递、方向修改及旧 caret revision 拒绝。
继承与局部覆盖、两种段落方向、五种对齐、窄宽换行和 inline box baseline
均有定向测试。未修改递归渲染主循环，没有增加依赖或字体。

GPU 离屏测试覆盖方向切换、收缩与恢复后的 Partial/Full 逐像素一致性。
三张场景图位于 `target/verification/official-example-capability-increment/bidi/`；
这是本机 Vello 的更新一致性证据，不能替代上游栅格或其他平台的呈现验收。
官方入口完成计数暂不调整。

验证入口：`cargo xtask verify text.layout-direction`。
原 styled/structured flow 和独立 ASCII 引擎继续作为回归分母。
该 suite 覆盖 46 项测试，包括独立运行的 GPU 用例；
外部 LayoutManager 消费冻结方向的新增契约 target 共 6 项全部通过。
普通样例测试中忽略的该用例不计为未执行。
Core `--lib` 与新增契约 target 的严格 Clippy 已通过。
已查看恢复尺寸的 GPU 图像，未发现字形反射。

## 低 miter 与 Cocoa profile

[ADR-049](../../adr/adr-049-low-miter-and-cocoa-sample-profile.md)已接入受检 StrokeStyle、
普通 Vello 与 AA OFF 路径。原始 limit 保留；低于 1 的 Miter 使用有效 Bevel join，
visual outset、Figure bounds 和旧新 damage 消费同一扩张。
既有命中采用中心线与 tolerance，不新增未经验证的尖角命中承诺。
Core 零逻辑宽度仍不绘制；ShapeStyles 显式映射为 Cocoa profile 的 1 逻辑单位，
并使用四套上游 ZERO dash 数组。控制面板公开说明 profile。

`cargo xtask verify render.low-miter` 已通过 24 个 Graphics 契约、4 个样例测试，
以及独立 GPU 测试的 16 个低 miter/Bevel 对比组合。
组合覆盖 AA ON/OFF、开闭路径、反向与 0/0.5，逐像素相等。
40 个 ShapeStyles 矩阵与 Web 编译另由 `render.antialias` 复核。

## 通用下划线

[ADR-050](../../adr/adr-050-prepared-text-underlines.md)已接入 MeasureContext 与
公开 Graphics。实际 glyph run 的 face/revision/index、variation 和 size
决定 FontMetrics，StyledGlyphRun 保存实际 baseline 和 advance bounds。
每个逻辑范围查询一次选择几何，按行合并区间，再与实际 run 相交；
owned PreparedTextUnderlines 保存版本、颜色、指标与矩形，绘制不重查字体。
缺失指标、字体替换后的旧 face、不匹配的外部指标与无效 grapheme 端点明确拒绝。
OutlineTextEngine 透传相同指标协议。

PathExample 替换原单字体算法，仍保留选择背景、白色选字和两次文字绘制。
外部 Figure 合并装饰 ink 与逻辑范围，使用既有 preparation/damage 更新；
混合字体与 RTL 消费者完成收缩、恢复、字号修改及移除后的 GPU Partial/Full 比较。
三张图在 `target/verification/official-example-capability-increment/underlines/`。

`cargo xtask verify text.underlines` 已通过 3 个新契约、2 个字体/ICO 契约、
2 个 Path 测试与独立 GPU 回归。release 成本测试默认忽略，单独采样。
这些证据证明本机更新一致性，不代表跨平台或上游原生栅格全部对等。

## 性能与产物成本

release 采样覆盖 4,992 UTF-8 字节、672–683 个实际 run，宽度 120/600，
以及 LTR/RTL 布局上下文。成本测试发现初稿对每个 run 重查完整选择几何，
镜像边界还会输出重复片段；修正后每个输入范围只查一次，按实际 run bounds 拆分。
修正前快照保存在 `cost-before.json`，不能作为可交付版本的性能。

| 镜像 | 宽度 | run / 下划线片段 | 排版 P95 ms | 下划线准备 P95 ms | 命令记录 P95 ms |
|---|---|---|---|---|---|
| 否 | 120 | 683 / 683 | 2.687 | 0.292 | 0.416 |
| 否 | 600 | 672 / 672 | 2.958 | 0.243 | 0.727 |
| 是 | 120 | 683 / 683 | 2.236 | 0.349 | 0.463 |
| 是 | 600 | 672 / 672 | 2.170 | 0.207 | 0.351 |

初稿准备 P95 为 42.54–45.07 ms，修正后为 0.207–0.349 ms。
命令记录包含状态与填充命令，共 1,348/1,370 条；这些是 CPU 准备/记录成本，
不含 GPU 提交，也不构成长文或 60 fps 承诺。静态帧可复用准备结果。
运行入口：
`cargo test -p novadraw --release --test p2_text_underline_contract underline_direction_preparation_cost -- --ignored`。

产物探针固定 `opt-level=s`、LTO、单 codegen unit、strip 与 panic abort。
基线从 `bf6409d0` 的只读归档构建；当前产物分别不调用和调用镜像/下划线接口。
三者使用相同两套已有字体，没有增加依赖或内置资源。

| 探针 | Native dylib 字节 | Wasm 字节 | Wasm gzip 字节 |
|---|---|---|---|
| 基线 | 21,142,832 | 20,876,816 | 12,716,633 |
| 当前、未启用 | 21,142,848 | 20,886,246 | 12,720,399 |
| 当前、启用 | 21,175,904 | 20,926,454 | 12,735,323 |

未启用相对基线增加 Native 16 字节、Wasm 9,430 字节、gzip 3,766 字节；
启用增加 Native 33,072 字节、Wasm 49,638 字节、gzip 18,690 字节。
这是 Core 消费者探针的同配置结果，不代表 Vello 和完整样例的最终下载体积。
Web 的传输增量以上述 gzip 差值为依据；浏览器冷启动仍待独立验收。
字体初始化探针每次新建 TextSystem，复用已映射的动态库，不等于浏览器首次加载。

所有原始 JSON、构建日志、探针源码和产物位于
`target/verification/official-example-capability-increment/`。

## 收尾门禁

| 门禁 | 最终结果 |
|---|---|
| workspace fmt / 严格 Clippy | 通过 |
| workspace test | 1,516 passed，0 failed，12 ignored |
| Native / Wasm 公开 API 与架构边界 | 通过，Runtime methods 28/28 |
| `cargo xtask docs` | 391 个 Markdown 与 xtask 引用通过 |
| 三本书 HTML / PDF | 均构建成功 |
| `render.antialias` | 40 个 GPU 矩阵及 Web 后端编译通过 |
| `text.layout-direction` / `text.underlines` | 契约及独立 GPU Partial/Full 回归通过 |
| `render.low-miter` | 24 个 Graphics、4 个样例契约及 16 个 GPU 组合通过 |

12 项忽略对应人工、性能或 GPU 显式执行门禁；本批方向、下划线和 AA 的 GPU
用例已单独执行，release 成本用例也已执行，没有把忽略计数作为通过证据。
方向断言曾因新增 run bounds 而失效，已改为验证逻辑身份、基线、尺寸不变，
以及 bounds 与字形采用相同平移；修正后的工作区和方向 suite 均通过。

代码按样例修正、文字能力和描边能力分别提交：
`876662fc`、`42fe12b1`、`ed5374ba`。门禁与书稿另作独立提交。
并行的长文本方案、字体资料、宿主输入与目录复核改动保留在工作区。
门禁完成后出现的并行 Flow 统计插桩不属于本批提交或验收范围；
上述通过记录对应本批验证时的版本，不外推到之后的并行改动。
ByteSec 曾阻止提交后向沙箱外写记录，实际 Git 提交均已核对成功。

本批新增 ADR、方案及实施记录按好好说话技能再次回读。
机械扫描的中文半角标点、破折号、空泛元话语和机械编号标题均为 0；
代码、数字、指标与命令保留原意。`git diff --check` 在提交前执行。

本批已完成。通用设备 hairline、上游原生栅格和未执行的平台呈现仍保留待验收，
不变更官方入口完成计数。执行队列只在
[P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
