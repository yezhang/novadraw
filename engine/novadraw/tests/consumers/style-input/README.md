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

S08-B 增加公开 Border 契约和 `border_workload` 成本入口。每种模式预热一次，
采样 101 次，每个样本执行 120 次操作。mode 0 构造两个嵌套 Compound、
两条 Line 和一条 Margin；mode 1 重复查询基础指标并构造一次冻结绘制；
mode 2 固定尺寸录制；mode 3 连续改变宽度录制。后两种模式测量 CPU Border
录制，未驱动完整 Runtime 帧或 GPU，不代表稳定帧缓存或窗口 resize 延迟。
迁移前后使用相同场景，原始接口仅作 Result 和 Insets 迁移。

Core 的 default feature 集为空，animation 当前常驻；默认和无默认消费者
均验证，没有新增 feature、依赖、字体资源或 loader。产物包含可达 Core
样式与树更新代码，不包含 Native 窗口或 GPU 宿主。

S08-C 增加 admission 和 detached geometry 两组公开契约，覆盖受检构造、
外部 Figure／Border、BorderCandidate、共享 owner、发布拒绝与有效恢复。
frame_workload 使用 64 个节点和共享嵌套 Border；三种模式依次为稳定帧请求、
连续 resize 与 style 更新，每次驱动 prepare_submission 和 Presented 确认。
稳定模式在首帧后允许 Idle。每个样本包含初始化、120 次后续帧请求及 teardown，
P50／P95 按整批计时，不能当作单帧时间或 GPU 延迟。
