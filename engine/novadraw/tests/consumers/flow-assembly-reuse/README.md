# Flow assembly reuse consumer

能力归属：Core Text／Flow。独立 crate 只使用公开 API，复用同一合同夹具。
当前夹具覆盖共享内容与请求、clone／分析请求、容量缩减、旧 lease、非法输入及布局等价。
另包含公开 typed 输入登记和 Runtime 内实际读取、发布及旧 evidence 留存用例。
另有外部 root／payload／handler 的开放声明用例，经普通 registry 和
FlowLayoutService 完成 Runtime 发布，验证旧来源 lease 的只读寿命。
`ReusablePage` 位于普通库代码中，Native／Wasm check 都编译其 Figure、
贡献声明、外部 handler 和 prepare adapter，不只编译共享字符串构造。
这些用例不关闭 FR-02 的完整生产矩阵。

```bash
cargo test --manifest-path tests/consumers/flow-assembly-reuse/Cargo.toml --lib --offline
cargo check --manifest-path tests/consumers/flow-assembly-reuse/Cargo.toml --lib --target wasm32-unknown-unknown --offline
```
