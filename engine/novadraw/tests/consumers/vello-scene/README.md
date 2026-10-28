# Vello Scene 外部消费者

类型：`implementation-guide`

独立 Cargo workspace，只通过公开 Runtime、RenderBackend 与
`novadraw_backend_vello::scene` API 使用编码器。不开窗口，不需要 GPU；
用于验证下游可消费性与首个输出切片的有限成本，不代替完整 Masonry/Xilem 应用。

```bash
cargo run --release --manifest-path tests/consumers/vello-scene/Cargo.toml --features scene
```

无 `scene` feature 时，同一输入仅准备 Runtime submission；用于比较最小可执行包的
代码保留增量。release profile 固定 opt-level=s、LTO、单 codegen unit、abort 和 strip。
100 个矩形、DPI=2，首次编码与 1000 次同源 warm 编码分别计时，
不包含模型准备、排版、事件、GPU、present 或窗口首次加载。
两版分别构建并在覆盖二进制前记录大小；不使用 debug 大小作为分发成本。

integration GPU/失败契约见
[scene_contract](../../../crates/novadraw-backend-vello/tests/scene_contract.rs)。
