# 样式输入消费者

独立 crate 只通过公开领域路径消费 Core。契约测试复用
`p2_s08_style_input_contract.rs`，在独立依赖解析环境下验证 opacity 输入原因、
字体 token、第三方 Figure 的继承与更新，以及 Graphics 的测量和绘制。

```bash
rustup run 1.96.0 cargo test --manifest-path tests/consumers/style-input/Cargo.toml --lib
rustup run 1.96.0 cargo test --manifest-path tests/consumers/style-input/Cargo.toml --lib --no-default-features
rustup run 1.96.0 cargo run --release --manifest-path tests/consumers/style-input/Cargo.toml
rustup run stable cargo build --release --lib --target wasm32-unknown-unknown --manifest-path tests/consumers/style-input/Cargo.toml
```

成本场景为两个挂载节点，每个样本 120 次父样式更新及子节点继承查询，
预热一次后运行 101 个样本。记录 P50/P95、allocator 请求次数及请求字节，
包括 Runtime 构造与 teardown，不等于逐帧 GPU 或平台输入延迟。
字号在 12px/13px 间切换，alpha 为 0.5。迁移前后使用相同 release 配置，
只有样式组装和读取改为受检值；原始文本仍在每次用户更新时解析一次。

Core 的 default feature 集为空，animation 当前常驻；默认和无默认消费者
均验证，没有新增 feature、依赖、字体资源或 loader。产物包含可达 Core
样式与树更新代码，不包含 Native 窗口或 GPU 宿主。
