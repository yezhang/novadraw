# ADR-038 草案评审

类型：`verification`

日期：2026-10-15

状态：`reviewed`；结论为修订后复核，尚不建议进入 Execute。

## 结论

[ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md) 有两项 P1 需要先
冻结契约：逆变换的数值失败分类，以及测量成功值的校验与缓存边界。另有两项 P2
补充：Route 依赖指纹与身份校验的分工，以及现行 SSOT 的同步范围。

评审仅记录发现和修订要求，没有修改 ADR 决策或 S07 Rust 实现，也没有关闭来源
审计 ID。本轮没有运行新增行为用例。

## S07-DR01：求逆的 None 仍可能吞掉数值失败

优先级：P1。

位置：ADR-038 第 5 节（102–106）及错误组合（167）。

草案规定不可逆返回 Ok(None)，计算溢出返回 CoordinateQueryError。
现有 [Affine2D::inverse](../../../crates/novadraw/src/geometry/transform.rs#L148-L157)
先调用底层 inverse，只要任一结果系数非有限就返回 None；它没有区分数学奇异与
数值计算/表示失败。

例如有限对角矩阵 diag(1e-320, 1) 的数学逆存在，但其第一轴倒数超出 f64 表示范围。
这只是数值分类例子，不是已经通过 Runtime admission 的现场复现。
若迁移仅在任意 ID 入口增加 Result 并包裹现有 inverse，数值失败仍会变成 Ok(None)，
与草案的“溢出进入领域错误”冲突。

修订要求：

- 明确 None 表示数学奇异，还是也包含数值不可求逆；不能同时作两种解释；
- 若采用当前草案的溢出错误合同，明确区分输入非有限、奇异和逆结果不可表示，
  并把求逆所需的最小内部调整列入范围；
- 不以固定 epsilon 擅自把合法小尺度判为奇异；说明退化判定与数值失败的边界；
- 矩阵增加单独求逆负测，覆盖奇异、有限可逆、逆结果溢出与行列式计算的
  underflow/overflow 分类。现有 S07-G03 的正向组合溢出不能替代这些用例。

## S07-DR02：测量 Result 的成功值尚未定义完整不变量

优先级：P1。

位置：ADR-038 第 6 节（117–144）与签名表（156–159）。

草案要求“测量及 projection 成功后填缓存”，却未明确如何判定 callback 的
Ok(measurement) 或 Ok(size) 是否成功。现有
[FigureMeasurement](../../../crates/novadraw/src/figure/mod.rs#L421-L435) 是可直接构造的值，
当前 [tree 测量](../../../crates/novadraw/src/graph/layout_measurement.rs#L605-L698)
在 layout callback 与 projection 后填缓存，没有与
[FigurePresentation::new](../../../crates/novadraw/src/figure/preparation.rs#L36-L80)
相同的完整指标准入。

因此，只改为 Result 并在 callback 后加 `?`，仍可能把 Ok(NaN)、负尺寸、
越界 baseline 或 Border/insets/container projection 的溢出当作成功缓存。
直接 preferred/minimum 查询无需产生 LayoutOutput，无法靠后续 placement 校验兜底。

修订要求：

- 明确 preferred/minimum 的最终尺寸必须有限且非负；baseline 如存在，必须有限并
  位于约定区间，复用既有 FigurePresentation 指标合同；
- maximum 的合法无上限单独定义，不能用同一 finite 校验误拒绝正无穷；
- 明确 callback 输出及最终 projection 的校验所有者、顺序和错误归属；
  校验通过后才返回成功或填缓存；
- 覆盖 override、LayoutManager、intrinsic、prepared 与 Border/container projection
  各测量分支，不只验证带 manager 的一条路径；
- 增加外部 callback 返回非法 Ok 值、projection 溢出、错误不入缓存、修正后重试成功
  的负向与恢复用例。

这是派生输出的校验，属于 S07 的失败传播边界；不要求提前完成 S08 detached
构造重构，也不要求为所有 f64 创建新 wrapper。

## S07-DR03：明确依赖指纹允许观察主体缺失

优先级：P2。

位置：ADR-038 第 7 节（178–182）；矩阵 S07-P03/P04。

草案已允许保留恢复所需的失败观测，但没有明确 `dependency_generation` 是否属于
必须拒绝无效身份的“任意 Figure 主体读取”。现有
[invalidate_stale_dependencies](../../../crates/novadraw/src/connection/runtime.rs#L867-L890)
通过该指纹检查既存依赖；其用途包含检测旧主体已经消失。
[TrackedRouteSceneQuery::observe](../../../crates/novadraw/src/connection/query.rs#L269-L285)
也在实际读取前获取指纹。

修订建议：明确依赖存在性/代次指纹与对象事实查询的区别。指纹可表达主体缺失，
不代表 bounds/visibility 读取成功；事实查询仍先验证身份。不要机械把缺失指纹
升级为稳定化失败，也不要因事实读取失败清掉恢复所需的全部依赖。

增加删除已记录依赖、失效后稳定 unresolved、更新为有效新绑定后恢复的用例，
并由外部 RouteSceneRead 证明相同规则。不得假定旧 stale ID 可以重新变为有效。

## S07-DR04：现行 SSOT 同步清单不完整

优先级：P2。

位置：ADR-038 第 5/6 节的 ADR-018 同步说明及实施章节。

当前 [public-api-contract.md](../../design/architecture/public-api-contract.md#L121-L131)
仍规定坐标 Option 保留 ADR-018 合同；parity ledger 也记录旧 Option 签名。
草案只具名同步 ADR-018，未把当前公共 API SSOT、坐标规范和阶段错误说明列为
实施必检入口。

修订建议：冻结同步清单，包含 ADR-018、public API contract、坐标规范、tree/query
规范、受影响 parity 行、API snapshot、外部 consumer 和恢复说明。
parity 合法行为基线不因签名迁移直接降级，但负测闭合前不得宣称 S07 已验证。

## 复核条件

S07-DR01/02 写入 ADR 与逐项验证矩阵后复核；DR03/04 补入明确的协议与迁移清单。
重点检查：

1. 错误类别由可观察事实决定，None、合法零值与无上限不被误改；
2. owner 在缓存或候选发布前校验派生结果，Result 不等于成功值已受检；
3. 普通 LayoutError 经 Validation/FramePreparation 返回，失败不晋升新 stable epoch；
   panic 继续走原 fault boundary，不混同为普通查询错误；
4. 内置与外部实现采用相同校验和错误路径，不增加特权入口。

草案中保留 unknown/stale 的合并缺失分类、复用 tree identity、限制 LayoutOutput
保证为本轮未发布候选等选择，本轮未发现需要撤销的证据。

## 文档检查

`cargo xtask docs` 与 `git diff --check` 均通过；文档门禁日志见
[adr-038-draft-review-docs.log](../evidence/adr-038-draft-review-docs.log)。
评审稿已按好好说话原则回读，中文相邻字符间半角标点、破折号及空泛元话语扫描为
0 命中。上述检查不替代 S07 新增行为用例。

## 修订跟进（2026-10-16）

ADR-038 与验证矩阵已完成修订 1，逐项处理 S07-DR01–DR04。求逆的 None 限定为
数学奇异；测量 owner 校验选中分支及最终投影后才返回成功或填缓存；Route 指纹
允许观察主体缺失；接受后的 SSOT 同步清单已冻结。对应关系见
[ADR 修订记录](../../adr/adr-038-query-identity-and-failure-semantics.md#修订记录)。

本段记录修订完成，不改变首轮评审结论。修订 1 尚待复核，新增行为用例尚未执行，
来源审计 ID 保持开放。

修订文档门禁通过，日志见
[adr-038-revision-1-docs.log](../evidence/adr-038-revision-1-docs.log)；
`git diff --check` 通过。ADR 与矩阵已按好好说话原则回读，机械扫描的中文半角标点、
破折号、空泛元话语及对比句式均为 0 命中。
