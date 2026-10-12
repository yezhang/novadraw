# 7. Damage、绘制与帧提交

> **本章解决的问题**：场景变化后，如何既清除旧像素、生成新内容，又正确管理跨 CPU、
> GPU 和窗口系统的帧生命周期？

绘制正确不等于调用了正确的 path API。节点移动以后，旧位置必须被修复；资源必须属于
当前渲染会话；后端异步完成也不能反向篡改场景。

核心结论是：

> 稳定场景只证明“可以生成这一帧”，后端完成反馈才证明“这一帧的提交生命周期结束”。

## 7.1 Dirty 与 damage

**Dirty** 描述引擎内部哪些事实需要重新计算，例如布局失效、文字排版失效或连接待
路由。

**Damage** 描述逻辑表面上哪些区域的像素可能不再正确。

两者不等价：

- 文本内容变化可能同时产生测量 dirty 和视觉 damage；
- 一个不可见子树可以 dirty，但暂时没有 surface damage；
- 遮挡关系变化可能没有节点几何 dirty，却需要重绘重叠区域。

dirty 驱动稳定化，damage 驱动呈现修复。

## 7.2 旧新视觉范围

节点从 A 移到 B 时，正确 damage 是：

```text
damage = old_projected_visual_bounds
       union new_projected_visual_bounds
```

旧范围用于清除残留像素，新范围用于绘制新内容。visual bounds 必须包含边框、阴影、
装饰和可能溢出的子内容。

如果祖先滚动、缩放或裁剪变化，旧新投影链也不同。Runtime 必须在覆盖状态前保留旧
投影摘要。

## 7.3 Damage 的保守原则

damage 是正确性边界，允许多报，不允许漏报：

```text
所有可能变化的像素 ⊆ damage
```

区域合并可以降低提交数量，但不能通过过度细碎的几何优化冒险漏像素。复杂滤镜或无法
精确界定的效果，应提升到可证明的祖先范围，必要时使用整表面 damage。

全量 damage 是合法降级策略，但不能掩盖场景状态不一致。

## 7.4 稳定场景前不能录制

绘制命令必须读取同一稳定 epoch。若布局只更新了一半，录制结果可能同时包含旧连接和
新节点位置。

帧准备顺序是：

```text
接受 mutation
-> 收敛布局和派生状态
-> 冻结本次稳定场景视图
-> 汇总 damage
-> 录制绘制命令
-> 封装提交
```

“冻结”不一定复制整棵树，但必须保证录制期间结构和相关状态不会被重入修改。

## 7.5 递归绘制

Renderer 按 Figure 树递归执行：

1. 检查可见性和 damage 相交；
2. 保存 Graphics 状态；
3. 应用节点变换；
4. 应用节点裁剪；
5. 绘制节点背景、边框和自身内容；
6. 切换到子内容域；
7. 按 children 顺序绘制；
8. 绘制节点前景或装饰；
9. 恢复 Graphics 状态。

具体阶段可按协议调整，但遍历、状态恢复和 Z-order 必须由统一 Renderer 管理。

## 7.6 Graphics 是命令录制接口

Graphics 提供后端中立的绘制语义：

- 保存与恢复状态；
- 变换和裁剪；
- 填充与描边；
- 图像和文字；
- 透明度与合成；
- 字体指标和测量所需的统一入口。

Figure 调用 Graphics 录制命令，不直接访问 GPU 对象。这样 Figure 行为可以在不同后端
复用，命令也可以被测试、诊断或离线检查。

Graphics 不是场景查询接口。绘制回调不应通过它修改 Figure 树。

### 录制不等于帧出版

独立调用者可以通过命令录制器（`CommandRecorder`）和绘制上下文（`PaintContext`）
生成 `RecordedDrawing`。它只表示一段已经通过绘制语法校验的命令记录，不携带 surface、
backend session 或 frame identity，也不等于可以交给后端的帧。

图形对象、形状和边框使用同一套 `PaintContext` 语言。绘制回调返回结构化
`GraphicsError`；Renderer 可以恢复内部状态栈，但只要任一回调失败，本帧就不能出版。

只有 Runtime 能把录制结果提升为 `RenderSubmission`。出版前，它还必须：

- 确认派生状态已经稳定；
- 冻结重绘区域和资源同步；
- 校验 backend capability 和设备输入；
- 分配 session/frame identity；
- 建立 in-flight 状态。

因此，公开录制器负责“命令是否有效”，Runtime 负责“这一帧能否成为当前会话中的事实”。
把两个边界合并，会允许 Figure 或应用绕过稳定化、资源基线和完成确认。

## 7.7 状态栈

每个节点进入时保存状态，退出时恢复。否则一个 child 的 transform、clip 或 opacity
会泄漏到 sibling。

状态栈必须满足：

- push/pop 平衡；
- 失败路径也能恢复；
- clip 只会进一步约束当前范围；
- 当前变换与坐标服务约定一致；
- 命令录制结束时回到初始深度。

不平衡应在提交前被诊断为录制错误，而不是交给后端猜测。

## 7.8 RenderSubmission

一次提交携带后端完成当前帧所需的全部拥有数据：

```text
RenderSubmission {
    session_id,
    frame_id,
    surface,
    damage,
    commands,
    resources,
}
```

- `session_id` 区分 backend 重建前后的资源宇宙；
- `frame_id` 在 session 内单调标识提交；
- `surface` 描述本次提交使用的逻辑和物理表面信息；
- `damage` 指示需要修复的表面区域；
- `commands` 是后端中立绘制记录；
- `resources` 是当前 session 的完整快照或相对已确认基线的增量。

`stable_epoch` 属于 Runtime 内部因果边界：Runtime 只允许从一个稳定 epoch 生成上述
提交，但后端不需要据此查询场景。后端只消费提交，不获得 Runtime 所有权。

## 7.9 Session 与 Frame

设备丢失、后端切换或其他需要重建 backend resource universe 的故障会开启新 session。
单纯的 surface lost 通常只要求在同一 backend session 中重新配置表面，不应自动递增
session。旧 session 的迟到完成不能被当成新 session 当前帧的结果。

```text
(session 4, frame 120) != (session 5, frame 120)
```

frame identity 也用于：

- 对齐资源使用期；
- 忽略重复完成；
- 记录丢帧和重试；
- 关联截图与诊断证据。

身份必须由 Runtime 或呈现协调器分配，不能由后端自行推测。

## 7.10 资源增量

字体、图像和 glyph 轮廓等资源可能跨帧复用。资源管理需要区分：

- 场景引用仍存在；
- 某帧命令仍引用资源；
- GPU 是否完成该帧；
- 新 session 是否需要重新上传。

Figure 销毁不意味着 GPU 资源可以立即释放。资源生命周期通常要等待所有引用帧完成。
反过来，后端缓存也不能成为文字或图像语义的权威来源。

### 资源来源不属于场景运行时

文件路径、URL、同步文件 I/O 和 Web fetch 属于不同平台语义。Runtime 不保存统一
ResourceProvider，也不主动读取文件或轮询异步任务。

资源进入场景的规范顺序是：

```text
应用选择 Native/Web provider
-> provider 取得 bytes
-> Core 纯解码
-> Runtime 完成资源请求
-> 依赖该资源的图形对象失效
```

每次异步加载都绑定资源请求令牌（`ResourceRequestToken`）。同一资源开始新请求后，
旧令牌的迟到完成必须返回 stale，且不能改变资源状态、revision、待提交资源增量或依赖
图形对象的失效状态。

同步资源修改同样会使活动令牌失效。令牌保护的是 Runtime 内的发布顺序，不承诺取消
平台 I/O。provider 读取错误、字节解码错误和 Runtime 资源 mutation 错误也属于不同
失败域，不能压缩成一个字符串。

## 7.11 提交状态机

```mermaid
flowchart TD
    Stable[稳定场景] --> Prepare[Runtime 准备 RenderSubmission]
    Prepare --> Host[Host 取得提交]
    Host --> Backend[RenderBackend::submit]
    Backend --> Result{RenderResult}

    Result -->|Presented| Presented[已呈现]
    Result -->|Skipped / Retry| Deferred[未呈现，可继续调度]
    Result -->|SurfaceLost| Surface[表面失效]
    Result -->|DeviceLost| Device[设备失效]
    Result -->|Unsupported / InvalidInput| Rejected[确定性拒绝]
    Result -->|OutOfMemory / Fatal| Terminated[终止自动提交]

    Presented --> Return[Host 回传结果]
    Deferred --> Return
    Surface --> Return
    Device --> Return
    Rejected --> Return
    Terminated --> Return
    Return --> Complete[Runtime::complete_submission]

    Complete --> Ack{SubmissionAcknowledgement}
    Ack --> Baseline[提交资源基线]
    Ack --> Preserve[恢复资源并保留 Full]
    Ack --> Reconfigure[同 session 重配 surface]
    Ack --> Rebuild[递增 session 并重建 backend]
    Ack --> Stop[停止自动提交并暴露失败]
```

`RenderResult` 把正常处置与运行失败分开：

```text
RenderDisposition = Presented | Skipped | Retry

RenderFailure =
    Unsupported | InvalidInput
  | SurfaceLost | DeviceLost
  | OutOfMemory | Fatal
```

Backend 负责分类提交结果；Runtime 负责确认资源和 session 状态；Host 只按照
acknowledgement 执行重新调度、surface reconfigure、backend rebuild 或停止提交。
三者不能互相越权。

## 7.12 完整案例：移动带边框节点

节点原本在 A，具有向外扩张 2 像素的边框，现移动到 B。

1. Runtime 在 mutation 前读取 A 的旧 visual projection；
2. 原子更新 bounds；
3. 布局、连接和相关容器进入失效集合；
4. 稳定化得到最终 B 和新连接路径；
5. Runtime 计算 A 外扩范围与 B 外扩范围的 union；
6. Renderer 以该 damage 为裁剪提示遍历稳定树；
7. 背景或下层内容先修复 A；
8. 节点及其边框在 B 录制；
9. 命令、资源和 damage 封装为新 frame；
10. 后端返回完整 `RenderResult`；
11. Host 把结果交回 Runtime；
12. Runtime 确认资源基线、完整重绘需求和 session 状态；
13. Host 根据 acknowledgement 决定是否继续调度。

如果 B 被其他节点遮挡，damage 仍需覆盖 A；如果节点移动到表面外，旧 A 仍必须修复。

## 7.13 后端边界

后端可以：

- 校验或降低 Render IR；
- 管理设备资源；
- 执行局部或全量呈现；
- 返回 Presented、Skipped、Retry 或结构化 RenderFailure；
- 保存后端本地原始错误用于诊断。

后端不能：

- 修改 Figure bounds；
- 决定业务模型状态；
- 改写 damage 的场景语义；
- 因为优化方便而重排有可见语义的命令；
- 把后端专有类型暴露回 Core 公共协议。

失败诊断必须保持 backend-neutral。Core 可以接收稳定摘要和 source chain 文本，但不能
把 Vello、WGPU 或平台错误类型放进公共签名，也不能通过字符串匹配决定失败分类。

## 7.14 部分重绘与完整场景

damage 可以限制需要修复的区域，但命令生成仍应保持完整场景语义。实现可以裁剪遍历，
也可以录制完整命令后由后端裁剪，只要结果等价。

局部重绘最危险的假设是“只画发生变化的节点”。旧区域需要重建其下方所有内容，透明、
裁剪和叠放也可能要求绘制未变化对象。

## 7.15 失败与恢复

### 录制失败

尚未提交后端。场景仍稳定，但本帧未生成。错误应携带失败节点或资源上下文。

### Skipped 与 Retry

二者都表示当前帧未发布。Runtime 恢复 in-flight 资源并保留完整重绘需求。Host 可以
根据可见性和 wakeup 规则再次请求 redraw，但动画时间和场景状态不回退。

### SurfaceLost

表面获取或配置失效时，Runtime 保持当前 backend session，恢复资源并保留完整重绘。
Host 让现有 backend 重新配置 surface，成功后再请求帧。

### DeviceLost

场景事实保持，但 backend 资源宇宙失效。Runtime 是唯一递增 session 的权威，并要求
下一帧发送资源快照和完整重绘。Host 重建 backend，但不能再次擅自重置 session。

### Unsupported 与 InvalidInput

这是确定性拒绝，不能自动循环重试。Runtime 恢复资源和完整重绘需求，Host 停止自动
提交并把失败暴露给应用。

### OutOfMemory 与 Fatal

Runtime 终止当前自动提交链路。OOM 不自动重试，避免无界失败循环；应用可以选择释放
资源、重建更高层宿主或结束会话。

### Completion 身份不匹配

无 in-flight frame、session/frame 不匹配、Runtime faulted 或 session ID 耗尽都属于
结构化 completion error。它们不能折叠为一个布尔值，也不能提交错误资源基线。

## 7.16 必须保持的不变量

1. 最终命令只读取稳定场景；
2. damage 保守覆盖所有旧新像素影响；
3. 绘制顺序来自 Figure 树；
4. Graphics 状态栈在任何路径都平衡；
5. 资源与 session/frame identity 绑定；
6. 后端结果不反向成为场景事实；
7. 只有 Runtime 可以确认资源基线和递增 backend session；
8. 未呈现或失败的提交保留完整重绘需求；
9. 确定性拒绝和终止失败不得自动循环重试；
10. 录制结果不携带帧身份，只有 Runtime 可以出版提交；
11. 平台 provider 不进入 Runtime，异步资源完成必须验证请求令牌。

## 7.17 常见错误设计

### 只 damage 新 bounds

旧位置留下残影。

### Figure 直接调用后端

后端类型穿透 Core，命令无法统一验证或回放。

### 应用把录制结果直接包装成提交

应用绕过场景稳定化、重绘区域、资源基线和 session/frame identity，形成第二个帧出版
权威。

### 绘制中修改 Figure

录制读取的快照不再稳定，后续节点看到不同 epoch。

### 后端失败就回滚模型

把呈现故障错误地升级为业务事务失败。

### Host 根据 RenderFailure 自行修改 session

Runtime 与 Host 对资源宇宙产生两个权威，迟到 completion 无法正确拒绝。

### 把所有失败都当作 Retry

Unsupported、InvalidInput、OOM 和 Fatal 会形成无界提交循环，并掩盖确定性错误。

### Runtime 直接读取文件或 URL

平台 I/O 穿透 Core，Native/Web 形成不同公共表面；迟到完成也无法判断是否仍属于当前
资源请求。

### 以“变化节点列表”代替 damage

透明和叠放要求重建区域内容，不等于重画变化对象。

## 7.18 与后续章节的关系

本章从稳定场景走到像素提交。下一章在 committed scene 与 RenderSubmission 之间加入
可丢弃的 Presentation Plane，解释动画如何复用同一 damage、资源和帧合同而不污染源
状态。第 9 章再从平台事件反向进入场景，第 10 章展示视口和连接如何同时产生派生几何
与 presentation 变化。

## 7.19 思考题

1. 为什么 dirty 集合不能直接作为后端的 damage？
2. 节点完全移出视口后，为什么仍可能需要一帧提交？
3. Skipped、Retry 和 SurfaceLost 为什么都要保留完整重绘，但恢复动作不同？
4. DeviceLost 后为什么只有 Runtime 可以递增 session？
5. Unsupported 为什么不能自动当作 Retry？
6. 局部重绘为什么不能只录制发生变化的 Figure？
7. stable epoch、frame ID 和 session ID 分别解决什么问题？
8. 为什么 `RecordedDrawing` 不能直接视为 `RenderSubmission`？
9. 资源请求令牌过期时，哪些状态必须保持不变？
