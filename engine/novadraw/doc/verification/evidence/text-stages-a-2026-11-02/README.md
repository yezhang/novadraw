# ADR-059 A 验证证据

类型：`verification-record`

能力归属：Core Text。Rust 1.96.0；macOS arm64 与 Wasm。
结论和限制见 [实施记录](../../reviews/text-stages-a-implementation-2026-11-02.md)。

本目录保存定向验证、Core library 回归、独立 Native／Wasm 消费者、
公开 API 和文档检查的原始日志。构建目录使用 `/tmp/novadraw-text-stage-*`。
本片不采集宽度复用性能或产物体积。

Native C 工具链使用
`CC=/Library/Developer/CommandLineTools/usr/bin/clang` 与
`CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/Library/Developer/CommandLineTools/usr/bin/clang`。
公开边界脚本使用 `RUSTUP_TOOLCHAIN=1.96.0`。

```bash
cargo +1.96.0 test -p novadraw --lib
cargo +1.96.0 test -p novadraw --lib text::staged
cargo +1.96.0 test -p novadraw --test long_text_layout_contract --test p2_t03_styled_text_contract --test p2_t03_external_flow_engine --test shared_glyph_storage_contract --test p2_text_underline_contract
cargo +1.96.0 clippy -p novadraw --lib -- -D warnings
cargo +1.96.0 test --manifest-path tests/consumers/text-stages/Cargo.toml --lib
cargo +1.96.0 test --manifest-path tests/consumers/text-stages/Cargo.toml --lib --no-default-features
cargo +1.96.0 build --manifest-path tests/consumers/text-stages/Cargo.toml --lib --target wasm32-unknown-unknown
cargo +1.96.0 build --manifest-path tests/consumers/text-stages/Cargo.toml --lib --target wasm32-unknown-unknown --no-default-features
bash tools/scripts/check_architecture.sh
cargo xtask docs
cargo +1.96.0 fmt --all -- --check
```

独立消费者另完成 library 与测试夹具的 Clippy 检查。
测试中的存储预留失败由私有注入制造，不作为真实 allocator 的全故障证明。
