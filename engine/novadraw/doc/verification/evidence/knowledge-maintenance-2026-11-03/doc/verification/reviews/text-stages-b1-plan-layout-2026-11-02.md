# ADR-059 B1：GSUB／GPOS 计划查询与 lookup 收集准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的
Harfrust 0.3.2 生产依赖。`api_semantics`：`text.flow`、`text.interaction`。
状态：本子片已实现并通过定向核验；B1 和默认启用仍未关闭。

本片接续[配置准入](text-stages-b1-configuration-2026-11-02.md)，
按已批准的[分析准入提案](../../design/architecture/text-analysis-admission-proposal.md)
补齐 ShapePlan 的字体表查询和 lookup 集合。
普通计划与准入计划共用查询、feature 收集和 lookup 收集内核；
普通入口仍允许原有语言和脚本回调，准入入口只进入已有费用依据的路径。

## 先读表头，再按实际集合准入

新增私有 `ot/query_admission.rs`，沿用锁定的 read-fonts 0.35.0 reader。
这些 reader 只借用数组；查询中真正执行的搜索和记录访问另外计费。
没有修改 read-fonts 或其他传递依赖。

| 操作 | 执行前的费用依据 |
|---|---|
| script tag 构造 | 无 language 时只处理固定大小 tag，最多写入三个 inline 项，并覆盖清理访问 |
| script 选择 | Offset16 与 count；候选 tag 及 DFLT／dflt／latn 的重复搜索分别覆盖，每次搜索至多 n＋1 个记录访问 |
| language 选择 | 按原顺序重复读取 Script，包含最后的 dflt 查询；每次 binary search 单独预付 |
| required feature | LangSys 的必选索引、FeatureList、完整 Feature 读取；保留子表损坏时的原有结果 |
| language／global feature 查询 | 前者覆盖每个索引及记录访问；后者逐条准入，按原顺序返回首个匹配 |
| feature variation 替换 | nullable offset、variation／substitution 表头、记录及 binary search；替换缺失时才读取原 Feature |
| lookup 展开 | LookupList count、Feature、每个 lookup 索引；有效索引写入前 fallible 预留，重复和无效索引保持原有行为 |
| lookup 去重 | 排序已有准入；每轮读取、合并和截断前补齐费用；stage 与最终 feature 存储继续既有准入 |

FeatureList 的 tagged element 与 FeatureRecord reader 会各读一次 tag。
两次读取均计费；不能用一次 tag 的费用覆盖两个调用。
必选 feature 仍读取完整 Feature，不改成只看 tag，
以免损坏的子表在新内核中意外变成有效 feature。
变体替换也保留原先的缺失、损坏和 fallback 顺序。

work 继续表示输入字节或元素访问，包含重复扫描；
allocation 继续表示完整新增容量槽位。
临时记录的写入、潜在迁移和释放分别由现有预留及初始化费用覆盖。
没有每字符倍数、字体白名单或基于 `max_ops` 的整体上界声明。

## 拒绝不发布半成品计划

准入错误立即传播原始 source，丢弃本次临时计划；
不通过普通入口补做，已有 ShaperData 和 ShaperInstance 仍可用。
查询和展开中没有向共享缓存安装半成品。
language、非空 AAT、脚本专用计划回调与 trak 仍在未知操作前返回 `Ok(None)`。
本片支持 GSUB／GPOS 计划构造，不据此允许实际执行 shaping。

定向消费者覆盖默认 LangSys、显式 dflt LangSys、必选 feature、
重复和越界 feature／lookup 索引、变体替换、每个表截断位置及真实字体。
普通／准入计划比较四方向的 glyph、位置、cluster 和计划键。
八组完整结构字体逐点拒绝，检查 source、严格 trace 前缀、
相同重试 trace 和后续普通调用可用。

## 验证与采样

重放 receipt（`target/verification/text-stages-b-2026-11-02/plan-layout-verification.json`）
从原始归档重放八份冻结补丁，再追加本片补丁。
三个既有 src 文件改动，一个私有模块新增，其他 75 个 src 文件逐字节保持；
验证结束后再次核对生产源码与重放结果。

| 验证范围 | 结果 |
|---|---|
| 原始／生产 Native／portable | 既有矩阵各 39,744 份观察一致；新增布局表矩阵各 50,688 份观察一致，分别核对 SHA-256 |
| 外部 Native／portable | 各 34 项通过，包含新增完整输出和逐点拒绝测试 |
| 私有 Native／portable | 各 197 项通过 |
| Core Native／Wasm | `novadraw --lib` check 通过 |
| Wasm 消费者 | portable library 构建通过，未执行 Wasm 测试 |
| Clippy／格式 | Native library／plan-layout-cost 和 portable library 无 warning；新增模块、测试与采样定向格式通过 |

新增原始对照矩阵使用 33 份字体输入、默认／非默认坐标、两轮调用、
两种脚本、两种 language 选择、四组 feature、六种文本及四方向。
观察同时包含 glyph、position、cluster、序列化和计划键；
原始对照通过普通入口执行，保留尚不支持准入的语言及阿拉伯脚本行为。
准入消费者另行验证已支持计划的完整输出和失败路径。

计划成本 receipt（`target/verification/text-stages-b-2026-11-02/plan-layout-cost-summary.json`）
保存四组输入、四种模式各 1,000 次有效样本，共 16,000 次；
每组预热 100 次，模式逐样本轮换。
计时只包含新建计划；成功对象释放在计时之外，拒绝包含临时候选清理。
字体缓存、实例、配置、实际 shaping、Core 和 GPU 均不在计时范围内。

| 输入 | 普通 P95（µs） | 空回调 P95（µs） | 计数 P95（µs） | 最后点拒绝 P95（µs） |
|---|---:|---:|---:|---:|
| 结构 GSUB | 5.250 | 6.084 | 7.958 | 7.750 |
| 结构 GPOS | 4.125 | 5.209 | 5.959 | 6.208 |
| Inter 非默认坐标 | 5.541 | 7.084 | 8.625 | 9.041 |
| 中文默认坐标 | 4.875 | 5.417 | 6.625 | 6.875 |

成功计数模式分别记录 work 2,913／2,914／6,440／4,281，
allocation 122／122／201／105，回调 981／981／1,453／1,021 次。
最后点拒绝尚未完成最终 feature 复制，work 比成功少 5，
此前分配仍计费，候选清理仍有成本，拒绝不保证更快。

原始／生产普通路径成本 receipt（`target/verification/text-stages-b-2026-11-02/plan-layout-ordinary-cost-summary.json`）
另外使用同一份普通 API 源码，分别编译原始与生产 Native release。
三轮轮换执行先后顺序，每组各 1,000 次并预热 100 次，共 24,000 个有效样本。

| 输入 | 原始 P95 三轮范围（µs） | 生产 P95 三轮范围（µs） |
|---|---:|---:|
| 结构 GSUB | 2.458–3.958 | 2.583–2.958 |
| 结构 GPOS | 2.292–2.666 | 2.459–2.709 |
| Inter 非默认坐标 | 4.209–6.167 | 4.208–4.416 |
| 中文默认坐标 | 3.125–3.917 | 3.292–3.333 |

结构 GPOS 与中文组的生产中位数比对应原始轮次约增加 0.17–0.25 µs；
保留该差异，不声明普通路径零开销。
两套采样时间不同，未控制整机负载，不能用两张表互相折算加速倍数，
也不据此声明端到端排版或宽度复用性能通过。

成稿已分别核对事实、技术因果和中文表达，未取得独立评审结论。

## 后续范围

继续 B1 的脚本计划回调、语言、实际 shaping 与 buffer 恢复，
再推进 B2 Fontique／Parley 编排和 B3／B4 导出与 backing 计费。
Core 阶段缓存保持默认关闭。
