# B1：脚本专用规划回调准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖；
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：本子片完成；B1 整片仍未完成，生产缓存保持默认关闭。

## 实现范围

本子片补齐 Arabic、Syriac、Hangul 的 ShapePlan 回调。Arabic 的 feature
登记、阶段 pause、`rclt` 字体查询及 mask 数组构造接收同一个泛型准入策略；
Hangul 的三项 jamo feature、`calt` 覆盖及 mask 数组构造也在操作前计费。
普通入口与准入入口共用这些计算内核。

`ot_map::has_feature_with` 把策略传入已有的布局表查询内核，
不通过普通查询绕过准入。每轮 feature 数组访问、固定脚本判断和 mask
查询分别有费用；字体表内部的语言／feature 搜索沿用既有查询费用。
原始 feature 次序、global 标志和 pause 函数保持。

规划数据使用 `HeapSlot<T>` 的单元素 Vec：增长先准入再
`try_reserve_exact`，随后准入搬入存储及最终释放。
原有 `Any` 数据查询通过安全的借用保持，不引入 unsafe 或 infallible
的额外 Box 分配。未覆盖的上游脚本仍使用原有普通路径。
私有 `ColdCallbacks` 和数据存储分支描述 Harfrust 已有的固定脚本算法表，
不新增 Core 能力身份，也不作为用户扩展注册协议。

未知脚本回调、非空 AAT 与 trak 的拒绝边界保持。此处支持的是规划，
不是 `shape_internal` 的替换、定位或规范化准入。
未进入实际塑形前，不能据此声明 B1 或 ADR-059 的分析模板已经可用。

## 验证输入与拒绝

真实字体采用 InterVariable、NotoSansArabic-VF 和 NotoSansSC-VF。
普通输出矩阵还包含既有冷构造夹具、四个方向、空／非空文字、
全局与范围 feature，以及 Arabic、Syriac、Hangul 的指定脚本。

在原 Arabic 字体上指定 Syriac 会选到默认规划，不能验证 Syriac 专用回调。
最终夹具将 Arabic 字体 GSUB／GPOS 的 `arab` 脚本记录改为 `syrc`，
重新按 tag 排序，保留 lookup 数据和相对偏移。
这是测试合成字体，不代表完整 Syriac 字体资源或生产文字效果验收。
逐点拒绝用例检查最终两次初始化访问，确认已进入专用规划数据存储。

新消费者逐一拒绝每个已执行准入点，核对原始 Error source、完整调用前缀、
没有成功候选、完整重试及后续普通塑形可用。普通与准入计划比较 glyph、
position、cluster／flags 等完整公开观察。
此处的普通塑形可用性不证明尚未实现的塑形拒绝后 buffer 恢复。

## 匹配成本

最终准入成本共 12,000 个 Native release 样本：
三个脚本 × 四种模式 × 每组 1,000 次，另各预热 100 次。
每个样本旋转模式次序；字体、ShaperData 和 Shaper 在计时外构造。
计时包含新计划构造；成功计划释放在计时外，拒绝后的候选清理在计时内。

| 脚本 | 普通 P95 µs | 无计数准入 P95 µs | 带计费准入 P95 µs | 最末拒绝 P95 µs | 成功 work／allocation／调用 |
|---|---:|---:|---:|---:|---|
| Arabic | 6.417 | 8.750 | 9.958 | 10.042 | 6367／317／1834 |
| Syriac | 6.458 | 8.291 | 10.000 | 9.834 | 6311／317／1800 |
| Hangul | 3.750 | 4.917 | 5.917 | 6.292 | 4228／97／998 |

最后一次拒绝发生在规划数据搬入存储前，少收两个初始化访问；
已执行的容量预留仍计费，调用数与成功一致。
allocation 是新增整容量槽位，不是字节。
`before-syriac-fixture-*` 保留旧默认规划用例的中间数据，不参与最终结论。

另有原始／生产相同普通 API 的三轮匹配采样，共 18,000 个样本，
顺序为原始→生产、生产→原始、原始→生产。P95 范围如下：

| 脚本 | 原始普通 P95 µs | 生产普通 P95 µs |
|---|---:|---:|
| Arabic | 4.792–5.292 | 5.292–5.541 |
| Syriac | 4.917–5.125 | 5.000–5.375 |
| Hangul | 2.959–3.125 | 3.083–3.250 |

这些样本的后台负载没有控制。生产普通入口存在小幅成本增加，
不能把本子片描述为普通计划构造提速；也没有对单个存储分支做独立消融。
上述微秒数据不包含实际塑形、Core 分析、GPU 或宽度复用收益。
生产缓存保持默认关闭。

## 证据与后续

原始归档、十二份有序补丁、源码保持、输出和消费者日志统一保存在
[B 证据目录](../evidence/text-stages-b-2026-11-02/README.md)。
当前核对工具为 `tools/scripts/check_harfrust_script_callbacks_admission.py`，
先重放前十一份冻结补丁，再追加本片补丁，并逐字节核对全部生产源码。
此前工具保留各自的历史快照，不用于当前源码总核对。

十二份补丁重放通过，本片修改 12 个源码文件，其余 68 个文件保持；
最终全部 80 个文件与重放结果逐字节一致。
增量补丁 SHA-256：
`e482e976eba0aef2b1954906a277c5ac823eea00221dfa210da8c620cd6dbf22`。

| 输出矩阵 | 每配置观察数 | 原始／生产 Native／portable 对照 |
|---|---:|---|
| 既有普通计划 | 39,744 | 四配置一致 |
| 布局表计划 | 50,688 | 四配置一致 |
| 私有语言 | 6,720 | 四配置一致 |
| 通用语言 | 24,048 | 四配置一致 |
| 新增脚本回调 | 51,840 | 四配置一致 |

新增矩阵 SHA-256：
`d93e24d32419d33c441dc4a2172de25c97a59e74c31cebd43e01a7ef449eafdb`。
既有矩阵的四组签名保持不变，完整签名保存在
`script-callbacks-verification.json`。
Native／portable 各 200 项私有测试、38 项消费者测试通过；
严格 Clippy、Core Native／Wasm check 和 portable Wasm 消费者构建通过。
Wasm 仅构建，未执行测试。

下一步继续 B1 的实际塑形操作准入和 buffer 恢复；
B2–B4、C／D 与默认启用条件仍未完成。
成稿经过技术结构与中文表达自审，未取得独立文档评审结论。
