# 颜色文本解析拒绝非法输入

类型：`implementation-verification`

日期：2026-10-29

范围：ADR-020 既有 Color 解析契约；本切片已修复并通过定向验证。

`api_semantics`：`graphics.context`。

## 反例与根因

`Color::from_hex` 先检查去掉可选 `#` 后的字节长度，再按两个字节切片并交给
`u8::from_str_radix`。输入 `中abc` 满足六字节长度，但第一段切片落在中文字符内部，
触发 panic。`+f0000` 则因整数解析允许正号而被接受，违反十六进制数字的语法。
`FromStr` 委托相同入口，也受影响。

修复前，新外部消费者测试为 1 passed、2 failed；失败分别确认 UTF-8 切片 panic
和正号被接受。公开契约见
[ADR-020](../../adr/adr-020-engine-value-and-render-contract.md)，
实现见 [Color](../../../crates/novadraw/src/color.rs)。

## 修复与消费边界

解析器直接读取字节，只接受 ASCII `0..9`、`a..f` 和 `A..F`，两位合成为一个
RGBA 分量。字节长度校验保证所有索引有效，解析不再切片 UTF-8 字符串。
非法数字继续返回首个受影响分量的 `InvalidDigit`；非法长度继续返回
`InvalidLength`。合法大小写、可选 `#`、六位默认不透明和八位 alpha 顺序保持不变。

本切片没有新增公开类型或依赖，解析只处理六或八字节，不分配内存。
没有修改 Figure、Border、Runtime 提交或递归渲染。没有做产物或性能测量，
不宣称包体积变化或性能提升。

## 定向验证

外部测试见
[color_text_input_contract.rs](../../../crates/novadraw/tests/color_text_input_contract.rs)。
它覆盖 Unicode 字符落在各分量及跨分量边界、正号、空白、`FromStr`，
并穷举 256 个分量值，核对大小写、前缀和 alpha。

| 命令 | 结果 |
|---|---|
| `cargo test -p novadraw --test color_text_input_contract` | 3 passed，0 failed |
| `cargo test -p novadraw --lib color::tests` | 9 passed，0 failed；库级测试编译有既有 warning |
| `cargo +1.96.0 clippy -p novadraw --test color_text_input_contract -- -D warnings` | 通过 |
| `cargo check -p novadraw --lib` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `git diff --check` | 通过 |

默认 Rust 1.99 的严格 Clippy 被既有 SVG `chunks_exact_mut` 和两处队列
`drain_collect` lint 阻断。本切片没有修改这些无关路径，也没有关闭 lint。
Rust 1.96 的严格定向检查通过，不代表当前 workspace 全量门禁通过。

`cargo xtask docs` 被既有长文本记录中的缺失文件
`target/verification/long-text/scroll-resize-shared-20261028/hot-comparison.json`
阻断，本切片没有修改该记录或生成其证据。中文成稿已回读，机械扫描的中文半角标点、
破折号、元话语和编号标题均为 0 处；术语、命令和测试结果已逐项核对。

## 后续范围

S08 整批保持 `not_started`。边框构造 panic、公开字段绕过不变量、样式语法和
非法 alpha 的处置仍需盘点后形成设计；已有合法饱和构造、范围收敛和圆角适配
保持原语义。本记录只关闭颜色文本解析反例，不关闭 S08 或整个 API-10。
