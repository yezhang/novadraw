# Parley 存储接口独立消费者

类型：`verification-guide`

能力归属：Core 文字依赖验证；本消费者不依赖 Novadraw。
用于检查 ADR-059 B 已接入生产依赖的存储观察及操作准入接口，
不代表阶段适配器或 Flow 已实现。
完整范围见[补丁复核记录](../../../doc/verification/reviews/text-stages-b-storage-patch-2026-11-02.md)。

消费者直接使用 `dependencies/parley-0.7.0`，不需要先准备隔离副本：

```bash
cargo test --locked \
  --manifest-path tests/consumers/parley-storage/Cargo.toml --lib
cargo test --locked \
  --manifest-path tests/consumers/parley-storage/Cargo.toml \
  --no-default-features --features portable --lib
cargo build --locked \
  --manifest-path tests/consumers/parley-storage/Cargo.toml \
  --no-default-features --features portable \
  --target wasm32-unknown-unknown --lib
```

macOS 如默认 linker 不可用，可将 `CC` 与
`CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER` 指向 Command Line Tools 的 clang。
构建目录可用 `CARGO_TARGET_DIR` 独立指定。

消费者各配置有 8 项测试：120 组读取前后与 reflow 对照、模板与行结果容量、
字体 backing 容量及最后一个布局引用释放、960 组复制后对齐转换及四个准入拒绝点。
另有 640 组断行对照、逐个拒绝点与长段落线性计费检查。
旧记录的 160 组分母在复制切片按实际循环更正。
它们验证依赖接口的作用范围，
不代替 ADR-059 C 的公开结果、来源与交互全量等价门禁。
Core 会话预算另有 5 项定向测试，入口为
`cargo test -p novadraw --test p2_parley_operations_contract`；
完整成本依据及限制见[操作记录](../../../doc/verification/reviews/text-stages-b-operations-2026-11-02.md)。

断行准入和普通入口的范围见
[断行记录](../../../doc/verification/reviews/text-stages-b-line-breaking-2026-11-02.md)。
上游对照脚本需要已经从原始归档准备的 baseline；
如尚未准备，可先使用准备脚本加 `--baseline`。
它比较 960 组普通入口输出，并在隔离副本运行 2 项 Drop 哨兵测试：

```bash
python3 tools/scripts/check_parley_line_admission.py \
  --archive /path/to/parley-0.7.0.crate \
  --target-dir /tmp/novadraw-text-stage-consumer \
  --output target/text-line-admission-evidence
```

新增模块另有 5 项私有容量／算术测试：
这些测试使用隔离 harness。先按实际归档路径准备；Python 需要 3.12 或更新版本。

```bash
python3 tools/scripts/prepare_parley_storage_candidate.py \
  --archive /path/to/parley-0.7.0.crate
cargo test \
  --manifest-path target/text-stages-b/parley-0.7.0/Cargo.toml \
  --no-default-features --features std,storage-candidate-tests \
  --lib layout::storage::tests
```

发布版 Parley 缺少上游 `parley_dev` 测试支持包。准备脚本给隔离副本添加
`storage-candidate-tests`，仅在定向私有测试中排除上游总测试模块，
新增存储模块的测试仍运行。该 harness 改动不在生产候选补丁内；
本轮没有运行或宣称通过 Parley 全套上游测试。
