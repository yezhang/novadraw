# K07 实施记录

用户批准修订 3 后，按 source capture、结构准入、目标导入与 view、Thumbnail、
宿主背景、InheritBackground 的顺序完成六个切片。
方案、ADR-044/045、API 语义账本和使用/内部原理书籍已同步。

共享视图以活源 Runtime 的稳定结果为来源，目标只消费不可变快照；
多版本 image/font 使用独立导入 ID，终止、结构环与失败均有受检反例。
Thumbnail 的输入使源 mutation 发生一次，同一快照更新两个视图，
overview resize 不重新捕获，取消订阅后完整清理。

宿主背景由公开 typed provider 输出，Core 在统一帧准备中冻结背景与 Canvas。
InheritBackground 独立宿主已验证透明/不透明 root、滚动与移动分离、背景变化/移除、
DPI、暂停恢复和后端 session repair。macOS Native 证据见
[K07-6](../../../doc/verification/reviews/k07-6-inherit-background-evidence-2026-10-23.md)；
其他平台背景像素仍待验收。

catalog 当前为 16 complete、8 in_progress、1 blocked，24 个可运行。
其中双 Canvas 背景入口使用独立 Native 二进制，普通面板包含 23 个入口。
剩余工作见
[官方示例清单](../../../doc/verification/plans/draw2d-official-examples-next-slices-2026-10-23.md)。

## 最终边界检查

workspace fmt/check、`cargo clippy --workspace -- -D warnings`、
`cargo check -p web-validation --target wasm32-unknown-unknown --lib`、
`scripts/check_architecture.sh` 和 `git diff --check` 均通过。
Native/Wasm API 检查确认 Runtime 28/28，workspace 仅有 Kurbo 0.13.1。
`cargo xtask docs` 检查 360 份 Markdown 通过。
日志位于 `target/verification/k07-final-{fmt,check,clippy,web,api,docs}.log`。
`cargo test --workspace` 完成全部 141 个结果目标：1,401 passed、4 ignored、
0 failed，包含单元、集成和文档测试。日志为 `target/verification/k07-final-test.log`。
主程序 `--list` 确认 catalog 为 16 complete、8 in_progress、1 blocked。
最后补齐窗口 surface Suboptimal 的重新配置处理后，受影响二进制的严格 Clippy 与
`example.inherit-background` Native 像素门禁再次通过。

交付前已按“好好说话”重新检查成稿的事实、语气和句子。
针对本记录与 K07-6 review 的中文半角标点、元话语、破折号机械扫描均为 0。
