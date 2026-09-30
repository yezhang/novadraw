# 代码评审报告

- 仓库：drawjs/novadraw
- 检测模式：目标审计：五项优先代码问题
- 检测范围：841be42 审计快照；问题锚点已复核；5文件/75行非全部阅读范围
- 生成时间：2026-09-30 11:16
- 检查文件：5
- 变更行数：75

## 缺陷统计

- P0：0
- P1：5
- P2：0
- 合计：5

## 缺陷详情

### 1. [P1][逻辑错误] 构建期失效树转交 Runtime 后未进入首次 validation

- 位置：`engine/wasm_rust/novadraw/novadraw/src/runtime/runtime.rs:766-793`
- 置信度：10/10

**问题描述**

Builder 安装布局器只标记树节点 invalid，Runtime::with_text_layout_engine 却创建空 UpdateManager，初始化 activation 不入队；stabilize 仅在 has_pending_layout 时验证。具体输入为 Builder 创建 100x100 Rectangle 根、10x10 Rectangle 子并安装 StackLayout，不调用 validate_subtree，之后 Runtime::new 并 prepare_frame/prepare_submission_state。没有 Label/Image 等刷新时首帧直接录制，子节点仍是 10x10 而非 StackLayout 要求的 100x100，stable_epoch 已晋升。完整调用方与布局实现已静态复核；现有 1024 节点测试手工 revalidate，未覆盖此交接路径。主 agent 最小 probe 已复现：首帧 child=10x10、root_valid=false；显式 revalidate 后 child=100x100。

**修复建议**

Runtime 接管附着树时将初始 validation root 纳入统一更新事务；不能依赖宿主额外 revalidate、resize 或文本刷新才能完成首次布局。

---

### 2. [P1][逻辑错误] Web 滚轮未转换 DOM delta 符号，导致滚动方向反转

- 位置：`engine/wasm_rust/novadraw/novadraw-platform-web/src/input.rs:145-158`
- 置信度：10/10

**问题描述**

examples/web/web-validation/src/lib.rs:958-991 原样传 DOM delta_x/y，本方法只转换量纲。Core container/scroll_pane.rs:193-203 则执行 old - distance。因此在顶部向下滚动 deltaY=+100 会请求负 origin 并 clamp 到0，中段会反向上移。Winit 0.30.12 event.rs:953-975 明确其正值是内容向右/下移动，其 Web event.rs:150-151 对 DOM delta 取负；Native adapter 符号与 Core 相符，当前 Web adapter 不符。

**修复建议**

在 Web scroll adapter 将 DOM 的视口滚动方向转换为 Core 内容位移方向，覆盖 Pixel/Line/Page 与双轴；不要改变独立 Ctrl-wheel zoom 的指数符号，并以实际 viewport origin 断言闭环。

---

### 3. [P1][业务语义问题] 合并的 compile_fail 导入掩盖根层低层 API 泄漏

- 位置：`engine/wasm_rust/novadraw/novadraw/README.md:38-40`
- 置信度：10/10

**问题描述**

ADR-021 保留的三层公开表面和 README 声明低层协议不能从 root 导入，但 lib.rs 仍导出 FigureNode、UpdateManager、PendingMutations、EventDispatcher。此 compile_fail 将 FigureNode、RenderCommand、UpdateManager 合并为一次导入，只因 RenderCommand 不存在就通过，无法验证另外两个名字的不可达性。本次 cargo test -p novadraw --doc 全通过，而独立 rustc 探针确认 FigureNode 和 UpdateManager 均可从 root 导入。该测试会持续漏报边界回归；并非证明这些类型已经提供绕过 Runtime 的可变入口。

**修复建议**

按 ADR-021/023 收窄 root 导出，advanced 从定义模块导出；为每个禁止 root 名称建立独立负向编译用例，另保留领域路径正向导入用例，避免任一未解析符号掩盖其他泄漏。

---

### 4. [P1][健壮性问题] Policy 扩展回调缺少 Viewer panic/fault 保护

- 位置：`engine/wasm_rust/novadraw/novadraw-editor/src/viewer/mod.rs:2109-2112`
- 置信度：9/10

**问题描述**

command_for_request 在完整方法 2089-2121 中直接调用可修改自身状态的 EditPolicy::command，没有 catch/fault guard；resolve_policy_target 2048-2064 同样直接调用扩展。直接调用方 EditorDomain::execute_request（domain.rs:418-433）在该方法返回后才进入 CommandStack，无法由其 guard 覆盖。外部 policy 返回当前 active target、understands=true，command 修改自身状态后 panic，宿主捕获 unwind 后 Viewer.faulted 仍为 false，model_mut 和后续请求仍可继续，违反 Editor architecture.md:345-346 的扩展 panic 后 fault 契约。同类 command/feedback 入口也未检查既有 Viewer fault。此为静态确认的条件性缺陷，未运行故障注入；不主张自动回滚扩展副作用。

**修复建议**

为公开 policy command/feedback 入口统一增加 ensure_ready 与扩展 panic guard；扩展 panic 时先标记 Viewer faulted，再执行适当清理并传播 panic，不能只依赖 CommandStack 的 catch。

---

### 5. [P1][健壮性问题] Web 释放文本输入焦点时在 RefCell 借用内同步触发 blur 重入

- 位置：`engine/wasm_rust/novadraw/novadraw-platform-web/src/dom_text_input.rs:205-230`
- 置信度：7/10

**问题描述**

active textarea 保持聚焦时，keydown 经 event_ready、DirectEditWebApp::on_text_input_ready 和 Editor 产生 Release，再由 sync_text_input_effects 调到本方法。if let scrutinee 中 bridge.borrow_mut() 的临时 RefMut 在成功分支内仍存活，Release 分支通过 apply_action 第293-296行调用 input.blur()；同步 blur listener 第170-175行再次 borrow_mut 同一 bridge，触发 already borrowed panic。即使 Release 已把 active 清为 None，borrow 仍先于 focus_lost 内部 active 检查。该特定聚焦释放路径有静态完整调用链，本次未运行浏览器，不覆盖或否认其他序列的历史人工 PASS。

**修复建议**

先以独立语句获取 owned WebTextInputAction 并释放 bridge RefMut，再执行 focus/blur 等可能同步触发 DOM 回调的操作；定向重放 textarea 仍聚焦时 Enter 接受和 Escape 取消。

---
