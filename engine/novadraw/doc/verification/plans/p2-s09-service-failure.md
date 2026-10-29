# S09 服务失败验证矩阵

类型：`verification-plan`

状态：草案，待评审；2026-10-29。目标见
[ADR-057](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)。
本页定义未来退出条件，没有已通过结果。

| ID | 场景 | 必须观察 |
|---|---|---|
| SF-01 | Animation mode／suspension 同值、异值与已 faulted 同值 | 未变化、变化、fault 分离；fault 检查先于同值短路 |
| SF-02 | behavior processing 普通无变化、拒绝、panic | 无变化成功；拒绝保留该次状态；panic 遵循 owner fault |
| SF-03 | Inspector 事件读取与清空的锁 poison | 结构化操作错误，不能报告为空或成功清空 |
| SF-04 | observer 入队遇 poison | 首个 fault 带 epoch／sequence，self-removal；其他 observer 完成当前 effect |
| SF-05 | 普通外部 observer panic | Runtime fault 保持；不能降级为 Inspector 本地错误 |
| SF-06 | Runtime／Figure scope 的 attach、detach、dispose | 显式 owner；重复注销幂等；Figure dispose 自动注销 |
| SF-07 | foreign token、Runtime fault 后 detach、Inspector fault 后 detach | foreign 结构化错误；清理不调用 callback、不稳定化 |
| SF-08 | buffer 满、采集 fault、重建后 attach | 淘汰最旧；fault 后有缺口；不补造未采集的历史 |
| SF-09 | Anchor／Router register 输入拒绝、fault、有效重试 | 一个规范 fallible 入口；失败不留下登记或伪 ID |
| SF-10 | 自定义 Command／Policy Error 的多层包装 | 原始 source 可追溯／downcast；分类与显示文字独立 |
| SF-11 | 文本／非文本 panic payload | 文本保持；其他只保存分类；不调用用户格式化器 |
| SF-12 | 失败前已提交前缀与未提交候选 | 沿用既有原子性层级，不承诺任意扩展副作用回滚 |
| SF-13 | 独立消费者与 Native／Wasm API | 内置与外部共用协议，旧 panic alias compile-fail |
| SF-14 | 同值动画与事件入队成本 | P50／P95、分配、复制、有界存储及容量满对照 |
| SF-15 | 同配置 release 与可选能力关闭 | Native／Wasm 原始／压缩字节、依赖与首次资源 |

Execute 前先审阅现状，逐项列出受影响入口与错误类型。
不靠历史审计行号声称完成盘点，也不为 poison／revision 耗尽增加公共测试 hook。
私有故障注入、公开生命周期、外部消费者与 compile-fail 分开统计。

内环使用受影响 crate 的 lib 或单 integration target；最终运行 workspace
fmt、严格 Clippy、test、API／依赖边界及 docs。现有 check／has 契约做回归，
不把已完成能力重新定义为新增交付。
完成记录按实际覆盖关闭 R07、TC-10、API-F08、API-F10，
仅回填 API-F09 残余 register alias。
