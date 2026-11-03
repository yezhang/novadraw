# 精确文字缓存查找优化

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 的 `FlowLayoutService`。
状态：内部缓存优化、定向回归和 CPU 对照完成；整体长文本性能仍未闭合。
本片沿用 [ADR-051](../../adr/adr-051-exact-flow-layout-reuse.md)，不改变公开 API、
贡献 callback、字体布局、完整几何发布或默认原生字形输出。

## 根因与最终修改

原 `FlowWork::layout` 每次先计算完整 `StyledTextRequest::fingerprint`，
遍历文本、样式和 inline box，再扫描最多 256 个段落资产。
原 Large 的请求重复率较高，完整指纹计算仍发生在命中之前。
仅给指纹增加索引或增加最近一次请求复用的初步采样没有稳定收益，
因此最终实现改为直接读取文本长度来缩小候选范围。

长度仅用于候选分组。命中继续比较引擎身份、引擎 revision 和完整 request；
相同字节数的不同文本、宽度、样式或 inline box 不会因此共用错误结果。
未声明 `cache_identity` 的外部引擎继续完整计算。
命中后的诊断、工作计数和共享 `StyledTextLayout` 返回路径保持原合同。

段落资产使用固定槽位按原 FIFO 顺序淘汰，避免 `Vec::remove(0)` 移动全部资产。
长度索引同步移除退休槽位，空分组立即删除；旧快照仍持有自身不可变布局。
缓存上限继续为 256，最多 256 个活动候选和 256 个长度分组，
未增加字体、依赖、GPU 资源或默认 feature。
索引的 Vec 容量可能高于当前活动候选数，本片未实测其堆留存字节数；
同长度工作集仍可能扫描全部候选，不能宣称最坏情况恒定时间。

## 正确性验证

| 检查 | 结果 |
|---|---|
| 私有缓存契约 | 3 项通过：相同长度下完整请求／引擎／revision 隔离；超过两轮容量的 FIFO 淘汰及旧布局留存；同长度碰撞分组中只移除退休候选 |
| 公开文字回归 | 55 项通过：布局复用、绘制包络、structured／styled flow、外部引擎、继承方向 |
| Native 与 Wasm 编译 | release 原始场景消费者构建、Wasm Core check 通过 |
| 严格 Clippy | Rust 1.96 的 Core lib 通过 |

代码变更只在 [flow/snapshot.rs](../../../crates/novadraw/src/figure/flow/snapshot.rs)。
原生 glyph 仍完整录制；本轮没有开启原生覆盖裁剪、改变 shaping 或启用 worker。
既有轮廓／未知包络的绘制回归保持通过，本轮未新增 GPU 或窗口验收。

可复跑命令：

```bash
cargo test -p novadraw --lib paragraph_index
cargo test -p novadraw --test long_text_layout_contract --test long_text_paint_contract --test p2_t03_structured_flow_contract --test p2_t03_styled_text_contract --test p2_t03_external_flow_engine --test p2_inherited_direction_contract
cargo +1.96.0 clippy -p novadraw --lib -- -D warnings
cargo check -p novadraw --lib --target wasm32-unknown-unknown
cargo build -p draw2d-official-examples --release --bin textflow-perf
```

Rust 1.99 的严格 lib Clippy 被既有 `svg.rs` 的 chunks lint 和 UpdateManager
两项 drain lint 阻断，未修改这些独立代码，也不宣称该工具链严格检查通过。
日志保存在本地 `target/verification/text-cache-2026-11-02/`，不纳入 Git：
`contracts.log` 为契约测试，`regressions.log` 为公开回归，
`clippy-1.96.log` 为 Rust 1.96 Clippy，`wasm-check.log` 为 Wasm 检查。

## 原 Large 的匹配对照

使用同一个 `textflow-perf` 原始工厂和 extended 驱动。
每个版本三个新进程会话，每会话前 8 个样本预热、随后 32 个正式样本；
两个版本按 before／after、after／before、before／after 交错执行。
每次样本构造独立 Runtime，依次首次稳定化、追加 Z、无变化查询、滚动、
宽度 resize 和 CPU IR 录制。两个版本均启用相同诊断。

共有每版 96 个正式样本，无失败；来源规模、布局计数、缓存命中计数和输出数量一致：
40 段、501 个来源、51,232 字节，初始 561 行／1,022 run／4,168 fragment；
追加后滚动没有新增 layout，resize 调用 6 次 layout，
录制 7,889 条命令和 51,233 个原生 glyph。
来源提交仍只推进一次 component revision。

下表是三个会话各自 P95 的中位数，单位 ms。
“查找累计”包含该操作内所有段落请求，不是单次 cache lookup 的延迟。
完整会话分布见本地收据 `target/verification/text-cache-2026-11-02/performance-summary.json`。

| 阶段 | 优化前 | 优化后 |
|---|---:|---:|
| 首次布局的查找累计 | 0.671336 | 0.140785 |
| 追加的查找累计 | 0.257464 | 0.061591 |
| 滚动的查找累计 | 0.109626 | 0.016920 |
| resize 的查找累计 | 0.672197 | 0.283428 |
| 首次稳定布局整体 | 37.506708 | 36.733791 |
| 追加与稳定化整体 | 15.913666 | 15.855624 |
| 滚动与稳定化整体 | 6.418875 | 6.398000 |
| resize 与稳定化整体 | 34.002291 | 33.730833 |
| resize 后 CPU IR 录制 | 3.947583 | 4.114875 |

四个查找阶段在三个会话中均下降，累计 P95 中位数降低约 58%～85%。
整体追加、滚动和 resize 的变化不足 1%，IR 录制观测值上升约 4.2%。
这些总阶段变化不能单独归因于缓存修改，也不证明整体目标已经达到。
本轮未控制其他进程负载，诊断开启影响绝对时间；
样本规模小于既有正式 LT 验收，不能替代其 1,000 次热样本／200 个新进程分母。
没有 GPU、surface present、input-to-present 或生产关闭诊断的数据。

原始六份 JSONL、脚本、构建日志、源码前后快照与补丁保存在
本地 `target/verification/text-cache-2026-11-02/performance-summary.json`。
基线 Core 文件来自 `1148c012`；性能消费者在变更前后从同一工作区冻结，
二进制散列在收据中。本片没有构建或比较产物体积。

## 状态与后续

本片关闭的是既有精确缓存查找的重复工作，LT-00～03 保持 in_progress。
原生可见覆盖、宽度分阶段复用、贡献纯度／版本合同及生产呈现证据仍待推进。
先以[行业参考](../../reference/text-layout-performance-2026-11-02.md)和实际阶段分母
选择下一片，不按文本长度自动切换渐进模式，也不从微小整体波动推断成功。

交付前按技术文档清晰度与好好说话自审：核对主体、条件、缓存身份、FIFO、
采样分母和证据限制。没有独立 reviewer，不声称独立评审通过。
