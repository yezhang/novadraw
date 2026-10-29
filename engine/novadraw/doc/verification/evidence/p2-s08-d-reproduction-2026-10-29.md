# S08-D 成本复现说明

类型：`reference`

日期：2026-10-29。保存消费者、驱动器、锁文件和源码差异，
最终测量结果由 S08-D 完成记录解释，不以本页声明验收通过。

## 源码

迁移前 Core 来自 `cfc7ac9c`，迁移后基线为
`4f104633fcb7c555d165cc20d2902ab554aa6d4a` 加
[Core 差异](p2-s08-d-current-core-2026-10-29.patch)。
Git 根为 drawjs，归档路径前缀为 `engine/novadraw/`；
恢复时保持 `crates/novadraw` 与 `assets/fonts` 的相对路径。
迁移后差异包含测试夹具，release 构建不启用 cfg(test)。

| 保存文件 | 恢复位置 |
|---|---|
| [最小基线 workspace](p2-s08-d-baseline-workspace-2026-10-29.toml) | 基线根 Cargo.toml |
| [消费者 manifest](p2-s08-d-consumer-2026-10-29.toml) | tests/consumers/style-input/Cargo.toml |
| [锁文件](p2-s08-d-consumer-2026-10-29.lock) | tests/consumers/style-input/Cargo.lock |
| [共同驱动器](p2-s08-d-driver-2026-10-29.rs) | 消费者 src/main.rs |
| [迁移前 lib](p2-s08-d-baseline-lib-2026-10-29.rs) | 基线消费者 src/lib.rs |
| [迁移后 lib](p2-s08-d-current-lib-2026-10-29.rs) | 当前消费者 src/lib.rs |

基线只缩小 workspace member 集合，不改 Core 源码。
两个消费者 manifest、lock 和 main 的字节一致；lib 仅适配公共 API。
旧样式入口在每次设置前解析文本一次，读取继承样式后再解析；
新入口保存 FontDescriptor，读取直接使用 descriptor。
这种差异是 S08-A 的目标行为，其他三个 Runtime 帧模式没有修改工作量。

release 不编译 lib 中的测试模块，因此复现成本不需要复制测试 include 文件。
字体归档与当前四个 TTF 已核对字节一致。来源支持文件、字体尺寸和 SHA-256 见
[复现收据](p2-s08-d-reproduction-2026-10-29.json)。

## 构建与采样

Native 使用 Rust 1.96.0，Wasm 使用 stable Rust 1.99.1。
配置由同一 manifest 冻结：opt-level=s、LTO、codegen-units=1、panic=abort、strip。
分别构建默认与 `--no-default-features`；所有构建使用 `--locked`。
Core 当前 default feature 集合为空，两组用于确认 feature 边界，没有假设动画可裁剪。

```bash
rustup run 1.96.0 cargo build --release --locked --manifest-path tests/consumers/style-input/Cargo.toml
rustup run 1.96.0 cargo build --release --locked --no-default-features --manifest-path tests/consumers/style-input/Cargo.toml
rustup run stable cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path tests/consumers/style-input/Cargo.toml
rustup run stable cargo build --release --locked --lib --no-default-features --target wasm32-unknown-unknown --manifest-path tests/consumers/style-input/Cargo.toml
```

为 before／after、Native／Wasm、default／off 分别使用独立 target。
构建后把 Native 可执行文件与 Wasm cdylib 保存为采样器要求的八个名称。
不在采样期间清理构建目录或重编译。

```bash
python3 doc/verification/evidence/p2-s08-d-cost-receipt-2026-10-29.py \
  --artifacts tests/consumers/style-input/target/s08-d \
  --receipt doc/verification/evidence/p2-s08-d-cost-receipt-final-2026-10-29.json \
  --rounds 10
```

每配置做十轮 before／after 交替采样；每次执行包含八种模式。
每模式预热一次，101 个样本，每样本 120 次后续操作。
frame 模式另含初始化首帧，稳定模式随后允许 Idle；计时包含初始化和 teardown。
P50／P95 为整批时间，allocator 为请求次数与请求字节，不是单帧、峰值或 RSS。
固定尺寸 Border 录制只测 CPU；Runtime frame acknowledgement 不代表真实 GPU 呈现。
Wasm 产物只做体积对照，没有声称浏览器运行时性能。

正式结果见[最终采样收据](p2-s08-d-cost-receipt-final-2026-10-29.json)，
其中保存 UTC 开始／结束时间、十轮原始结果和八个产物指纹。
本 Agent 的编译、测试与 GPU 任务结束后才运行正式采样；
较早的配对记录保留为诊断，没有用于最终延迟结论。
