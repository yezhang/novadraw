# Group 6：Domain / Tool / Request / Policy / Feedback / Auto-expose

类型：`verification`

主审执行。范围为当前全文件语义，不限 diff。先读官方 GEF guide、Tool.java、
Request.java、EditDomain.setActiveTool、AbstractEditPart.getCommand/getTargetEditPart、
TargetingTool.updateTargetUnderMouse，再读项目 architecture 与 G5.5 契约。
Rust 全文读取 domain.rs、request/mod.rs、policy/mod.rs、feedback/mod.rs、
autoexpose.rs，tool/mod.rs 的 Selection/Bendpoint/reconnect release/cancel、
connection create 与更新路径，以及 Viewer command/feedback/connection-plan 调用方。
全文核对 g4_editing_loop_contract.rs；Native keyboard 只核对平台直达 Domain 的边界。
未使用 Zest，未修改产品或现有测试。

## 语义映射

路径简写 `E/` 为 `novadraw-editor/src/`；Java 路径为官方 org.eclipse.gef 包。

| ID | 官方类/方法与契约 | Rust 入口 / 证据 | 状态与边界 |
|---|---|---|---|
| G6-M01 | `EditDomain.setActiveTool:414-423` 停用旧工具、给新工具 domain 并激活 | E/domain.rs:98-128,172-190 | 部分：内置 Selection/Connection/Endpoint/Bendpoint 的切换清理存在；没有可安装的外部 Tool |
| G6-M02 | `Tool` 解释 mouse/key/focus/native drag 等输入；不由平台硬编码业务快捷键 | E/domain.rs pointer_*；apps/native/node-editor-demo/src/main.rs:1687-1728 | 收窄：Domain 无统一 key/focus/cancel 输入协议，demo 自行处理 Delete/N/C/Z/Escape；Figure 键盘优先与 active Tool key routing 尚未闭合 |
| G6-M03 | `SelectionTool.handleButtonDown` 由 Handle 或 EditPart 提供 DragTracker | E/tool/mod.rs:159-204；E/feedback/mod.rs:41-51 | 部分：固定 source/role 的内置 gesture 有效；HandleRole 封闭、无外部 tracker factory，新增交互须改中心分支 |
| G6-M04 | `Request` type/subclass/extendedData 可扩展意图 | E/request/mod.rs:568-633 EditorRequest | typed structs 是合理迁移；封闭六类 enum 不保留开放意图扩展，不应把无 Any map 等同于不许外部 request |
| G6-M05 | `ChangeBoundsRequest` 携带 source、delta、modifier、target | E/request/mod.rs:192-316；E/tool/mod.rs:831-858 | 部分：move/resize、source 顺序和 revision 有效；SelectionTool 未填 target_candidate；reparent 后置 |
| G6-M06 | `EditPolicy` role 可替换/贡献命令、feedback、生命周期 | E/policy/mod.rs:23-48,167-211,338-385 | 保留核心组合；Custom role 开放；当前安装时重复 role 拒绝，未提供 Java live replace/remove 全部公开语义 |
| G6-M07 | `getTargetEditPart` 返回实际操作对象；`TargetingTool:458-473` 使用该身份 | E/viewer/mod.rs:1351-1355,1391-1400 | **缺陷 G6-F1**：返回 ID 只判 is_some，不解析/校验/路由至该对象；不是普通命中与业务目标分离 |
| G6-M08 | `AbstractEditPart.getCommand:486-491` 聚合多个 policy contribution | E/viewer/mod.rs:1330-1369 | 确定性组合存在；source Vec 顺序→BTreeMap role 顺序；显式 PolicyError 与 None 区分 |
| G6-M09 | `Tool` Javadoc 要求执行前清 feedback、恢复临时变化 | E/tool/mod.rs:248-291,432-479,593-646；E/domain.rs:194-229 | 内置正常路径保留；cancel 不执行模型命令，undo/redo 先取消；异常中途 cleanup 需额外故障注入证据 |
| G6-M10 | `DomainEventDispatcher` 与 Figure 控件共存、已消费不再编辑 | E/tool/mod.rs:149-156；E/viewer/mod.rs dispatch_mouse_* | press 仲裁已存在，G3/G4 契约有验证；不能外推全部键盘/原生 drag 生命周期 |
| G6-M11 | gesture tracker 在交互期间固定 source，target 可变化 | E/tool/mod.rs:26-35,310-330；E/domain.rs:302-326 | 内置 gesture 迁移合理；modifier 取 press snapshot，move/release 无新 modifiers，不支持拖动中按 Shift 改约束 |
| G6-M12 | `CreateConnectionRequest` 两阶段、第一阶段固定 source | E/policy/mod.rs:230-289；E/tool/mod.rs:663-794 | ConnectionCreation plan 是可插拔会话，cancel 丢弃、完成给 model-only Command；比把临时 Command 提前压历史明确 |
| G6-M13 | `ReconnectRequest` 固定 connection 和 source/target 端 | E/policy/mod.rs:292-334；E/tool/mod.rs:593-634 | 内置正常路径保留，candidate 每次重算；外部 callback panic 的 Editor fault 属于跨组 G5-F03 |
| G6-M14 | `BendpointRequest` 固定连接、操作与 index；commit 后 undo | E/request/mod.rs:109-179；E/tool/mod.rs:328-479 | typed index/operation 和路由坐标明确；显式 self-loop 折点由模型/策略承担，非 framework 推测形状 |
| G6-M15 | source/target feedback 在独立层，handle 参与 hit、反馈穿透 | E/feedback/mod.rs:78-114；E/viewer/mod.rs:1373-1408 | layer/owner typed 迁移成立；目前 command 和 feedback 共用 source policies，业务 target 重定向同 G6-F1 |
| G6-M16 | `ViewportAutoexposeHelper` 边缘带、重复 step、viewport 变化后更新工具 | E/autoexpose.rs:43-144；E/domain.rs:329-386,513-552 | host 注入 elapsed、50ms clamp、surface 阈值、双轴 clamp 是合理迁移；根 viewport-only 明确收窄 |
| G6-M17 | scroll/zoom 不改模型；fixed surface pointer 产生新 request/feedback | E/domain.rs:329-386；E/tool/mod.rs:220-245 | 常规路径有 G4 自动验证；回到起点阈值内时 refresh 提前返回可能保留旧反馈，另列待验证边界，不在本组定为确认缺陷 |
| G6-M18 | 选择规则/RootEditPart 可定制 | E/selection/mod.rs、E/viewer/mod.rs root layers | selection 数据结构可复用；目标 architecture 列出 SelectionPolicy/RootLayerFactory，当前无这些外部注入出口，不按类型名相近判完成 |

## G6-F1：Policy 的目标重定向未生效

- P1，业务语义，置信度 10/10。
- 位置：`novadraw-editor/src/viewer/mod.rs:1351-1355`；同根因 feedback 在 1391 行。
- 官方契约：`EditPolicy.target` 对应 `getTargetEditPart` 的身份返回；项目 architecture
  第 8 节也写“返回 target part”。`understands` 本身已表达资格，不需要第二个布尔接口。
- 具体输入：外部 policy 挂在模型 2 的 Part，`target()` 返回模型 3 的有效 PartId；
  对模型 2 的 DeleteRequest 执行 command lookup。实际调用仍是模型 2 的 policy，
  host.model()==2，模型 3 的 policy 未收到请求；返回 foreign/stale ID 同样只算 true。
- 本次运行公共 API 探针：`../editor_probe.rs`、`../editor-probe.log`：
  `policy_redirect: requested_target_model=3 actual_command_hosts=[2]`。
- 修复应先定义独立 target resolution，再在目标上聚合 command/feedback；原 source
  请求身份仍保持。目标校验必须处理 foreign/stale，若支持连续 redirect 则需防环。
  不能简单把 source host 换为 target host 后继续调用原 policy（会改变 policy 归属）。
- 此项与新增 Tool/Request 扩展不同：前者是已有公开方法承诺未兑现，后者是范围收窄。

## 额外复现与验证

同一独立 executable 还重现 Group 5 的两项：
`initial_failure: activated=[1] deactivated=[]`；
`projection_panic: caught=true viewer_faulted=false`。
panic 是探针故意触发并在外层捕获，程序退出 0；日志不是构建失败。
探针断言的是缺陷现状，不能在未来产品回归门禁中把这些反向断言保留为正确行为。
未新增或修改 unit tests，链接的是本次 workspace.quality 生成的 rlib，哈希已记录。

G4 合同文件当前 10 个测试已由主审 workspace.quality 执行；G3/G4/G5.2-G5.5 的
headless replay 通过。未执行本次 Native 视觉验收，Web build 有独立失败，G6 未交付。

## Rust 与七维评估

类型化 request/host、command-only 模型写、非全局时钟和单 owner gesture 合理；
有 trait 的 Policy/ConnectionPlan 是真实扩展，有名字但不可注入的 Tool 不是。
`&mut` 顺序协调未发现框架内部并发竞争。没有本组可证实安全入口缺陷。
本组明确逻辑/业务缺陷为 G6-F1；callback fault/资源清理与 Group 5 去重。
feedback 每帧 remove/add，command lookup 为 source 数乘 role 数，外加外部 callback；
性能尚无耗时基准，不凭函数长度或 enum 使用形式报质量缺陷。
外部 request/tool/selection/root 扩展、统一键盘和增量 modifier 属于实施方案中的契约补全。
