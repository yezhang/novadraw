# Service failure consumer

独立 workspace，只从公开领域路径消费 Core；Inspector 和 Editor 通过可选依赖接入。
覆盖 ADR-057 的 S09-A Animation、S09-B Observations／Inspector 订阅恢复和
S09-C Connections 注册及 S09-D Editor 扩展诊断。

## Editor 扩展诊断

```bash
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --no-default-features --features consume-editor
cargo check --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --no-default-features --features consume-editor --target wasm32-unknown-unknown
cargo run --manifest-path tests/consumers/service-failure/Cargo.toml --locked --release --no-default-features --features consume-editor --bin editor-failure-cost
```

外部 Command／Policy／Tool 走引擎的公开注册和执行协议，测试多层 source
downcast、无 Clone／Send 要求的错误、失败操作、panic payload 与历史／fault 边界。
测试使用 unwind profile；Wasm 编译不等于运行了 panic 场景。
`consume-editor` 选择消费者调用，不是引擎 feature。

成本探针预热 10,000 次，测量 201 个样本，每样本执行 10,000 次操作。
输出的 P50／P95 是每个样本的总纳秒数，分配数是全部样本的总数。
测量覆盖文本／owned 错误构造、Clone、source downcast 和历史查询，
不描述任意用户错误构造或恢复成本。实际状态见
[S09-D 实施记录](../../../doc/verification/reviews/p2-s09-d-implementation-2026-11-02.md)。

## Connections 注册

```bash
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features --features consume-connections
cargo check --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features --features consume-connections --target wasm32-unknown-unknown
```

`consume-connections` 只选择消费者的调用。内置和自定义 Anchor／Router 使用同一
fallible 注册入口。测试检查注册不调用策略、连接准备才调用策略、真实 observer
panic 后拒绝注册，以及新 Runtime 的 namespace 隔离。faulted Runtime 上的连接
注销仍返回 `Faulted`；不同 owner 的 Anchor／Router 沿用 `UnknownAnchor`／
`UnknownRouter`，不改变既有错误分类。

Wasm 编译检查覆盖健康注册调用，panic 测试在 Native unwind profile 运行。
旧直接返回 ID 的调用和已删除的 `try_register_*` 由 Core 的四项 compile-fail
doctest 检查。详细结果见 [S09-C 实施记录](../../../doc/verification/reviews/p2-s09-c-implementation-2026-11-02.md)。

## 订阅恢复样例

在 Novadraw 根目录（`engine/novadraw`）启动：

```bash
cargo run --manifest-path tests/consumers/service-failure/Cargo.toml --no-default-features --features consume-inspector --bin inspector-lifecycle
```

这是不依赖窗口或 GPU 的终端样例。每次输入一个字母并回车，按顺序验证：

1. 输入 `m`，再输入 `e`：Figure 的 x 变成 10，事件包含本次移动，checkpoint 的 sequence 前进。
2. 连续输入两次 `d`：依次显示 `detach=true`、`detach=false`。
3. 输入 `m`，再输入 `e`：x 变成 20，旧 Inspector 的事件不增加。
4. 输入 `r`：显示 `gap accepted`，新快照的 x 为 20，`new events=0`；
   新基线承认断采缺口，不补录上一步的历史。
5. 输入 `m`，再输入 `e`：x 变成 30，新事件的 sequence 大于恢复基线。
6. 输入 `c`，再输入 `e`：缓冲为空；清空不使 Runtime 的发布游标倒退。输入 `q` 退出。

`r` 的恢复编排由消费者的 `src/inspection.rs` 提供：
清理旧 token → CPU 稳定化并分发通知 → checkpoint → 新实例及订阅 →
快照 → 再次 checkpoint。两次完整 checkpoint 相同才交付新基线；
失败时清理已建 token，并报告清理错误。宿主在此期间独占 Runtime 编排，
恢复成功后才继续输入和 backend 回执。
多个 Runtime 分别建立基线，不能按 sequence 建立全局顺序。

样例不提供 poison 按钮。poison、真实并发竞争和身份耗尽在私有测试中注入，
公开消费者验证 dispose、foreign 重试、在途帧、延迟回执和恢复失败清理。
release 使用 `panic=abort`，样例中的 `catch_unwind` 仅能在 unwind profile
捕获普通回调 panic；它不能恢复已经 faulted 的 Runtime，也不能拦截 abort。

```bash
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features --features consume-inspector
cargo test -p novadraw-inspector
cargo test -p novadraw-inspector --release --lib inspector_service_cost -- --ignored --nocapture
```

## Animation 与体积消费者

```bash
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features
cargo build --manifest-path tests/consumers/service-failure/Cargo.toml --release --lib --locked --offline --target wasm32-unknown-unknown
cargo run --manifest-path tests/consumers/service-failure/Cargo.toml --release --bin animation-failure-cost --locked --offline
```

`consume-animation`、`consume-inspector` 和 `consume-connections` 只选择消费者的调用，
不是引擎 feature。下述体积比较是既有 A／B 采证方法，只在用户明确要求时运行。
关闭默认 feature 后，仅 `empty_owner` 可达；分别启用两项可测动画和 Inspector 的可达体积。
`consume-inspector` 的可达入口执行健康 attach、CPU 稳定化、读取和显式 detach；
实例和 Runtime 在同一次调用结束时销毁。S09-B 的同源码体积比较另用固定
frame 消费者，避免把新增 CPU 分发工作量混进 API 改动的体积归因。

成本探针使用同一源代码对比旧 bool 与新 Result，因此把返回值交给 `black_box`；
领域 Result 的断言由四项公开契约测试单独完成。
五个交替会话、每会话 20 次预热和 201 个样本，每个样本执行 10,000 次同值或模式切换。
Runtime 构造、样本容器和模式重置不计入时间及分配统计。
当前测量无活动轨道，不描述取消大量轨道、GPU 呈现或 Inspector 入队成本。

release 为 `opt-level=s`、LTO、单 codegen unit、`panic=abort` 和 strip；
panic 故障契约测试使用正常 test profile 的 unwind。
结果和依赖锁定范围见 [S09-A 实施记录](../../../doc/verification/reviews/p2-s09-a-implementation-2026-10-30.md)与
[S09-B 实施记录](../../../doc/verification/reviews/p2-s09-b-implementation-2026-10-30.md)。
