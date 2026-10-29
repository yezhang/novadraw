# S08-C：Detached 几何与外部指标准入

类型：`implementation-verification`

状态：已完成；2026-10-29。准入、几何入口、消费者迁移与回归门禁已通过。
整批渲染与成本关闭归 S08-D。决策见 [ADR-056](../../adr/adr-056-detached-input-invariants.md)，
用例归属见[验证矩阵](../plans/p2-s08-detached-input-invariants.md)。

## 已实现的边界

初始 bounds、insets、style 和 Border 基础指标在准入时捕获，
同一候选值用于校验和发布。BorderMetrics 保存受检的 Insets 和 preferred size；
Compound 递归检查当前 child 输出，组合溢出保留 outer／inner 错误来源。
owner 指标准备和替换的 effect 在发布前检查，拒绝保留旧状态。

矩形、椭圆、圆角矩形、三角形、Root、GraphProjection、分层和缩放容器、
Viewport、Label、Clickable、Button、Toggle、TextFlow 和 FlowPage 的原始 bounds
入口已改为 Result。Image、ScrollBar 和 SceneView 的 with_bounds 采用同一检查。
Bounded::set_bounds 返回 ShapeGeometryError；失败不写入配置。
矩形、椭圆和圆角矩形不再公开 bounds、线宽及圆角数值字段，描边复用 StrokeStyle。

Polyline／Polygon 在原始点列归一化时检查非有限坐标、跨度和描边外扩；
detached setter 检查缩放后的点，失败保持旧几何。Polygon 的闭合绘制点数阈值
进入内部组合配置，避免先按折线外扩后再收缩。ScalablePolygon 检查模板跨度，
映射先计算点在模板中的比例，再乘目标尺寸，避免小跨度产生无穷缩放系数。
已有零线宽、负坐标、零尺寸及 corner dimensions 的直径单位保持。

接受外部 Border 的 with_border 均检查基础指标。挂载和 owner 准备仍重新检查当前输出，
不会把构造时的成功当作后续成功。所有调用方通过显式 Result 或公开受检值迁移。

BorderedFigure 的公开替换入口改为受检 BorderCandidate，detached 安装与 Runtime
commit 共用候选，提交时不重复读取基础指标。Viewport origin 先检查两个坐标，
失败不修改共享范围；内部 ScrollPane 构造在发布前复用 bounds 校验。
带圆角三角形和显式逐角矩形的 resize 先准备轮廓，派生失败保持旧 bounds。

Viewer 根层和直接文本反馈通过 ViewerError::Geometry 传递构造失败，
保留 ShapeGeometryError 的 source；不为派生输入增加 expect。
官方样例、Canvas 消费和性能夹具的合法固定输入显式处理 Result。
新增 Viewer 非法 bounds 回归检查失败不创建 Part。

## 本轮验证

| 验证 | 结果 | 证据 |
|---|---|---|
| 新增 detached geometry 契约 | 7 项通过，含 26 个 bounds 构造入口和 18 类 Bounded 更新 | [定向日志](../evidence/p2-s08-c-detached-contracts-2026-10-29.log) |
| 几何、准入、Border、Graphics、独立圆角、ScalablePolygon | 合计 71 项通过，1 项成本探针按原配置忽略 | 同上 |
| 独立消费者默认 feature | 最终 38 项通过 | [默认消费者](../evidence/p2-s08-c-external-default-final-2026-10-29.log) |
| 独立消费者关闭默认 feature | 最终 38 项通过 | [无默认消费者](../evidence/p2-s08-c-external-off-final-2026-10-29.log) |
| Core lib 严格 Clippy | 通过 | [Core 检查](../evidence/p2-s08-c-core-clippy-2026-10-29.log) |
| 独立消费者 lib 严格 Clippy | 最终通过 | [消费者检查](../evidence/p2-s08-c-external-clippy-final-2026-10-29.log) |
| workspace check | 所有生产目标通过 | [结构化输出](../evidence/p2-s08-c-workspace-check-2026-10-29.jsonl) |
| 外部非法 Figure 的整批原子性 | 定向通过 | [批次回归](../evidence/p2-s08-c-external-batch-2026-10-29.log) |
| 几何入口续验 | 11 项通过，补 Image／ScrollBar／SceneView、Viewport origin、ScrollPane 和圆角 resize | [续验日志](../evidence/p2-s08-c-followup-contracts-final-2026-10-29.log) |
| 外部准入续验 | 12 项通过，补候选捕获、subtree、共享 owner 隔离与延迟失败恢复 | [owner 与 Deferred](../evidence/p2-s08-c-owner-deferred-final-2026-10-29.log) |
| Native／Wasm API 与依赖边界 | 最终通过，快照新增 Border 领域的 18 个公开符号，Runtime 28/28 | [API 检查](../evidence/p2-s08-api-verified-2026-10-29.log) |
| 最终 workspace 严格 Clippy | 通过 | [最终 Clippy](../evidence/p2-s08-clippy-verified-2026-10-29.log) |
| 最终 workspace 普通测试 | 159 个目标汇总，1,653 项通过、0 失败、20 忽略 | [全量日志](../evidence/p2-s08-workspace-tests-verified-2026-10-29.log) |
| workspace doctest 续验 | 14 个目标汇总，62 项通过、0 失败、1 忽略 | [doctest 日志](../evidence/p2-s08-workspace-doctests-verified-2026-10-29.log) |

Core 整库首轮为 427 项通过、1 项失败、1 项忽略。失败来自旧负测在构造内置矩形时
使用负宽度，错误现在按契约提前返回。该用例已改为外部 Figure 返回非法初始 bounds，
保持 Runtime 批次原子性的原验证意图，并单独复验通过。
最终普通测试已全部通过；Core README 的入门 doctest 遗漏 Result 处理，
补齐后全部 workspace doctest 通过。两份日志合计 1,715 项通过、0 失败、21 忽略，
去除失败轮次的重复计数，见[汇总收据](../evidence/p2-s08-test-summary-2026-10-29.json)。

早期全量任务曾受构建缓存被清理影响，出现 missing rmeta／object／executable；
这些日志保留为环境失败。随后使用隔离 target 完成迁移核对，继续找到
Viewer、官方样例和准备阶段测试夹具的旧调用，均已修正。
Native 使用 Rust 1.96.0，Wasm API 使用 stable Rust 1.99.1。

## 关闭范围与限制

公开替换协议、内部宿主构造及上述错误观测已补齐。Deferred capability 拒绝沿用
现有 ComponentUpdateRejected 身份通道；该通道不保留 generic Error 的具体类型，
直接 typed update 和 owner effect 的结构化 source 已验证。

五项新增 compile-fail 覆盖 bounds／stroke／corner 私有字段、旧直接返回和 raw Border
替换。现行协议文档、API snapshot 与独立消费者最终验证已完成。
最终普通测试与 doctest、格式、严格 Clippy、文档和架构均已通过。
Native GPU、默认／关闭默认 feature 的 Native／Wasm 成对产物和 Runtime
稳定帧／resize／style 成本见 [S08-D 记录](p2-s08-d-completion-2026-10-29.md)。
本切片完成不单独关闭整批审计项。
