# S09-A Animation 服务结果实施记录

类型：`implementation-verification`

日期：2026-10-30

能力归属：Core（`novadraw`）Animation 与直接帧调用方。
示例只迁移调用方式，Inspector／Observations、Connections 与 Editor 归后续切片。

用户已接受 [ADR-057 修订 3](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-057-service-failure-and-inspector-lifecycle.md.txt)并授权实施。
S09-A 的实现、消费者、成本、公共 API 与 workspace 最终门禁已完成。
S09 整批保持 `in_progress`，本记录不关闭其他切片的来源审计项。
Core 与调用方实现提交为 `fd9d8ef8`；消费者、接受状态及证据单独提交。

## 入口与行为

本次先按[当前源码盘点](p2-s09-design-inventory-2026-10-30.md)核对实际入口，
再修改以下调用链：

| 入口 | 实施结果 |
|---|---|
| `AnimationMut::set_mode` | 返回 `Result<bool, AnimationError>`；先检查健康，再判断同值；健康同值为 `Ok(false)`，配置变化为 `Ok(true)`，fault 为 `RuntimeFaulted` |
| 私有 `AnimationMut::set_surface_suspended` | 同样先检查健康；bool 表达 suspension 配置变化，damage 单独比较；无轨道的配置变化不产生 presentation damage |
| 私有 `AnimationMut::process_behaviors` | 返回领域 Result，健康检查先于空 effects；普通工厂／计划拒绝仍记录到既有有界队列，保留已成功计划和已消费事实 |
| Runtime behavior 委托 | 在取走待消费事实前检查 fault；映射 `FramePreparationError::Animation`，不消费被拒绝调用的事实 |
| 稳定查询、动画事务、submission preparation | 显式传播 Animation 错误；公开 Runtime fault 仍按已有外层入口返回 `FramePreparationError::Faulted` |
| `FramePreparationError::source` | Animation 分支保留领域 Error，可 downcast |

工厂 panic 仍由既有 guarded 边界清理 presentation、标记 Runtime fault 并继续传播。
源模型已提交的 bounds 不回滚；普通拒绝不升级为服务 fault。
测试专用 frame helper 保留原来的 Option／panic 接口，并显式处理新增 Result。

已迁移四组 Core Animation 契约、animation-app 和官方 Tree 的 `set_mode` 调用。
Logic 应用的工具模式不属于 Animation，保持其独立契约。
新增 compile-fail doctest 要求旧的直接 bool 消费方式编译失败。

## 验证对应

矩阵为 [S09 服务失败验证矩阵](../plans/p2-s09-service-failure.md)。
跨切片矩阵项这里只记录 Animation 部分。

| 矩阵项 | 实际证据 |
|---|---|
| SF-01／16 | 5 项私有测试：fault 同值、空 effects、无轨道 suspension、Pause／Finish 的状态与 damage、内部帧错误映射 |
| SF-02／12 | 普通拒绝返回 `Ok(false)` 且保留诊断；混合成功／拒绝保留有效计划、已提交 bounds，第二次稳定化不重放；panic 清理并保留模型提交 |
| SF-13 | 独立 workspace 两种配置各 4 项公开契约；Native／Wasm release 消费者构建；旧 bool 用法 compile-fail |
| SF-14／15 | 下述无轨道模式成本及三类消费者 Native／Wasm 产物比较 |

[私有测试](../evidence/p2-s09-a-2026-10-30/private-tests.log) 5 项通过。
[六组 Animation 回归](../evidence/p2-s09-a-2026-10-30/animation-regression.log)
共 100 项通过：behavior 7、timeline 29、geometry 41、reservation 4、transition 15、新故障 4。
独立消费者的[默认配置](../evidence/p2-s09-a-2026-10-30/consumer-tests.log)和
[关闭默认配置](../evidence/p2-s09-a-2026-10-30/consumer-no-default.log)各 4 项通过。

## 最终门禁与工具链

[workspace check](../evidence/p2-s09-a-2026-10-30/workspace-check.log)、
[独立消费者严格 Clippy](../evidence/p2-s09-a-2026-10-30/consumer-clippy.log)和
[Native／Wasm API 检查](../evidence/p2-s09-a-2026-10-30/api.log)均通过。
API 检查包含公开路径、禁止的第三方类型、依赖方向、渲染热路径及 Runtime 28／28 方法快照。
[animation-app 公开验证](../evidence/p2-s09-a-2026-10-30/animation-app.json)
14 个场景全部通过，包括 ReducedMotion 清理、Behavior 中断和 suspension 策略。
该命令为 `cargo +1.96.0 run -p animation-app -- --verify`，不代表窗口人工验收。
[最终 workspace 测试](../evidence/p2-s09-a-2026-10-30/workspace-tests.log)
在 Rust 1.96.0 下完成 177 组汇总：1,749 通过、0 失败、24 忽略。
包括新增 set_mode compile-fail 和最大树深度 dispose；忽略项继续保留其原验收边界。
[格式检查](../evidence/p2-s09-a-2026-10-30/fmt.log)和
[文档门禁](../evidence/p2-s09-a-2026-10-30/docs.log)通过。

Native 最终严格 Clippy 沿用上一批验收的 Rust 1.96.0，
[日志](../evidence/p2-s09-a-2026-10-30/workspace-clippy-1.96.log)通过。
Rust 1.99.1 的[首次严格检查](../evidence/p2-s09-a-2026-10-30/workspace-clippy.log)
因三个既有 lint 失败：SVG 的固定 chunk 和两个队列的 drain／collect。
未把队列容量保留策略的改写纳入本片，也未宣称 1.99 的 workspace 严格门禁通过。
Wasm API、release 产物与成本比较使用 Rust 1.99.1。

## 模式调用成本

基线为 Git `9825957efb1859ffca504e03eafd79ff19a226bd` 的隔离 worktree。
两边使用同一个消费者源码、依赖版本、Rust 1.99.1 和 release 配置；
完整命令、产物指纹及原始大小见[产物收据](../evidence/p2-s09-a-2026-10-30/sizes.json)。
Core 当前默认 feature 为空，没有单独的 animation feature。

构建结束后交替执行五个会话。每会话预热 20 批，采样 201 批，
每批调用 10,000 次；Runtime 构造、模式重置与样本容器不计入测量。
下表为各会话批次分位数的中位数，单位 ns／批，不是单次调用分位数：

| 场景 | 基线 P50 | 当前 P50 | 基线 P95 | 当前 P95 | 测量期间分配 |
|---|---:|---:|---:|---:|---:|
| 无轨道同值 | 35,208 | 35,375 | 36,625 | 38,083 | 两边均为 0 |
| 无轨道模式切换 | 35,125 | 38,000 | 36,708 | 39,125 | 两边均为 0 |

同值 P50 增加约 0.5%，模式切换 P50 增加约 8.2%。
首会话耗时较高，五个会话原始日志保留；该微基准只描述无轨道配置调用。
本场景不生成 record，因此没有通知复制；未测量活动轨道批量取消、
Inspector 入队／锁竞争、GPU 帧时间或产品首次加载。

## 可达产物

Native 测量 stripped cdylib，Web 测量 Wasm；gzip 为固定 `mtime=0` 的压缩结果。
两边都使用 `opt-level=s`、LTO、单 codegen unit、abort panic 和 strip。
三个变体分别仅构造 Runtime、消费模式 API、健康 attach／读取 Inspector。
这里的 Inspector 变体建立后续切片的基线，不验证新的生命周期行为。

| 消费者／平台 | 基线原始 bytes | 当前原始 bytes | 基线 gzip bytes | 当前 gzip bytes |
|---|---:|---:|---:|---:|
| Runtime／Native | 1,447,776 | 1,447,776 | 632,416 | 632,256 |
| Runtime／Wasm | 1,292,831 | 1,292,937 | 431,309 | 431,380 |
| Animation／Native | 1,464,496 | 1,464,496 | 643,393 | 643,424 |
| Animation／Wasm | 1,323,337 | 1,323,501 | 440,984 | 441,037 |
| Inspector／Native | 1,447,920 | 1,447,920 | 636,353 | 636,416 |
| Inspector／Wasm | 1,303,699 | 1,303,799 | 434,580 | 434,547 |

Animation 可达 Wasm 原始增加 164 bytes，gzip 增加 53 bytes；Native 原始体积不变。
不把文件对齐、链接布局及压缩差异当作逐条指令的体积归因。
没有新增第三方依赖、默认 feature、字体或图片。
消费者没有 JavaScript 胶水和首次加载资源，不能由此推断完整产品下载成本。

## 交接

下一片为 S09-B Observations 基础协议与 Inspector 生命周期：
统一 listener 准入／耗尽／fault 清理、持久发布游标、无 GPU 通知结清、
poison 首 fault、subscription token 和宿主恢复边界须共同验收。
Connections 和 Editor 仍按后续切片推进。

本记录交付前分别核对技术结构、事实与中文表达，保留矩阵范围及测量限制。
