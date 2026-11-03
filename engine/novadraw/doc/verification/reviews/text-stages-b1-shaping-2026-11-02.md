# B1：非扩张塑形准入与 buffer 恢复

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖；
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：本子片实现与定向门禁完成；
B1 整片仍未完成，生产缓存保持默认关闭。

## 实现与支持范围

新增 `Shaper::try_shape_with_plan_admitted`，借用 `&mut UnicodeBuffer`，
接收独立于 Novadraw 的泛型准入回调。
普通与准入入口共用 `shape_internal_with` 和原有算法阶段；
普通策略会在编译时消除准入回调。

本片支持已映射的 ASCII、LTR Latin／Common／unknown、默认坐标和全局
features；字体不能含 GDEF、GSUB、GPOS 或 AAT 布局表。
支持条件依据算法分支与输入数据，不依据字体名称或测试字体名单。
复杂脚本、范围 feature、未知布局表或缺失 nominal glyph 在执行 shaping
之前返回 `Ok(None)`，原输入保持。
方向或脚本与计划不一致返回 `PlanMismatch`，不通过 assertion panic。

该路径确实执行非空的归一化、glyph 映射、属性初始化、横向 advance、
position 初始化和 flags 传播。它不是只处理空文本的准入占位。
ASCII 无组合 mark、默认不可见字符或分解扩张；已映射的输入在 normalizer
中走 simple 分支，不会进入未知的分解、替换或容量增长。
这不代表常规富文本或带 GSUB／GPOS 的生产字体已经支持完整分析准入。

## 操作前费用与恢复

支持检查本身也有费用，包括表目录搜索、stage／feature 访问、
输入扫描和 nominal mapping。cmap 0／4／6／10／12／13 的借用映射 reader
按锁定的 read-fonts 0.35.0 源码给出访问上界。
二分搜索最多执行 `bit_length(n)` 轮；format 4 的最终 delta、range offset
与 glyph 读取另计，symbol fallback 至多再执行一次映射。
不修改 read-fonts 的生产源码。

pipeline 在阶段前逐笔准入已证明的遍历。normalizer 再次映射时再次收费，
不因为支持检查已暖缓存而省略费用。
`map_glyphs_fast` 的 info 与 output alias 两次访问分别收费；
digest、position 清空和横向 metrics 的整个已初始化数组访问也分别收费，
不能只按逻辑 glyph 数计费。三项 digest mask 和四次 Unicode 表索引有
独立的固定访问依据。新容量费用为零，因为本支持路径不会扩张。
原有 `max_ops` 与 buffer 长度保护保持。

支持检查拒绝时保留原输入。pipeline 拒绝时调用 `leave` 恢复内部上限，
释放已占用的 Unicode／normalizer／GSUB 变量位，清空输入并保留数组容量。
上游 `clear()` 假定 shaping 已完成，不重置这些变量位；
若中断后只调用它，下次普通 shaping 会因变量重复分配而 panic。
本片在中断恢复中显式重置位图，定向测试覆盖该行为。

成功后才将存储移交给 `GlyphBuffer`，调用方仍持有一个空可用输入 buffer。
恢复需要的固定状态和语言释放费用在进入 pipeline 前准入。
GlyphInfo／GlyphPosition 是无元素析构器的 POD，清空 Vec 保留容量。
没有失败后的普通重做、隐式 finalize、成功候选或 lease 发布。

## 输出与失败验证

消费者包含六种 cmap 的结构夹具，以及从 InterVariable／NotoSansSC-VF
移除布局表后保留字形映射和 metrics 的夹具。
这些改写字体只用于隔离路径，不作为真实字体效果验收。
非空测试明确核对非零 glyph 与 advance，比较完整公开 glyph info、position、
cluster／flags 和长度，并检查成功后的 buffer 所有权。

每个执行过的准入点逐一拒绝，核对完整调用前缀、Error source、
预算粘滞、未产生结果，以及后续普通 shaping 可用。
私有测试另查变量位、内部上限和两份数组容量。
未知输入保留原内容，随后能够执行普通完整塑形。

新增原始／生产普通 API 矩阵每配置 12,672：
11 份字体 × 2 次 × 2 个脚本 × 4 个方向 × 2 组 feature × 6 份文本
× 2 种备用容量 × 3 组 flags／cluster 编排。
其中还包含未改写的三份真实字体，覆盖本片没有新增准入支持的普通复杂路径。
既有五个矩阵继续对照。六个矩阵共每配置 185,712 条公开观察，
原始／生产 Native／portable 的四组签名逐矩阵一致。
新增矩阵 SHA-256 为
`b958578f818f91869cc287eb07b5a2e48f64dd2e11dd8b7b1d54325ec6916d42`；
既有五组签名保持不变，完整结果见 `shape-verification.json`。

## 匹配成本

Native release 共 16,000 个样本，另每组预热 100 次：
两份布局表移除夹具 × 两种备用容量 × 四种模式 × 每组 1,000 次。
每份输入为 960 个 ASCII 字符，模式逐样本轮换。
计时包含实际 shaping 与 buffer 移交，输入、font／plan 构造和成功结果释放
在计时外；失败恢复在计时内。

| 输入 | 普通 P95 µs | 无计数准入 P95 µs | 计费准入 P95 µs | 最末拒绝 P95 µs | 成功 work／allocation／调用 |
|---|---:|---:|---:|---:|---|
| Inter／默认容量 | 10.875 | 13.292 | 14.000 | 12.458 | 84750／0／1010 |
| Inter／备用 8192 | 21.500 | 23.708 | 26.459 | 24.250 | 142606／0／1010 |
| Noto SC／默认容量 | 10.875 | 13.209 | 13.917 | 12.375 | 84771／0／1010 |
| Noto SC／备用 8192 | 21.625 | 24.042 | 24.708 | 23.167 | 142627／0／1010 |

最末拒绝少收 960 次最终 flags 写入访问，调用数与成功一致。
默认／备用容量的明显差异来自原有算法访问整个初始化数组；
本片覆盖费用，没有修改这些算法的访问范围。

原始／生产同一个普通 API 的三轮采样另有 24,000 个样本，
次序为原始→生产、生产→原始、原始→生产：

| 输入 | 原始普通 P95 范围 µs | 生产普通 P95 范围 µs |
|---|---:|---:|
| Inter／默认容量 | 11.208–11.458 | 10.709–10.916 |
| Inter／备用容量 | 21.834–21.958 | 21.125–21.291 |
| Noto SC／默认容量 | 11.166–11.500 | 10.292–10.584 |
| Noto SC／备用容量 | 21.583–22.291 | 21.167–21.375 |

后台负载没有控制，未做独立消融。普通路径这组数据未显示退化，
但不能将差异归因为确定性提速。准入新增支持检查和费用回调有可测开销；
这些结果不包含 Core 分析、宽度复用、复杂脚本或 GPU。

## 证据与剩余工作

原始归档、十三份有序补丁、源码与矩阵记录统一放在
[B 证据目录](../evidence/text-stages-b-2026-11-02/README.md)。
当前核对工具是 `tools/scripts/check_harfrust_shape_admission.py`；
前十二份补丁保持冻结，追加 `harfrust-0.3.2-shape.patch`。
本片改动四份源码、新增一个模块，其余 76 份源码保持原始字节；
全部 81 份生产源码与十三份补丁重放结果逐字节一致。
增量补丁 SHA-256 为
`af7d3cd648701264f2de0099503caf2b4157eebad29e16a00506e1d413cbc974`。

Native／portable 各 202 项私有测试、43 项外部消费者测试通过。
Native／portable 严格 Clippy、成本程序 Clippy 与 portable Wasm 消费者构建通过。
Wasm 只构建，未执行测试。
Core Native／Wasm check 首次补验均遇到 `No space left on device`；
余量恢复后串行补验均通过。首次失败日志以 `before-disk-full-shape-core-*`
保留，最终日志为 `shape-core-*`。Python 语法、新增模块格式与文档门禁通过。

后续继续 B1 的组合字符、RTL、脚本 preprocessing、GSUB／GPOS 的实际
替换／定位及容量增长，然后接 B2 的 Parley 编排恢复。
B2–B4、C／D 和默认启用仍未完成。
成稿经过技术结构与中文表达自审，未取得独立文档评审结论。
