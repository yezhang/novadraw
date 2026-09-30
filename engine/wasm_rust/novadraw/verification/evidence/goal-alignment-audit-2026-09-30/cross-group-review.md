# 独立跨组复核

类型：`verification`

日期：2026-09-30。读取末轮 HEAD：`841be423815833d97c19862da7b1b0a8ad05591f`。

## 1. 范围与证据口径

- PROJECT_ROOT：`/Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw`；下文源码路径相对此处。
- REPO_ROOT：`/Users/bytedance/Documents/code/GitHub/drawjs`。分组 JSONL 的路径相对此处，不能直接与项目相对路径拼接。
- WORK_DIR：`verification/evidence/goal-alignment-audit-2026-09-30`，ARTIFACTS_DIR 同此目录；目录已存在。
- SKILL_ROOT：`/Users/bytedance/.trae-cn/skills/bits-code-guard`。
- 已按 AGENTS → CLAUDE 读取，补读用户/项目 memory；读取 `review_files.md` 范围头、`review_groups.md`、四组全部 md/jsonl。以现行契约复核现存实现，允许扫描代码，非理想架构设计，非最近 commit diff 审查。
- 本次执行 bits-code-guard 的独立跨组校验，完整读取 SKILL、general-workflow、review-dimensions、review-rule。Rust 无专项规则；不新增分组、不委派 agent，不覆盖主流程 comments/final_comments/report。
- 七维度均作适用性检查：逻辑、语义、健壮性、同步重入、性能及测试可靠性有下列结论；所读路径没有新增可证实安全漏洞；纯风格不报告。单线程 RefCell 重入不描述为跨线程竞争。
- 只读静态源码与既有证据；没有运行 Cargo、浏览器、布局/paint 探针，没有修改源码。`rustc --print sysroot` 仅用于定位本机语言文档，不是编译。既有 import probe 结果是引用，不是本轮执行。
- 第三方框架语义仍限定 Draw2D/GEF，不使用 Zest。本轮额外读取锁定 Winit 0.30.12 的平台事件实现，以核对平台输入契约。

## 2. 处置总表

共复核 13 条组级 JSONL 候选。没有 P0。置信度与严重度分别表示证据充分程度、修复优先级；静态确认不等于本轮运行复现。

| 原编号 | 独立结论 | 建议级别 / 置信度 | 处置 |
| --- | --- | --- | --- |
| G1-01 | Builder invalid 状态未交接到 Runtime validation 队列 | P1 / 10 | 保留；等待主 agent 探针补证，不重复运行 |
| G1-02 | 缓存命中前逐 Label/TextFlow 扫祖先，工作量为 sum(depth) | P2 / 9 | 从 P1 保守降级；确定冗余工作，但未证明实际帧时或退化量级 |
| G1-03 | 无分组路由仍逐边复制全部兄弟 | P2 / 10 | 保留，性能专项 |
| G1-04 | 首帧 Full / damage promotion 再次录制完整树 | P2 / 10 | 保留；等待主 agent 计数探针，不重复运行 |
| G2-01 | 成功分支持有 bridge RefMut 时调用 blur | P1 / 7 | 保留，缩小影响表述；外层 app 不另计缺陷 |
| G2-02 | DOM 滚动方向未转为 Core 内容位移方向 | P1 / 10 | 保留 |
| G2-03 | 示例把非左键映射成 Left | P1 / 10 | 保留，示例接入缺陷台账，不误写为 Core 按钮协议错误 |
| G2-04 | Web 语义 DOM 无 action/focus 回流 | P1 / 9 | 保留为集成缺陷，不扩大为 Core/全部原生 accessibility 失效 |
| G2-05 | 去重分支无 ClearValue，但原报 DOM 值恢复序列未获证 | 条件候选 P1 / 6 | 降信，需结合业务场景确认；不进入最终确定五项 |
| G2-06 | Windows AltGr 会携带 Alt 的前提与 Winit 实现冲突 | 不保留 | 撤回所报 Windows 德语 AltGr+Q 路径 |
| G3-01 | Policy command/feedback 缺少 fault guard 与入口拒绝 | P1 / 9 | 保留 |
| G3-02 | LayoutOutput 仅预检 child 身份，不检验 bounds 数值 | P2 / 10 | 保留为扩展输出防御不足 |
| G4-01 | 合并 compile_fail 不能验证每个禁止导出名 | P1 / 10 | 保留为测试可靠性缺陷，不声称绕过 Runtime mutation |

G1-02 的降级不是否定 O(N²)：复杂度有源码证据，不能由此直接写出“现有生产交互已经卡顿”或“慢于 Draw2D”。其优化在架构性能台账中仍应优先。

## 3. 五个重点误报候选

### G1-01：保留，排除“构造器已隐式补验证”

因果链：

1. `novadraw/src/graph/mod.rs:710-718,4248-4262`：Builder 安装 LayoutManager 后只调用 `mark_validation_path_invalid_for`。
2. `novadraw/src/runtime/runtime.rs:753-793`：接管 tree 后新建 UpdateManager；`runtime/update/deferred.rs:170-178` 明确初始化 `invalid_figures = Vec::new()`、`update_queued = false`。
3. `runtime/runtime.rs:4313-4337`：初始化 layer registry 不导入 invalid；activation 只转调 `graph/mod.rs:939-948` 的 `on_attached`，没有通用入队动作。即使识别到 LayeredPane，安装 StackLayout 也仅修改 tree。
4. `runtime/runtime.rs:4570-4577`：Layout 阶段只依据 `has_pending_layout()`；该查询仅检查 invalid 队列，见 `runtime/update/deferred.rs:441-443`。`runtime/runtime.rs:4622-4626` 不扫描树 validity 就发布 stable epoch。
5. `runtime/runtime.rs:4676-4701` 的 surface_changed 只设 Full，不触发布局；`graph/mod.rs:2576-2591` 的 render 只录制，不执行 validation。

正常触发为无文本/图像/特殊 border 的 Rectangle 父子树，父 100×100、子 10×10，Builder 安装 StackLayout 后直接交 Runtime。显式 Builder validation 是可调用操作，不是 `Runtime::new` 文档前置条件。`doc/adr/adr-018-runtime-driving-and-measurement-api.md` §1 和 `doc/design/rendering/update-manager.md:35-39,77-101` 要求 Runtime 统一驱动、validation 先于 recording。

文本 intrinsic 更新、宿主 resize 或应用手动 revalidate 可以掩盖问题，不构成通用交接。保留 G1 原报的正常输入路径；本轮没有生成或运行该探针。

### G2-01：借用成立，外层 app 重入不能重复算

`novadraw-platform-web/src/dom_text_input.rs:205-230` 的关键表达式：

```rust
if let Some(action) = self.bridge.borrow_mut().apply_effect(effect) {
    // Release -> apply_action -> input.blur()
}
```

Rust 2024 **只在匹配失败进入 else 前提前释放**，成功分支仍持有 scrutinee 临时值直到 then-block 完成。本机官方 Edition Guide `share/doc/rust/html/edition-guide/rust-2024/temporary-if-let-scope.html:186,209-220` 明确此规则；完整路径根为 `/Users/bytedance/.rustup/toolchains/stable-aarch64-apple-darwin`。不能把 tail-expression 生命周期缩短规则套用成“进入成功分支前 RefMut 已 drop”。

仍聚焦 textarea 的 Enter/非 composing Escape，经 `dom_text_input.rs:134-154` → `examples/web/web-validation/src/direct_edit_mode.rs:351-359,552-570` → Editor Accept/Cancel → Release → `dom_text_input.rs:293-296` 调用 blur。焦点失去同步分发 blur 时，`:170-175` 先重新 `bridge.borrow_mut()`，因此冲突发生在 `focus_lost()` 检查 active 之前。

边界与修正：

- `text_input.rs:80-86` 在返回 Release action 前已令 active=None。**若先释放 bridge 借用，再执行 blur**，`focus_lost` 在 `:179-187` 返回 None，blur listener 不调用 event_ready；所以不能声称相同 Release 路径修好 bridge 后必然继续撞上外层 app RefCell。
- 外层 app 确实在 `direct_edit_mode.rs:356` 或 `:757-759` 持有 RefMut，但“存在外层借用”不等于“有第二次回调”。其他 focus/Acquire 嵌套序列需要独立证据，不另报一项。
- 外部移焦先发生、再由 blur 处理 Release，与 textarea 仍聚焦时主动 Release 不同；不能拿前者通过排除后者。
- 本轮未验证浏览器 focus 状态和异常传播。重新评分：静态借用链为 10，部分依赖浏览器焦点与同步事件条件扣 3，最终 7，低于组2原报的 9；报告为聚焦释放路径中的同步借用冲突，不写“所有退出必崩”或“已浏览器复现”。

建议在独立语句取得 owned action，确保 bridge RefMut 释放后再执行 DOM effect；不能仅换成 match，match scrutinee 也保留临时值。

### G2-02：保留，区分内容位移与视口 origin

- `examples/web/web-validation/src/lib.rs:958-991` 原样传 `event.delta_x/y()`，Scroll 分支直接送 Runtime。
- `novadraw-platform-web/src/input.rs:145-158` Pixel/Line/Page 均保留符号。
- `novadraw/src/container/scroll_pane.rs:193-203` 以 `old - distance` 更新 range；DOM 向下 `deltaY=+100` 会减小 origin，而不是向下增加 origin。顶部会被 clamp，看起来“不滚动”。
- 本地 Winit 0.30.12 `src/event.rs:953-975` 明确正值代表内容向右/下移动；其 `src/platform_impl/web/web_sys/event.rs:150-151` 显式对 DOM delta 取负。该转换不是“自然滚动设置不同”的猜测。
- Ctrl-wheel 的 zoom 在 `input.rs:105-126` 独立计算 `exp(-delta * sensitivity)`；修复只对 Scroll 转号，不能把函数所有输入统一取负。

外部依赖源码根：`/Users/bytedance/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.12`。

### G2-05：降信，不能把 input 事件等同于写回 value

局部状态机缺口成立：`novadraw-platform-web/src/text_input.rs:137-147` 匹配 `suppress_input` 就返回 None；`dom_text_input.rs:156-168` 只有 Some 才执行 ClearValue。若 input 时 DOM 确实含 `"中"`，吞掉事件不会清空它，后续 `"中a"` 会作为整段 insertion 送出。

但原报遗漏了必要的浏览器因果前提：

- `composition_ended` 返回 ClearValue，`dom_text_input.rs:100-108` 当场执行 `set_value("")`。
- 尾随 input 监听读取的是 **回调时当前 `input.value()`**，不是 CompositionEvent.data / InputEvent.data 的历史 `"中"`。
- 所以“compositionend('中') → 尾随 input”不自动推出 host_value='中'。还需证实浏览器在 clear 之后重新写回已提交文本。若当前 value 已为空，原报的滞留及重复插入链不成立。
- 一个手工传入 `bridge.input("中")` 的测试只能证明条件性状态机缺口，不能证明真实 DOM 输入法采用该序列。

结论：按外部契约依赖降信规则，9→6，需结合业务场景确认。保留待验证风险，不写成 Chrome/所有 IME 的确定重复插入 bug。后续需要记录 composition/input 的顺序、inputType/data、回调前后 value、ClearValue 执行时刻。`suppress_input` 在缺少尾随事件时可能吞掉下一次同文插入，也是独立待证序列，不用它替换原报证据。

### G2-06：撤回所报 Windows 路径

组2仅核验了 text 生成，未核验 modifier 的输出归一化。锁定 Winit 0.30.12 存在直接反证：

```rust
let filter_out_altgr = layout.has_alt_graph && key_pressed(VK_RMENU);
mods.set(ModifiersState::CONTROL, key_pressed(VK_CONTROL) && !filter_out_altgr);
mods.set(ModifiersState::ALT, key_pressed(VK_MENU) && !filter_out_altgr);
```

位置：`src/platform_impl/windows/keyboard_layout.rs:275-283` 的 `get_agnostic_mods`。`event_loop.rs:999-1020` 用它构造 `ModifiersChanged`；`:1123-1127` 明确在按键事件前发送 modifier 更新。项目 `examples/native/node-editor-demo/src/main.rs:1901-1909` 读取该归一化状态，`:1925-1930` 原样交 bridge。

因此正常识别 AltGr 的德国布局下，右 Alt 不会作为原报假设的 `modifiers.alt=true` 到达 `text_input.rs:171`；不能用手工 `KeyQ + "@" + alt=true` 证明此平台路径。`keyboard.rs:232-248` 的 text 处理不推翻这个反证。

撤回“Windows AltGr+Q 必丢字符”的 P1。其他系统的 Option 字符、Linux AltGr、布局识别失败、左 Ctrl+Alt 等不在该反证覆盖内，也未得到完整新触发链，只留键盘布局兼容性风险，不泛化为所有 Alt 文本都安全。

## 4. 跨组冲突与同根因

| 主题 | 复核证据与裁决 |
| --- | --- |
| Builder / Runtime / Query 权威 | G1-01 是构建状态交接遗漏，不是要求 Builder 承担帧发布；G3 的 scoped editor 结论与其不冲突。`Runtime::tree` 仅返回 `&FigureTree`（runtime.rs:799-801），builder 要求 `&mut self`（graph/mod.rs:1027）；不能因 root 导出 FigureNode 就推断应用拿到 attached 可变树。 |
| API 导出与 compile_fail | `novadraw/src/lib.rs:93-98,115-130` 的低层重导出违反 ADR-021 保留的三层表面；ADR-023 只替代 package 聚合，并未取消此原则。README:38-40 把三种禁止名放同一 compile_fail，只要 RenderCommand 失败即整体通过。`public-import-probes.json` 的既有结果与现行 pub use 一致。合并为一个“负向编译门禁掩盖导出边界回归”条目，不再复制为运行时所有权漏洞。 |
| Policy fault 与组件 guard | `viewer/mod.rs:2089-2151` 直接调用可变 policy；`:2015-2025` 的 validate_selectable 和 `:2474-2484` 的 policy_host 都不检查 fault。`domain.rs:418-433` 的 CommandStack 在 policy 返回后才运行，不能保护前一段 panic。`:2486-2495` 的 refresh guard 是另一入口，不构成覆盖。与 G3 确認的 Core component guard 不矛盾。 |
| LayoutOutput 校验 | `graph/mod.rs:2236-2373` 两条提交路径共用只检查 child 身份的预检，Bounds 数值被丢弃。与 route 输出预检完备不能互相替代。没有正常内置 layout 产生 NaN/负尺寸的证据，维持防御不足 P2，不升成全局布局崩溃。 |
| 文本扩展与文本性能 | G3 证明 TextLayoutEngine 可注入、可返回非空快照，不意味着内置 Label 调度为 O(dirty)。`graph/mod.rs:3556-3570,3585-3611,3636-3662` 在缓存检查前逐个解析祖先；Label 缓存检查见 `figure/label.rs:208-228`。这是 G1-02 的确定工作量问题。闭集 capability 和外部 typed derived snapshot 则是另一类扩展覆盖差距，不合并成“文本不可扩展”。 |
| 路由批处理与 O(E²) | G3 的 route batch 原子性、单组障碍 snapshot 复用，与 G1-03 不冲突。`runtime.rs:1424-1432` 每次构造 routing_order；`connection/runtime.rs:958-969` 对 None scope 直接返回单连接。前者按兄弟集合复制，后者只算单边，边数 E 时至少 E×E 的 ID 处理。此前 O(E) 投影同步结论不是此路由路径的证据。 |
| 重复 Full 录制与合法 Full 策略 | `runtime.rs:4698-4718` 在 Full 标记仍为 true 时再次 render；`runtime/update/deferred.rs:575-577` 的 Partial repair 也已完整录制。G1-04 是同一次 submission 的重复工作。`update-manager.md` §7/8 允许 Partial 全量 recording；“尚无局部 CPU recording”本身是已接受策略，不是第二条 bug。 |
| 平台可复用边界 | G2 的 DOM/调度留在 examples 与 G3 的六包依赖正确可以同时成立。包图正确不代表所有可复用适配逻辑都提取完毕；composition root 持有 Runtime/backend 是合法组合，不能把整个示例搬到 Core。 |
| Accessibility | `lib.rs:1082-1175` 生成带 role 的 DOM，没有 action/focus 绑定。现有 Core action API 不会自动接通 DOM。G2-04 保留为 Web 集成缺陷；完整原生 provider 后置、Windows/Linux 缺运行证据是不同类别，不能合并成“全部平台无障碍失败”。 |
| 通用更新可达性 | G3 R2 的 VisualUpdateContext / callback mutation 闭集没有任意外部 component update，与 Core 直接 FigureEditor update_component 可用不冲突。属于入口覆盖/目标闭环差距，不以“外部 Figure 完全不能更新”替代精确结论。 |

性能项同属“每帧实际工作量与 dirty/缓存命中脱节”，但不是一个可用单点修复解决的根因：G1-02 为样式祖先解析，G1-03 为 routing_order 构造，G1-04 为 recording 决策顺序。风险台账应共享性能验证计划、保留各自位置，不机械去重为一条，也不将阶段位图差距再复制成第四个功能缺陷。

已接受限制与长期目标另账：dispose/rebuild 生命周期变体、strict topology、完整 Native provider 后置、partial 全量录制、typed derived component 后续协议，不报当前执行错误。它们不因此等于 Draw2D 全面对等。Group4 的方法表分母、Draw2D graph layout 本体、剩余 partial/deferred 无退出条件、Windows/Linux 发布证据不足、无同场景 Draw2D 性能对照，都应进入架构目标台账，不包装为代码 P0/P1。

## 5. 最终建议优先五项

本表是面向“全项目架构审计”的五项选集，再按 severity / confidence 排序；不是把所有 P1 无条件截断。G2-03 虽置信度高，但局限示例按钮接线；G2-04 属 Web 集成闭环。二者仍保留缺陷台账，不因未入选五项而视为解决。性能问题另列专项，不用未测延迟挤占确定的语义/失效边界问题。

| 顺位 | 建议条目 | 等级 / 置信度 | 主位置 | 最小修复方向 |
| --- | --- | --- | --- | --- |
| 1 | G1-01：Runtime 首次发布遗漏 Builder invalid 布局 | P1 / 10 | `novadraw/src/runtime/runtime.rs:766-793` | 接管树时将初始 validation 纳入 Runtime 事务；不依赖宿主补 mutation |
| 2 | G4-01：合并 compile_fail 掩盖禁止 root 导出的回归 | P1 / 10 | `novadraw/README.md:38-40` | 按每个禁止名独立负向编译断言，并按 ADR-021 收窄 root；不改变 Runtime 所有权 |
| 3 | G2-02：Web 普通滚轮方向错误 | P1 / 10 | `novadraw-platform-web/src/input.rs:145-158` | Scroll 方向在平台入口统一归一化，保留 Ctrl-wheel zoom 符号 |
| 4 | G3-01：Policy 回调绕过 Viewer fault 边界 | P1 / 9 | `novadraw-editor/src/viewer/mod.rs:2102-2105` | 入口 ensure_ready，扩展 panic 标记 fault 后传播，覆盖 command/feedback |
| 5 | G2-01：DOM Release 的借用跨越同步 blur | P1 / 7 | `novadraw-platform-web/src/dom_text_input.rs:205-230` | 先释放 bridge RefMut，再执行 DOM action；聚焦 Enter/Escape 后续定向验证 |

对应缺陷代码锚点：

```rust
// G1-01: 新建空队列，未导入 Builder 的 invalid 状态
updates: UpdateManager::with_namespace(namespace),
// G2-02: DOM 值仅换单位，未换方向
WebWheelDeltaMode::Pixel => (delta_x, delta_y, ScrollDeltaKind::LogicalPixels),
// G3-01: 直接进入扩展，外部 CommandStack guard 尚未运行
policy.command(host, request, &self.model)?
// G2-01: action 虽 owned，scrutinee RefMut 仍活到成功分支结束
if let Some(action) = self.bridge.borrow_mut().apply_effect(effect) {
    // DOM effect
}
```

G4-01 的问题片段为 README 的 `use novadraw::{FigureNode, RenderCommand, UpdateManager};`，位于单个 `compile_fail` 块；它测试的是“至少一个名字不可导入”，而非“每个名字均不可导入”。此项按规则的测试可靠性例外列 P1，不将一般文档漂移升级为 P1。

## 6. 交付与后续验证边界

- 本次只新增本文件，不改其他组结论或 JSONL。没有新独立根因需要追加 JSONL；反证、降信与最终选五项属于对原条目的裁决，主 agent 合并时应替换/排除原条目，不能把本报告当成五个额外缺陷重复累加。
- 主 agent 的构建期布局/重复 paint 探针结果应补到 G1-01/G1-04；本报告不预填 PASS/FAIL。
- 最高价值的平台补证是仍聚焦 textarea 的主动 Release 和真实 composition 前后 value 轨迹，不需要先扩成全量浏览器/平台矩阵。
- policy panic 注入、每个 root 禁止名独立 compile-fail、LayoutOutput 非法几何整批拒绝属于后续定向验证建议，本轮未运行。
- 本轮产物自检仅检查文本/引用位置与空白；`git diff --check` 无诊断。未跟踪报告另用 `git diff --no-index --check /dev/null <report>` 检查，无空白诊断（返回 1 表示存在新增文件差异）。不把产物校验表述为功能验证或全项目审查完成。
