# Service failure consumer

独立 workspace，只从公开领域路径消费 Core；Inspector 通过可选依赖接入。
当前覆盖 ADR-057 的 S09-A Animation 契约，尚未覆盖 S09-B 的订阅恢复。

```bash
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features
cargo build --manifest-path tests/consumers/service-failure/Cargo.toml --release --lib --locked --offline --target wasm32-unknown-unknown
cargo run --manifest-path tests/consumers/service-failure/Cargo.toml --release --bin animation-failure-cost --locked --offline
```

`consume-animation`、`consume-inspector` 只选择测量消费者的调用，不是引擎 feature。
关闭默认 feature 后，仅 `empty_owner` 可达；分别启用两项可测动画和 Inspector 的可达体积。
`consume-inspector` 当前只做健康 attach／读取，实例和 Runtime 在同一次调用结束时销毁。
完整 token／detach 契约留给 S09-B。

成本探针使用同一源代码对比旧 bool 与新 Result，因此把返回值交给 `black_box`；
领域 Result 的断言由四项公开契约测试单独完成。
五个交替会话、每会话 20 次预热和 201 个样本，每个样本执行 10,000 次同值或模式切换。
Runtime 构造、样本容器和模式重置不计入时间及分配统计。
当前测量无活动轨道，不描述取消大量轨道、GPU 呈现或 Inspector 入队成本。

release 为 `opt-level=s`、LTO、单 codegen unit、`panic=abort` 和 strip；
panic 故障契约测试使用正常 test profile 的 unwind。
结果和依赖锁定范围见 [S09-A 实施记录](../../../doc/verification/reviews/p2-s09-a-implementation-2026-10-30.md)。
