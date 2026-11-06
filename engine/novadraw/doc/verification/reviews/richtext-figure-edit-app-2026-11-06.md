# Richtext 编辑性能样例验证

类型：`verification`

本报告保留 ADR-061 接入前的独立组件基线。当前统一会话、选区与文档历史的
实施状态及验证见[ADR-061 实施记录](specialized-edit-controller-2026-11-06.md)。

能力归属：`novadraw-richtext` 扩展组件与 Native 样例；共享宿主属于 examples/support。
使用 Core Runtime 的公开文字、坐标、滚动与 typed mutation 协议，不新增 Editor／GEF 能力。

`richtext-figure-edit-app` 已支持点击放置光标、段内输入、Unicode 字素删除、
Home／End、Enter 和 IME 提交，并提供真实文档修改的 CPU 回放。
编辑后的混合字号／Emoji Native 图像通过检查。系统输入法候选窗口仍待人工验收，
长文绘制与完整编辑器协议仍保持未完成状态。

## 实现与契约

[样例入口与命令](../../../crates/novadraw-richtext/examples/native/richtext-figure-edit-app/README.md)
说明操作方式和三档负载。基础样式按段落保存，所有可编辑段落都没有 span；
键盘和回放都通过 `edit_plain_paragraph` 构造候选，再通过
`ReplaceRichTextDocument` 提交。文档仅由正文 Figure 持有，样例只保存段落 ID、
字节光标、组合文字和测量结果。

`RichTextLayoutSnapshot::position` 用当前段落布局接纳光标；
`caret_bounds` 同时检查文档／布局身份并累加段落高度，返回内容坐标矩形。
样例使用 Core 受检变换完成点击定位与光标显示，逆变换不可用时跳过点击。
光标位于独立覆盖 Figure，裁剪到 viewport；编辑后将光标滚动到可见范围。
左右键和删除按 Unicode 字素处理。上下键按当前行高探测，不承诺完整混合字号跨段导航。

共享宿主原先会把单场景的左右键、Home／End 当作切换场景，从而重建编辑状态。
本次仅在场景数小于等于 1 时放行这些按键，多场景规则保持原有实现。

## 验证

使用系统 Rust 1.99.1、Apple M1 Pro。命令使用默认 Cargo，没有固定工具链要求。

- `cargo test -p novadraw-richtext --lib`：7 项通过，覆盖文档版本／共享、
  非法编辑、缓存以及光标高度偏移、越界位置、旧宽度／旧文档拒绝。
- `cargo test -p richtext-figure-edit-app --bin richtext-figure-edit-app`：2 项通过，
  覆盖组合 Emoji／旗帜／组合附标字素，以及实际 Runtime 编辑、文本恢复、
  命中定位、滚动揭示、resize 和无段落 Figure 子树。
- 单场景宿主按键回归通过；组件与样例的定向 Clippy 均通过。
- `--headless` 的实际 Runtime 验证通过。
- `--screenshot --replay-edit` 注入九组 IME Preedit／Commit，逐组确认
  Preedit 不修改版本、Commit 改变版本，并通过 Vello 离屏提交及 GPU 等待后截图。

后续 Wasm 组件检查和扩大到多场景的宿主复验未通过：并行 Core 改动中的
`figure/projection.rs` 出现 E0195，`scope<'a>` 的实现缺少 preparation input trait
要求的 `Self: 'a`。该错误发生在编译 `novadraw` 时，本轮未修改这一文件；
前述已通过记录与 release 可执行文件对应此变更进入前的构建，不代表当前 Core
工作树可重新构建。此前单场景回归已通过，后续多场景复验不能记为通过。
全量 fmt 的后续复查也发现该 Core 文件及 `figure/capability.rs` 的格式差异；
本轮样例、richtext 与 support 的定向 rustfmt 检查通过。

最终截图位于
`target/visual-verification/screenshots/richtext-figure-edit-app_Editing_performance_1793935790.png`。
图像中可见三次头部插入、14／40／64 px 文字、彩色 Emoji、光标、正文与滚动条。
这是 Native 后端的编辑后绘制证据，不是完整 OS 输入法或屏幕端到端延迟验收。

## CPU 测量口径

release 构建启用 `profiling` feature，正式成本回放不启动 trace。
工作站其他进程未隔离，采样只用于展示当前成本，不作为最终性能门禁。
每档运行三轮，每轮每个位置八组插入 → 替换 → 删除，总计 216 次修改。
每组完成后检查段落文本恢复原值。中部按整文档 UTF-8 字节中点找到所在段落，
再向前取合法字素边界；不是简单按段落数取中间段。

位置选择、滚动定位和初始排版作为 setup 排除。
`mutation_ns` 包含候选构造与 typed commit；`layout_ns` 为源修改后的稳定化；
`record_ns` 为 CPU 绘制命令生成和模拟呈现确认。
`cpu_frame_ns` 还包含光标更新、滚动揭示与必要的再次稳定化。
模拟呈现没有 GPU backend，不能当作 GPU 完成或屏幕响应时间。

负载规模为：mixed 20,772 字符／29 段，large 274,772 字符／133 段，
giant 274,010 字符／6 段；最长段落分别为 862、2,146 和 273,926 字符。
下表为插入 `编辑🙂` 的成本，单位 ms，各位置 24 次样本；其他操作的原始结果保留在 JSON。

| 负载／位置 | 修改 P50 | 排版 P50 | 记录 P50 | CPU 总计 P50 | CPU 总计 P95 |
|---|---:|---:|---:|---:|---:|
| mixed／头部 | 0.016 | 16.702 | 0.921 | 18.052 | 21.334 |
| mixed／中部 | 0.019 | 17.303 | 0.900 | 18.635 | 23.137 |
| mixed／尾部 | 0.017 | 16.458 | 0.911 | 18.003 | 19.547 |
| large／头部 | 0.024 | 214.472 | 12.897 | 236.791 | 299.210 |
| large／中部 | 0.026 | 214.301 | 13.017 | 233.609 | 246.632 |
| large／尾部 | 0.027 | 213.354 | 12.987 | 231.676 | 240.338 |
| giant／头部 | 0.023 | 213.140 | 13.476 | 232.735 | 400.908 |
| giant／中部 | 0.452 | 216.898 | 13.574 | 241.293 | 265.689 |
| giant／尾部 | 0.451 | 216.030 | 14.244 | 248.591 | 299.063 |

各阶段的中位数不能直接相加得到总中位数。三个负载的内容结构不同，这不是优化前后对照。
巨段中部／尾部的候选构造成本较高，但当前主耗时仍是完整排版。

独立 mixed trace 的 216 次编辑包含 432 次完整快照构造，每次恰好两次；
加上 setup 共 435 次，累计准备 12,615 个段落。
`richtext.edit.layout` 平均 20.183 ms；`richtext.edit.mutation` 平均 0.024 ms。
快照和段落 scope 是嵌套关系，其累计耗时不能相加。trace 用于归因，不代替上表成本。
这些结果说明当前编辑路径会重复构造全文快照，本次只展示和测量这一成本，未实施优化。

本地产物保存到 `target/verification/text-richtext-edit/{mixed,large,giant,trace}/`，
每档包含 `metadata.json`、`samples.json`、`summary.json`；
trace 档另有 `edit.pftrace`。这些中间产物不纳入 Git。

## 保留边界

每次源修改仍完整装配排版快照并输出全文绘制命令，未实现增量段落排版或可见输出。
本轮不把编辑样例记为 FR-02／LT 完成，也不关闭
[原长文黑帧](rich-text-figure-stress-performance-2026-11-05.md)。
跨段合并、样式 span 编辑、文本选区、剪贴板、撤销和完整双向文本导航仍未交付。
中文输入法候选展示和连续真实键盘输入还需要人工验收。
