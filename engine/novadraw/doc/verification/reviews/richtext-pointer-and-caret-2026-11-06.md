# 富文本点击耗时与光标闪烁修复

类型：`verification`

能力归属：richtext 排版快照缓存；应用编辑反馈；共享 Native 窗口宿主。
Editor 使用已有的光标计时协议，本次不改变 Core 或 Editor 的会话契约。
日期：2026-11-06。

状态：选区重复排版与窗口计时接入已修复；34 项定向测试、Clippy、Native 回放、
匹配 CPU 采样及当前工作区编译检查通过。首次会话准备及长文完整性能验收未关闭。

## 问题与修复

点击和拖选只改变选区，但应用的 `EditSessionPlan::feedback` 每次都新建
`RichTextContentFigure`。新 Figure 的缓存为空，反馈发布因此重复准备全文布局。
修复后，Plan 保存 detached Figure 原型，反馈克隆共享同版文档的有界缓存。
这是布局资产复用，草稿和选区仍由 `RichTextEditController` 持有。

`RichTextContentFigure::clone` 共享缓存，`with_document` 在文档版本变化时分离
缓存。附着组件的文档更新也分离缓存，避免新文档候选淘汰旧文档的准备结果。
缓存仍最多保存两个宽度结果，键仍检查文档、文字引擎身份、字体世代、宽度与方向；
文字引擎未声明可缓存时仍重新准备。锁不跨越文字引擎回调，保留 Figure 的 `Send`。

不闪烁的原因在窗口宿主：custom `WindowScene` 没有时钟和唤醒接口，而默认
Runtime 在切换到 custom scene 时被释放。Editor 已有的 530 ms 光标计时器
因此没有被驱动。现在 `WindowScene` 转发 `advance_time` 和
`next_wake_deadline`，共享宿主在等待边界调用并设置 `WaitUntil`，显隐变化请求
重绘。输入前也更新时钟，保证点击和输入从当前时间重置闪烁。截图模式保持冻结
时钟，其他 custom scenes 的默认实现不要求定时唤醒。

## 对照条件

工作区存在并行 Core 投影改动；一次构建被未完成的 `Affine2D` 导入及
`capture_stable_projections` 接口阻断。最终匹配验证使用提交 `1e769419` 的独立
源码副本，叠加本次修改；前后使用同一份工作区 `Cargo.lock`、默认系统 Rust、
系统 clang、release 配置、字体、1120 × 760 逻辑窗口及 preparation budget。
副本中的七份修复源码与工作区逐字节一致。
复制后刷新副本源码时间戳，再构建 release，确保宿主依赖使用本次实现。
最终当前工作区的 edit-app `cargo check --bin` 也已通过。

`--benchmark-pointer` 在首、中、尾各执行 8 次 press/release。
计时包含选区输入、反馈替换及准备，排除会话激活、定位滚动、CPU 命令记录和 GPU
提交。每次比较准备快照的 `Arc` 身份，并检查源模型文档版本未改变。
单轮 24 个样本不能证明系统呈现延迟或全部负载的尾部成本。

原始 JSON 与独立源码副本在
`target/verification/text-richtext-pointer/`，不纳入 Git。
`stable-before-*` 为最终匹配基线；较早的 `before-*` 使用当时工作区 Core，
只用于初始诊断，不混入匹配结论。

## 验证结果

点击 CPU 成本，单位 ms；每行 24 次点击：

| 文本负载 | 修复前 P50 / P95 | 修复后 P50 / P95 | 快照复用次数：前 → 后 |
|---|---:|---:|---:|
| mixed，20,772 字符 | 8.847 / 11.299 | 0.230 / 0.324 | 0 → 24 |
| large，274,772 字符 | 123.936 / 160.389 | 0.366 / 0.566 | 0 → 24 |

首次会话准备单列：mixed 从 9.669 ms 到 10.429 ms，large 从 116.753 ms 到
110.426 ms。这是各一次的冷准备观察，不作为冷启动提速结论。

| 检查 | 结果 |
|---|---|
| richtext `--lib` | 8 项通过；同版克隆复用、改变文档后分离缓存、丢弃候选不淘汰活动快照、宽度／字体失效、引擎 opt-out、预算与容量 |
| 共享宿主 `--lib` | 12 项通过；custom scene 时钟转发、截止时间、时钟错误及既有宿主行为 |
| edit-app `--bin` | 4 项通过；拖选保留同一布局、截止前可见、截止时隐藏、下一周期恢复、输入重置、取消后无唤醒，以及已有编辑事务 |
| Editor 直接编辑契约 | 10 项通过；composition、字素、源失效、历史、拒绝恢复及反馈 |
| Clippy | 本次 library 与 binary 分别使用 `--no-deps -D warnings` 通过 |
| release `--headless` | 独立与 Editor 嵌入宿主均通过；首中尾定位、composition、模型隔离、历史、取消及 resize/zoom |
| 当前工作区 | `cargo check -p richtext-figure-edit-app --bin richtext-figure-edit-app` 通过，包含并行 Core 的当前接口 |
| 文档与格式 | 定向 rustfmt、diff whitespace 通过；xtask 校验 518 份 Markdown，0 结构错误 |

release `--screenshot --replay-edit` 退出码为 0。检查
`target/visual-verification/screenshots/richtext-figure-edit-app_Editing_performance_1793946750.png`
的 2240 × 1520 图像：中文、混合字号、彩色 Emoji 和首行 caret 正常，
没有发现正文叠字或黑帧。截图冻结计时，只证明该 mixed 回放的绘制；
闪烁边界由自动测试验证，不把单帧截图当作真实桌面闪烁节奏的采证。

复验命令：

```sh
cargo test -p novadraw-richtext -p novadraw-example-support --lib
cargo test -p richtext-figure-edit-app --bin richtext-figure-edit-app
cargo test -p novadraw-editor --test p2_e02_direct_text_edit_contract
cargo run --release -p richtext-figure-edit-app -- --benchmark-pointer --text-load mixed --output target/verification/text-richtext-pointer/mixed
cargo run --release -p richtext-figure-edit-app -- --benchmark-pointer --text-load large --output target/verification/text-richtext-pointer/large
```

首次进入会话仍需要准备草稿布局；文字编辑仍需全文布局，本次没有实施增量排版。
既有长文黑帧、两份正文的命令记录和人工操作系统输入法验收继续保留原状态，
参见 [ADR-061 实施记录](specialized-edit-controller-2026-11-06.md)。
