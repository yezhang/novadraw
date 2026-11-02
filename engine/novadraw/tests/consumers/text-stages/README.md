# Text stages consumer

独立 crate 通过 `text::shaping` 验证 ADR-059 A 的阶段协议。
测试夹具只使用公开 API，自定义 engine、typed key 和不可变 payload。
简单几何用于验证缓存与失败行为，不用于证明 Parley 的文字等价或性能。

```bash
cargo test --manifest-path tests/consumers/text-stages/Cargo.toml --lib --locked
cargo test --manifest-path tests/consumers/text-stages/Cargo.toml --lib --locked --no-default-features
cargo clippy --manifest-path tests/consumers/text-stages/Cargo.toml --lib --locked -- -D warnings
cargo build --manifest-path tests/consumers/text-stages/Cargo.toml --lib --locked --target wasm32-unknown-unknown
cargo build --manifest-path tests/consumers/text-stages/Cargo.toml --lib --locked --target wasm32-unknown-unknown --no-default-features
```

19 项测试覆盖阶段绑定、复合失败、精确命中、字节与条目额度、
计划缺失、操作前预算拒绝、共享 backing 和旧 lease。
source、调用计数与预算粘滞失败在同一 fixture 中核对。
事务中途预留失败的私有注入另用 `cargo test -p novadraw --lib text::staged` 验证。

实施范围与平台证据见
[A 实施记录](../../../doc/verification/reviews/text-stages-a-implementation-2026-11-02.md)。
