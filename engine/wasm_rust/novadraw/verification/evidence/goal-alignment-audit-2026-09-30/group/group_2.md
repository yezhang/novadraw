# Group 2：跨平台与平台宿主契约审计

类型：`verification`

日期：2026-09-30。范围：`full_file`，仅 Group2 平台与实际桥接链路，不限 diff。

## 1. 审计口径

- `SKILL_ROOT`：`/Users/bytedance/.trae-cn/skills/bits-code-guard`。
- `REPO_ROOT`：`/Users/bytedance/Documents/code/GitHub/drawjs`。
- `PROJECT_ROOT`：`engine/wasm_rust/novadraw`。
- `WORK_DIR`：项目内 `verification/evidence/goal-alignment-audit-2026-09-30`。
- 按 AGENTS -> CLAUDE 启动，读取项目记忆、既有 review_files/review_groups，再按规范到实现的顺序审阅。
- 起始 HEAD 为 `811b748f0a510edb90ad198346950ca3752b404d`，纳入当时全部相关 dirty 内容，包括未跟踪 `direct_edit_mode.rs`。审计期间其他工作流提交了这些内容，观察到 `3e99f02` 和 `841be42`；没有修改、撤销或提交用户工作。以下行号以本次实际读取的文件内容为准，末轮检索确认主要缺陷仍存在。
- 下文未加前缀的文件路径相对 PROJECT_ROOT；JSONL 的 `file` 相对 REPO_ROOT。
- 这是静态契约审计，不是四平台运行认证。未执行 Cargo、浏览器、原生窗口、性能基准或第三方源码构建。没有把测试存在写成测试通过。
- 读取完整相关方法、接口和直接调用方后检查七维度；Rust 无专项规则。原始组级 JSONL 供主 agent 汇总、交叉复核、排序及最终最多五项筛选，不覆盖总报告。
- 第三方框架参考边界已确认：`gef-classic/org.eclipse.draw2d` 与 `org.eclipse.gef`；未使用 Zest，也未以其扩展逻辑定义目标。本组额外只核对已锁定 Winit 依赖的平台事件契约。

## 2. 结论摘要

**目标明确是 Web、macOS、Windows、Linux 四平台，同时保持可扩展、平台无关 Core 和增量更新。现在可以证明包边界与主要输入机制已分层，但不能证明四平台运行验收全部完成。**

组级确定缺陷共 6 项，均为特定交互下的 P1，无 P0：DOM release 重入借用、Web 滚轮方向、Web 非左键误激活、Web accessibility action 断链、composition 去重后值残留、Native AltGr 文本被丢弃。置信度与具体路径见第 4 节及 JSONL。

Windows/Linux 缺少运行证据、完整原生 accessibility provider 后置、Core IME 与 Editor 专用桥接的层级差异，均没有作为运行失败写入 JSONL。

## 3. 目标到实际层级

| 契约 | 实际证据 | 判断 |
|---|---|---|
| 四平台长期目标 | `doc/design/architecture/overview.md:12-21,289-302`；`static-architecture.md:404-425` | 目标明确；Winit 承载三个桌面平台不等于三个平台都已验收 |
| Core 不依赖平台/backend/Editor | `novadraw/Cargo.toml`；`novadraw/src/lib.rs:17-45,101-103`；两个 platform Cargo.toml | 已对齐所读 manifest 和公开 host/event 值类型；未重新执行完整依赖图门禁 |
| 通用分发、坐标适配下沉 Core | `runtime/event/mod.rs:352-660`；`runtime/context.rs:395-410,568-650` | target、capture、focus、gesture、surface-to-local 与事件回调均在 Core；examples 没有另写一套 Figure target-local 转换 |
| Host 不拥有 Runtime/backend 领域实现 | `host/scene_host.rs:10-26`；Winit `host.rs`；Web `host.rs` | 已对齐。Winit 保存 Window 和平台效果；Web 是 callback adapter；Runtime/backend 由 composition root 组合 |
| redraw/wake/surface 生命周期 | support `app.rs:169-201,233-293,385-440,700-742`；Web `lib.rs:628-691,715-722,818-861` | Native WaitUntil、Web timeout generation、surface 更新链路存在；Web 示例 request_redraw 实际置 Cell 后同步 render，不是可复用 rAF 调度器 |
| Tooltip 时序/放置 | Core `runtime/tooltip.rs`；support `app.rs:251-266,758-810`；Web `lib.rs:1053-1079` | source/timing/placement 在 Core，Native overlay 和 Web DOM 展示在 examples。共享放置算法没有复制 |
| Accessibility Core | `runtime/accessibility.rs:54-94,139-264,287-410`；`runtime/runtime.rs:928-987` | snapshot/delta、namespaced identity、focus/default action API 已存在；Core 测试不证明 OS/DOM action 已接通 |
| Accessibility Native | Winit `host.rs:13-18,40-50,125-134`；support `app.rs:263-266` | 仅保存最后一次 update/revision，不是完整 baseline mirror 或 OS provider。完整 AccessKit/VoiceOver 明确后置，不报“原生 accessibility 全面失败” |
| Accessibility Web | Web `host.rs:68-73`；Web 示例 `lib.rs:1082-1175` | DOM name/role/disabled/toggle 输出存在；focus/default action 回流缺失，见 G2-04 |
| Core IME | `host/scene_host.rs:10-25`；Winit `host.rs:101-109`；Web `host.rs:60-62` | 定义 enabled/cursor area，Native 执行 Winit API，Web 仅转发 callback；不是完整文本编辑会话 |
| Editor IME 实际接入 | Winit `text_input.rs:47-89,93-218`；Web `text_input.rs`、`dom_text_input.rs`；Native `main.rs:1624-1631,1910-1941`；Web `direct_edit_mode.rs:351-359,553-574` | optional editor feature 提供 session-tagged event/effect；Native 与新 Web composition root 均真实消费，而非仅定义接口 |
| candidate area 与时钟 | DOM `dom_text_input.rs:193-230`；Web `direct_edit_mode.rs:438-458,567-574,587-633`；Native `main.rs:1607-1621,1994-2047` | dirty 改动补上 client origin、event-ready 和 caret wake；承認这些正确补齐，不倒置 diff 报告已修问题 |
| 可替换性/性能 | ADR-023；两个 platform lib/Cargo；Core event/context | 平台不持有树/渲染器；主渲染循环未分析或修改。同步 render、timer 频率的性能判断交 Group1，不在没有数据时判性能失败 |

### 四平台证据边界

| 平台 | 现有记录能证明什么 | 不能推出什么 |
|---|---|---|
| macOS | M10 人工记录为 macOS/Chrome；P2 direct-edit 记录 macOS 14.7.8/Vello/系统输入法用户确认 | 不证明任意键盘布局、任意输入法或新边界序列均通过 |
| Web | P2 记录 Chrome 154.0.8037.58/Vello 人工确认；Canvas2D 自动复核明确只覆盖 DOM/Editor 与非空像素 | 不证明 Firefox/Safari、所有移动输入序列或本次未运行的浏览器结果 |
| Windows | Winit 路径及 Ctrl 快捷键说明存在 | 未取得 Windows 原生构建、GPU、DPI、IME、窗口生命周期实测记录 |
| Linux | Winit 路径存在 | 未取得 X11/Wayland、字体/IME、GPU、DPI 实测记录 |

依据：`doc/verification/manual/web-platform.md:28` 明确排除 Windows/Linux 原生与 Firefox/Safari；
`manual/m10-tooltip-accessibility.md` 第 5 节；`manual/p2-direct-text-edit.md` 通过记录；
`reviews/draw2d-core-1.0-final-audit-2026-09-13.md` 第 9 节明确后置 Windows/Linux 发布资格。
`verification/suites.toml:632-664` 的平台为 native-macos/web，925-941 是 wasm/web build。
检索该 manifest、manual/reviews 及 gitroot `.github` 未得到四平台 CI/实测矩阵；这是证据缺口，不是平台失败。

## 4. 确定代码缺陷

### G2-01 [P1，9/10] DOM Release 持有 bridge 可变借用时同步触发 blur

- 位置：`novadraw-platform-web/src/dom_text_input.rs:205-230`。
- 完整路径：hidden textarea 获得焦点 -> Enter/无 composition 的 Escape -> DOM keydown listener -> event_ready -> `DirectEditWebApp::on_text_input_ready` -> Editor 产生 Release -> `sync_text_input_effects` -> `WebTextInputHost::apply_effect`。
- `if let Some(action) = self.bridge.borrow_mut().apply_effect(effect)` 的 RefMut 在成功分支结束前仍存活；Release 调 `apply_action`，其 293-296 行调用 textarea `blur()`。DOM 同步 blur listener 在 170-175 行再次执行同一 `bridge.borrow_mut()`，触发 RefCell already borrowed panic。把 active 清成 None 不能规避，因为 borrow 发生在 `focus_lost()` 内部检查 active 之前。
- 支撑：`direct_edit_mode.rs:351-359,553-574`；`dom_text_input.rs:134-178,279-299`；`novadraw-editor/src/domain.rs:346-393`；Viewer Release 发布点 `viewer/mod.rs:1705,1758`。
- 问题代码：`if let Some(action) = self.bridge.borrow_mut().apply_effect(effect) { ... input.blur(); }`。
- 建议：先用独立语句取得 owned action，结束 bridge 借用，再执行会触发 DOM 回调的操作；同时明确 event-ready 回写的同步重入边界。
- 归类：条件性功能缺陷/健壮性。限定在 textarea 仍聚焦的主动 accept/cancel；用户先手动移焦后再 Release 不触发同一路径。此结论来自静态调用链，未声称本次浏览器复现。与既有人工 PASS 记录应并存，主 agent 优先重放该序列核对。

### G2-02 [P1，10/10] Web 普通滚轮方向与 Core 的符号约定相反

- 位置：`novadraw-platform-web/src/input.rs:145-158`。
- DOM delta 原样从 `examples/web/web-validation/src/lib.rs:958-991` 交 adapter，adapter 对 Pixel/Line/Page 都只转换量纲、不反号。Core `container/scroll_pane.rs:193-203` 消费为 `model.set_value(old - distance)`。
- 触发：可滚动 pane 位于顶部，DOM 向下滚动 `deltaY=+100`、Pixel、Ctrl=false -> Core 请求 `0-100` 并被 clamp 为 0；滚动到中段后同样输入反而向上移动。横向同理。
- 本地外部契约核对：Winit 0.30.12 `src/event.rs:953-975` 明确正值表示内容向右/下移动；其 `src/platform_impl/web/web_sys/event.rs:150-151` 对 DOM `delta_x/y` 明确取负。Winit adapter 本身的符号与 Core 一致。
- 建议：Web scroll 归一化时统一转为 Core 内容位移方向；保留 Ctrl-wheel zoom 当前独立的指数符号，不对整个函数盲目取反。回归断言应检查 viewport origin，而不只是 adapter 输出数值。
- 归类：条件性功能缺陷/逻辑错误。非缺证据推测，也不是修改 Core 滚动契约的理由。

### G2-03 [P1，10/10] Web 把右键和中键伪装成左键，触发 Button/Toggle 动作

- 位置：`examples/web/web-validation/src/lib.rs:930-946`。
- 触发：在 Widgets 场景的 Toggle 内右键按下并释放，PointerEvent.button=2，但入口始终传 `MouseButton::Left`。Core `figure/widget.rs:166-177` 只允许左键按下/释放激活，现有桥接因此绕过该区分并切换 selected；中键也被误映射。
- 同根因实例：`examples/web/web-validation/src/direct_edit_mode.rs` 的 `on_pointer_down/on_pointer_up` 同样硬编码 Left，`on_click` 不区分按钮。只记一个缺陷，不逐调用点重复报。
- 建议：平台适配中映射 DOM button=0/1/2，并把不支持的按钮明确忽略；click-to-edit 的策略也按主按钮过滤。
- 归类：条件性功能缺陷/逻辑错误。不是要求实现多 pointer 或完整 pointer-cancel。

### G2-04 [P1，9/10] Web accessibility 输出 button role，却没有默认动作和焦点回流

- 位置：`examples/web/web-validation/src/lib.rs:1140-1162`。
- 触发：访问 Widgets/Tooltip Accessibility 或 Toggle 场景，由辅助技术对语义树中的 button 执行 click/default action。DOM 是带 role 的 div，没有 click/keydown/focus handler；也没有把 DOM element 绑定回 AccessibilityNodeId。事件监听只注册于 canvas、导航按钮和 window，`lib.rs:1325-1377` 没有语义树委托。
- 反向检索：`novadraw-platform-{winit,web}` 与 `examples/{support,native,web}` 中无 `perform_accessibility_action` 调用。Core API 本身存在且复用受控事务：`runtime/runtime.rs:940-987`。
- `snapshot.focus/delta.focus` 和 `node.state.focused` 也未映射为 DOM active descendant/focus；`tabindex=-1` 不是回流实现。
- 契约：`tooltip-accessibility.md:16,311-327` 要求 focus/default action 平台回调入口；389-395 仅后置完整 Native AccessKit/VoiceOver provider，没有后置 Web 的基本 action。
- 建议：在 platform-web 层提供可复用、带稳定 NodeId 映射的 DOM bridge，动作通过回调交 Runtime，投影变化保持焦点映射。不要在 adapter 直接修改 Figure。
- 归类：条件性功能缺陷/业务语义。已存在可被辅助技术发现的按钮却不可操作，不是因“没有完整 ARIA”报错。既有记录只验证 DOM/AX 节点存在和 canvas 键盘，未覆盖此回流。

### G2-05 [P1，9/10] composition 去重分支吞掉 input，却留下已提交的 DOM value

- 位置：`novadraw-platform-web/src/text_input.rs:137-147`。
- 触发序列：active session -> `composition_ended("中")`，产生一次 InsertText 和 ClearValue -> 浏览器后续 input 的 host value 为 `"中"` -> suppress_input 匹配后 `return None` -> 用户继续输入 `"a"`，textarea value 为 `"中a"`。
- DOM listener `dom_text_input.rs:156-168` 仅在返回 Some 时执行 ClearValue，因此匹配分支没有清空 `"中"`。下一次 bridge 将整个 `"中a"` 当成新增文本，已有 draft 中的 `"中"` 被重复插入。
- 这正是代码声称支持的 post-composition input 去重路径，不假定所有浏览器都有这一顺序；在没有尾随 input 的实现里，长期保留 suppress_input 也缺乏事件身份，需另作顺序验证。
- 建议：把“无 Editor event”与“仍需 ClearValue”分开表达，确保吞掉的非 composing input 也清空 host；按 composition 序列和 inputType 判定重复，而不是无限期比较相同字符串。
- 归类：条件性功能缺陷/逻辑错误。具体给定序列的错误可直接从 bridge 与调用方证明；本次没有宣称该序列在所有输入法出现。

### G2-06 [P1，9/10] Native 丢弃 AltGr 产生的合法文本

- 位置：`novadraw-platform-winit/src/text_input.rs:168-183`，关键 guard 在 171 行。
- 触发：active direct-edit、非 composing，使用 Windows 德语布局 AltGr+Q 产生 `text=Some("@")`，modifier 含 Alt。`KeyCode::KeyQ` 不属于命令分支，文本分支却要求 `!control && !meta && !alt`，于是直接返回 None。
- 直接调用方 `examples/native/node-editor-demo/src/main.rs:1901-1909,1919-1941` 把 modifiers 和 `event.text` 原样交 bridge；active session 分支随后直接 return，无后备文本输入。
- 外部契约已核对：本地 Winit 0.30.12 `src/event.rs:574-593` 将 text 定义为该按键产生的文本；`src/platform_impl/windows/keyboard.rs:232-248` 显式处理 has_alt_graph，保留系统文本。不是根据“Windows 未验收”推测其必然失败。
- 建议：区分真正快捷键与布局字符生成，不将 Alt/AltGr 一概视为不可输入；使用平台明确的文本/修饰语义，保留导航的 Option/Ctrl word 行为。
- 归类：条件性功能缺陷/逻辑错误。当前映射在该合法输入下确定丢字符；Windows/Linux 的运行认证仍独立待补，不能将此静态路径扩大为整个平台不可用。

## 5. 已确认差距与待验证风险，不进缺陷 JSONL

### 模块归属仍未完全兑现

- ADR-023 第 3 节将 DOM/canvas/浏览器调度/宿主生命周期归 platform adapter，但 `novadraw-platform-web/src/host.rs` 当前是 callback 转发壳。timeout generation、Tooltip DOM、accessibility DOM mirror 全部留在 `examples/web/web-validation/src/lib.rs:628-691,1053-1175`；Native Tooltip 的文字测量和 overlay lowering 留在 support `app.rs:758-810`。这不是仅靠重导出就完成了可复用平台能力提取。
- 其中 Runtime/backend 的组合、场景切换、demo 快捷键属于合法 composition root，不应全部搬进 Core。应收口的是可复用的平台呈现/调度 adapter，不是将窗口与 DOM 反向依赖引擎。
- Tooltip 专题 `tooltip-accessibility.md:90-102` 本身允许 example-support 同步效果，因此不能仅凭同步代码位于 examples 就判架构违例。实际可复用 DOM/overlay 实现的位置与 ADR-023 需进一步裁决。
- `p2-direct-text-edit.md:63-64` 写“Core 定义平台无关文本输入 event/effect”，实际类型在 `novadraw-editor/src/text_input.rs:1-155`，两个 platform 通过 optional editor feature 消费。默认 Core 依赖边界保持干净；这是精确的设计/代码归属不一致，不是已经证明的运行错误。交 Group3/Group4 裁决契约，而不是修改文档掩盖差异。

### 风险/证据缺口

- **原生 accessibility**：Winit 只保留最后 update，既未实现系统 provider，也未维护完整 Snapshot+Delta 基线。当前专题明确后置 provider；要扩展成可替换原生桥时需先定义 mirror/重建协议。`scripts/check_accessibility.sh` 只是打开 macOS 权限设置，不是 accessibility provider 验证。
- **lease 迟到事件**：两个 adapter 都以收到事件时的 `active` 给原生事件打标签。已有已标记旧事件会被 Editor 拒绝，不等于 OS/DOM 回调跨 release/acquire 后仍能辨识旧源。缺少 generation/source identity 的实际重放证据；不把未证明的旧事件时序升级为确定缺陷。
- **composition cancel/Native 去重**：Web `composition_ended("")` 与 Native `suppress_text` 的跨按键寿命值得定向重放。未继续扩展到 Editor 内部状态机审计；需要确认真实输入法序列，作为主 agent 的后续输入而非本组确定缺陷。
- **DOM failure 模型**：focus、style、attribute API 错误被忽略；尚无具体失败环境证据，不因 `_ = Result` 单独报 P1。`HostTextInputUnavailable` 的端到端路径仍需产品环境验证。
- **multi-pointer/cancel**：WebPointerInput 保留 pointer_id，但实际示例使用 primary-mouse facade，未接多点聚合。专题已排除完整多 pointer/pointer cancel；长期目标尚未兑现不等于本批失败。
- **gesture fallback 文档**：专题描述缺 Begin 可恢复 session，Winit adapter 对孤立 Moved 明确输出 Impulse，并有测试固定该行为。应记录平台变体，不仅凭理想规范示例判现有状态机坏掉。
- **性能与调度**：Web callback host 在示例中同步 render；是否应统一到 rAF、是否影响目标帧时，交 Group1 基准验证，不以静态重复代码推测性能退化。

## 6. 七维度复核

| 维度 | 结果 |
|---|---|
| 逻辑 | G2-02/03/05/06；检查输入单位、按钮、modifier、清空对称性 |
| 业务语义 | G2-04；区分“语义节点存在”和“用户能操作” |
| 安全 | 所读 Tooltip 文本用 text_content，ARIA 用属性赋值；未发现具体可利用注入路径，不报告理论问题 |
| 并发 | 单 UI 线程和 Rc/RefCell 是既有契约；G2-01 是同步回调重入，不虚构跨线程数据竞争 |
| 健壮性 | G2-01；其他吞错仅列需要验证的失败模型 |
| 性能 | 仅审平台调度职责，未深入热路径/benchmark；没有确定性能缺陷结论 |
| 质量 | 不因长函数、命名、常量或简单重复报缺陷；分层差距单列，不混入功能 JSONL |

Rust 没有专项规则文件；没有把 HTML/CSS 或生成的 wasm-bindgen JavaScript 当本组 JS/TS 业务代码审查。

## 7. 给主 Agent 的最小验证

以下均为推荐，**本组未执行**。不建议先跑 workspace full。

1. 最优先用 `web.build` 的现有 direct-edit 页面重放 G2-01：保持 textarea 聚焦，分别 Enter 接受和 Escape 取消，监听 panic/Console，确认 lease 和 focus 清理。附加外部点击触发 blur，验证不同路径。
2. `platform.p2-e02-text-input` 先取其中 `test.platform-web-p2-e02` 与 `test.platform-winit-p2-e02` 做 adapter 定向验证。现有 tests 只覆盖基础 helper，不应期待它们自动捕获 G2-01/G2-05/G2-06；需要主 agent 后续补充具体序列断言。
3. G2-05 最小状态序列为 compositionend("中") -> input("中") -> input("中a")，同时断言 host value 每步清空；G2-06 用合法 session + KeyQ/"@"/Alt 输入验证，并补一个 Windows 德语布局人工样本。不要用“host 上 Rust 测试通过”替代 Windows 实测。
4. G2-02 使用 `m8.viewport-scroll-zoom` 的 pane fixture 连接 Web adapter，验证正 DOM delta 导致 origin 正向增加；覆盖 Pixel/Line/Page、双轴和 Ctrl-wheel 不回归。现有 adapter 单测只检查数值传递，未覆盖方向闭环。
5. G2-03 在 Web Widgets 内测试左/中/右键：仅左键能增加 action revision/改变 selected。
6. G2-04 不止查 AX tree 节点数，应在生成的语义 button 上执行默认动作和 Focus，再检查 Core action revision、selected、focus owner；与 `m10_accessibility_contract::focus_and_default_actions_reuse_runtime_widget_transactions` 的 Core 断言区分。
7. 完成定向修改后再执行完整 `platform.p2-e02-text-input`、受影响的 Web 构建/手工平台切片。Windows/Linux 发布资格单独记录环境、GPU、X11/Wayland、DPI、布局/IME；没有证据就保持待验证，不填 FAIL。

## 8. 实际阅读清单

“全文”指完整该文件；“定向”指完整相关方法及直接调用方，不声称逐行穷尽全仓。

### 规则与规范（全文）

- AGENTS.md、CLAUDE.md；用户/项目 memory；bits-code-guard SKILL、general-workflow、review-dimensions、review-rule。
- 本 WORK_DIR 的 review_files.md、review_groups.md；doc/00-index.md。
- doc/design/architecture/overview.md、static-architecture.md、tooltip-accessibility.md。
- doc/design/input/scroll-zoom-gesture-contract.md。
- doc/design/editor/p2-direct-text-edit.md。
- doc/adr/adr-023-crate-consolidation-and-extension-boundaries.md。
- doc/verification/manual/web-platform.md、m10-tooltip-accessibility.md、p2-direct-text-edit.md。
- doc/verification/reviews/p2-text-input-bridge-2026-09-29.md、m10.5-tooltip-accessibility-2026-09-10.md、draw2d-core-1.0-final-audit-2026-09-13.md。

### 实现（全文）

- novadraw-platform-winit/{Cargo.toml,src/lib.rs,src/host.rs,src/input.rs,src/text_input.rs}。
- novadraw-platform-web/{Cargo.toml,src/lib.rs,src/host.rs,src/input.rs,src/text_input.rs,src/dom_text_input.rs}。
- novadraw/src/host/{mod.rs,scene_host.rs}、novadraw/src/event.rs、novadraw/src/runtime/event/mod.rs、novadraw/src/runtime/accessibility.rs。
- novadraw-editor/src/text_input.rs（只审跨组协议）。
- examples/web/web-validation/src/direct_edit_mode.rs（启动时未跟踪）。
- examples/support/{Cargo.toml,src/lib.rs,src/prelude.rs}；novadraw/Cargo.toml。
- scripts/{build_web_validation,check_facade_dependencies,check_public_api_dependencies,check_accessibility}.sh（只读，未执行）。

### 实现/验证（定向）

- examples/support/src/app.rs：构造/scene switch、输入分发、效果同步、render 组合、surface/suspend/resume、wake、Tooltip lowering；未读截图工具全部细节。
- examples/web/web-validation/src/lib.rs：580-861、920-1205、1300-1415，覆盖宿主、场景、输入、Tooltip、accessibility、注册入口；未深入 Canvas2D renderer。
- examples/native/node-editor-demo/src/main.rs：1570-2065 的平台壳；相关 dirty diff。harness.rs：时间/IME 桥接 diff。未深入模型策略和 Editor core。
- novadraw/src/runtime/runtime.rs：880-1010 的 host-facing 方法；runtime/context.rs：395-425、550-655；container/scroll_pane.rs：180-220、240-304；figure/widget.rs：150-215。
- novadraw-editor/src/domain.rs：340-402 的 session-tagged 入口；direct_edit.rs：345-418 的输入语义；viewer/mod.rs：Release 发布位置。只为核对桥接，不审内部算法。
- novadraw/src/lib.rs：1-140；Cargo workspace/platform/web-validation manifests 的相关依赖。
- verification/suites.toml：平台/IME/Web/M8 套件与命令、平台枚举检索；Core M6/M8/M10 契约测试名和相关 action/scroll 断言入口。
- examples/native/*/src/main.rs：入口索引检索，确认大多数 demo 走 example-support；独立 vello-app 为后端演示，不宣称逐行审过其 renderer。
- 本地依赖 `/Users/bytedance/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.12/`：src/event.rs 的 Ime/KeyEvent/MouseScrollDelta；Windows keyboard.rs:210-260；macOS view.rs:448-491；Web event.rs 的 delta 符号。

本组只写本文件与 group_2.jsonl。现有 suite/验收 PASS 均为引用历史记录，本轮没有产生新的运行 PASS/FAIL。

## 9. 产物自检

- JSONL：6 条；必需字段、gitroot 相对路径、源码行号范围、产物尾随空白检查通过。
- `git diff --check`：通过。该命令不覆盖未跟踪产物，产物文本另由上项检查。
- 结束时 HEAD：`841be423815833d97c19862da7b1b0a8ad05591f`。源码提交来自并行工作流，本组未执行任何 git 写操作。
- 未运行 Cargo 或浏览器；产物格式校验不是产品功能验证。
