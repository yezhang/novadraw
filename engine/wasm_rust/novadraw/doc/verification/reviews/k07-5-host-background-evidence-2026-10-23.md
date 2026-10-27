# K07-5 宿主背景受检合成

类型：`verification-record`

日期：2026-10-23

状态：`complete`；完成 Core 契约、Vello 合成与切片门禁。

范围：[ADR-045](../../adr/adr-045-host-background-submission.md)的背景提供者、
受检交接、帧输入冻结与恢复。官方 InheritBackground 消费者由 K07-6 验收。

## 提供者留在宿主

公开背景 API 位于 `host` 领域。默认 `RECORDED_BACKGROUND` 与外部
`HostBackgroundProviderKey<P>` / `HostBackgroundProvider<P>` 均由 Host-owned
registry 调用，Core 不保存 provider、执行 I/O 或轮询 future。
外部消费者通过 `RuntimeFrames` 取得 request，交入完整不可变候选。

请求包含 destination、来源、generation、request generation 与坐标输入。
更换来源或移除推进 generation，较新 request 使旧 request 失效。
同 generation 的候选 revision 由宿主递增；完整内容重复幂等，
同版本异内容返回 `SnapshotConflict`。候选导入在 Registry 与导入表副本中完成，
闭包错误不部分替换已接受背景。旧请求错误不覆盖当前错误槽。

surface、Canvas 位置与 DPI 使用独立版本；变换使用现行受检 inverse。
旧输入不匹配时拒绝帧准备，保留旧 CPU 结果与 pending work。
初次 Pending 使用明确的基础清屏值；失败保留成功背景。
背景变化、移除和输入变化独立请求完整重绘并唤醒 Host。

## 背景与 Canvas 一起冻结

背景绘制使用 host-to-Canvas 变换，不追加内容 Viewport origin。
Runtime 在相同准备入口中录制完整 Canvas、在其前插入受检背景前缀，
共同校验命令、capability 与设备数值，再冻结资源前缀与 CPU lease。
帧包包含 `clear_color` 和 `background_input`；Vello surface、截图和离屏目标
使用该清屏值，因此移除时能清除透明背景旧像素。

背景参与时使用 Full damage 和完整 Canvas recording。
后端需声明 `HostBackground` capability，并在资源接收前检查组合输入。
未支持后端返回明确错误；输入失配或局部 damage 的外部帧包也会被拒绝。
Retry/Failed、reset 和迟到 frame completion 复用既有单 in-flight 与导入 lease
协议；Presented 不清除在途期间产生的新背景更新。

## 已获得的证据

`p2_host_background`：7 个外部契约测试通过，覆盖独立更新与唤醒、
在途更新冻结、初次 Pending、失败保留、移除/迟到、stale/conflict、位置/DPI
失配、完整资源闭包、Retry/reset 和不支持后端。
日志：`target/verification/k07-5-contracts.log`。

`host_background_pixels`：Native GPU 像素测试通过，证明不透明 Canvas 遮住
背景、透明及半透明 Canvas 正确合成、内容独立移动不改变宿主背景、
Canvas 位置改变采样域、coverage 外使用基础清屏值，以及移除恢复透明像素。
日志：`target/verification/k07-5-pixels.log`。

稳定验收入口为 `graphics.host-background`，编排 Core 交接、身份耗尽和 GPU 像素；
命令定义由 [xtask](../../../tools/xtask/src/main.rs)维护。
`graphics.host-background`：7 项外部契约、1 项身份耗尽与 1 项 Native GPU 像素
通过，日志为 `target/verification/k07-5-suite.log`。
`cargo clippy -p novadraw --lib --test p2_host_background -p novadraw-backend-vello --test host_background_pixels -- -D warnings`：
通过，日志为 `target/verification/k07-5-clippy.log`。
`tools/scripts/check_architecture.sh --update-api`：Native/Wasm 表面通过，Runtime 28/28，
Kurbo 0.13.1；日志为 `target/verification/k07-5-api.log`。
库级测试保留既有 14 项 warnings，本记录只声明本切片指定 Clippy 目标通过。
`cargo xtask docs`：359 个 Markdown 文件及当前命令引用通过，
日志为 `target/verification/k07-5-docs.log`。`git diff --check`：通过。

本批新增中文已按事实、措辞和节奏复读；半角标点、破折号、元话语与 B2
机械扫描均为 0 项，源码标识符保留原词。

## 验收边界

本记录不替代官方双 Canvas 消费者及 Native 窗口截图。
Web 编译和 workspace 完整门禁在 K07 最终交付边界执行；
本记录不声明 Web/Windows/Linux 平台视觉验收通过。
