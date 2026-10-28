# Shape artifact probe

同一源码、依赖与 release 配置，比较普通 Figure 消费和启用逐角/分边 builder 的
可达产物。该差值是当前引擎中两种消费方式的增量，不是 Core 改动前后的总差值；
普通 Figure 的公开绘制路径已经包含圆角模式分支。

```bash
CARGO_TARGET_DIR=target/shape-artifacts/base cargo build --release --manifest-path tests/consumers/shape-artifact/Cargo.toml
CARGO_TARGET_DIR=target/shape-artifacts/independent cargo build --release --manifest-path tests/consumers/shape-artifact/Cargo.toml --features independent
CARGO_TARGET_DIR=target/shape-artifacts/base cargo build --release --target wasm32-unknown-unknown --manifest-path tests/consumers/shape-artifact/Cargo.toml
CARGO_TARGET_DIR=target/shape-artifacts/independent cargo build --release --target wasm32-unknown-unknown --manifest-path tests/consumers/shape-artifact/Cargo.toml --features independent
```

不包含 Native/WGPU host、字体、图片或 Web loader；只测最终链接的形状录制消费者。
