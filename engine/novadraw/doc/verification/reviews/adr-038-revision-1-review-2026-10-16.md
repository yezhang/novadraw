# ADR-038 修订 1 复核

类型：`verification`

日期：2026-10-16

状态：`reviewed`；设计复核通过，按用户授权进入实现。

## 结论

[ADR-038 修订 1](../../adr/adr-038-query-identity-and-failure-semantics.md)
已处理首轮评审的两项 P1 和两项 P2，未发现新的设计阻塞项。
用户已授权“复核 ADR-038 修订内容，确认无误后推进到实现阶段”，本次复核通过后
接受 ADR-038，P2-S07 转为 `in_progress`。

此结论只确认目标契约和实施范围。当前代码中的旧返回值及失败折叠仍需迁移，
新增行为用例尚未执行，R04、TC-09、API-F06、API-07 保持开放。

## 逐项复核

| 评审 ID | 复核结果 | 实施验收 |
|---|---|---|
| S07-DR01 | None 限定为有限输入的数学奇异；身份、非有限输入、结果不可表示和数值计算失败分开处理；禁止 epsilon 及直接行列式 0/无穷误判 | S07-G04–G07；可靠求逆须同时覆盖线性和平移结果，内部调整不涉及渲染主循环 |
| S07-DR02 | owner 校验选中测量分支及最终投影；preferred/minimum 有限非负，baseline 位于尺寸区间，maximum 正无穷为合法无上限；成功缓存只能保存校验通过的结果 | S07-L03、L09–L12；直接查询、非法 Ok、正常更新后重试分别提供证据 |
| S07-DR03 | 依赖指纹允许观察主体缺失，事实读取仍受检；失败观测与成功读取分开，恢复需要有效新绑定，旧 stale ID 不会复活 | S07-P03–P05；内置与外部 RouteSceneRead 共用协议 |
| S07-DR04 | ADR-018、公共 API、坐标、Tree/focus、阶段错误、parity、snapshot 和消费者同步范围明确 | 按矩阵“实施同步检查”记录实际更新或无需变更理由 |

普通 LayoutError 沿 Validation/FramePreparation 返回，不晋升新 stable epoch；
panic 保持原 fault boundary。本轮未发布 LayoutOutput 的丢弃保证不扩大为整轮回滚。
这些边界与 ADR-011、018、036、037 没有冲突。

## 执行顺序

1. 受检测量与 LayoutManager Result 通道，覆盖 owner 准入、缓存及未发布候选。
2. 任意 ID 结构、状态与坐标查询，完成身份错误和可靠求逆。
3. Event/Stable/Route、Editor、平台、示例与独立消费者调用点迁移。
4. 同步规范和 API snapshot，运行定向 suite、quick 与最终 full，再据实关闭来源审计。

先实施测量切片，以尽早验证 Result 成功值的准入和失败传播。所有切片仍属于同一个
S07 批次，不并行启动 S08/S09。
