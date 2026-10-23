# 共享视图与宿主背景

类型：`proposal`

状态：`design-draft`；待评审，未实施

日期：2026-10-23

范围：官方示例计划 K07；参考契约为 Draw2D，不扫描 Novadraw Rust 实现推导目标架构。
本文件不覆盖现有 [资源协议](resource-lifecycle.md)、
[派生状态收敛](derived-state-convergence.md)和 [坐标契约](../coordinates/coordinate-system.md)。

## 需求来自两个独立消费者

`ThumbnailExample` 的两个窗口观察同一个可变 source。主窗口拖动两个 Rectangle，
overview 保持 source 的宽高比。`parts/Thumbnail.java` 通过 `setSource` 注册
UpdateListener，在 `notifyPainting` 中检查 dirty Figure 是否属于 source 子树；
source 改变或 `deactivate` 时注销监听并释放图像。`getSourceRectangle` 定义捕获区域，
`adjustToAspectRatio` 扣除 Border insets 后确定目标尺寸。分块 GC、SWT 定时器与
macOS 图像缓存修补属于上游栅格实现，不是 Core 的公共协议。

`InheritBackgroundExample` 将同一段多行 Label 放进两个 Canvas。一个 root 绘制
不透明背景，另一个 root 透明，由宿主父 Composite 的渐变背景透出。背景属于宿主，
其位置不随 Figure viewport 滚动。仅在 Figure 树中再画一个渐变，不能证明宿主
背景继承成立。

参考路径位于 `/Users/bytedance/Documents/code/GitHub/gef-classic/`：

- `org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/thumbnail/ThumbnailExample.java`
- `org.eclipse.draw2d/src/org/eclipse/draw2d/parts/Thumbnail.java`
- `org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/swt/InheritBackgroundExample.java`

## 单一源状态与只读视图

source Runtime 独占 Figure 树和 attached-state mutation authority。overview 不接管、
挂载或复制这些活 Figure，也不调用 source 的可变能力。它消费一次稳定化成功后发布
的 owned、不可变 `SceneSnapshot`。宿主持有 source 与 destination，并按序传递快照；
不使用 Singleton、共享可变 Runtime、私有 Figure timer 或后台全树逐帧扫描。

建议公开领域为 `scene`，具体签名在设计批准后按现行 API 边界核对：

| 候选概念 | 所有者与职责 |
|---|---|
| `SceneSourceId` | producer namespace 与 source binding generation；替换 source 后旧更新失效 |
| `SceneSnapshot` | source identity、单调 source epoch、捕获矩形、只读 drawing 和不可变资源闭包 |
| `SceneSubscription` | source Runtime 管理的订阅；宿主获得更新和 dirty 通知，关闭后不再接收候选结果 |
| `SceneViewBinding` | destination Runtime 管理的导入与依赖；移除 view 释放引用和导入资源 |
| `SceneViewPolicyKey<P>` / `SceneViewPolicy` | typed identity + payload trait + registry；决定 source rect、aspect fit、clip 与 refresh 合并策略 |
| `SceneViewFigure` | 普通 Figure 组件，消费 binding；默认 thumbnail 与外部 view 共用同一注册、测量和绘制路径 |

首先只发布已提交场景状态。presentation 动画不写回 source state；如后续 view 需要
观察动画帧，必须显式增加独立版本和选择策略，不让 source epoch 混入 presentation。
此次 Thumbnail 静态拖拽不依赖该扩展。

## 按因果顺序发布

1. source mutation 提交，Core 执行现行 layout、route、text/resource preparation。
2. 稳定化成功后，按订阅捕获矩形生成 candidate drawing 与资源闭包。
3. 校验 candidate 的命令、资源及源身份，再发布一个完整 snapshot；失败返回
   `Result`，不发送半成品或伪造成功 epoch。
4. Host 将 snapshot 传给 destination。整批预检资源、引用和命令后，安装新的 view。
5. destination 计算旧/新可视区域 damage，准备自己的 RenderSubmission。
6. backend 通过 destination 现行 session、资源同步、retry 与确认协议呈现。

快照来自稳定的通用绘制记录。导出不得把 `Figure`、capability payload、LayoutManager
或回调放入 snapshot；消费时不得跨 Runtime 再执行 source 的 paint。具体捕获入口
在实现核对阶段确定，继续保护 `render_recursive.rs` 的主循环。

capture/import 失败保留上一次成功的 view，错误可查询并允许重试。source 被释放、
source binding 被替换或 view 显式关闭时，view 清空并擦除旧区域，不无限保留陈旧图像。
新源还未 Ready 时显示确定性空状态。重复 epoch 可幂等接受；倒序 epoch 或过期
binding 返回明确错误，不发布。计数耗尽也返回错误，不回绕复用身份。

## 资源闭包不能借用 producer cache

snapshot 包含所引用 image/font 的不可变 payload 与 revision；不依赖 producer
RenderSubmission 是否已发送或确认。producer 的 resource ID 只作为 namespaced
来源身份，不能直接进入 destination 的资源注册表或 backend cache。

destination 以来源身份和 revision 去重导入，分配 destination 自己的 typed resource
ID，完整重写 drawing 中的 image/font 引用。导入结果通过现有 ResourceOp 提交。
两个 view 可共享同一导入 revision；引用计数归 destination binding 管理。旧快照、
view 和在途提交的最后引用释放后才可回收。source 更新字体后，不允许新 glyph drawing
引用旧 font revision。source 关闭后，已在途的 destination 提交仍可完成或取消。

跨 backend/device 重建由 destination session 完成同步；source resize 和 target resize
不创建新 backend session。资源导入失败不部分替换当前 view，也不宣称外部副作用可回滚。

## 坐标、裁剪与 damage

捕获矩形使用 source 的明确坐标域，drawing 在导出时规范化到该矩形原点。
resolved style、ancestor transform 和有效 clip 必须冻结，避免截取子树后丢失继承。
snapshot 描述的 source rect 与 captured drawing 必须属于同一 epoch。

默认 aspect fit 使用扣除 view Border insets 的 client area：
`scale = min(client_width / source_width, client_height / source_height)`。
目标绘制区域再应用 view 的普通变换与最终 destination clip。零尺寸 source/client
产生空 drawing，不除零；非有限尺寸或变换返回受检错误。
viewport 滚动和缩放仍属于 view 的坐标协议，不手工累加绝对偏移。

source 的 dirty 区域按捕获矩形相交后变换到 destination，并合并旧/新可视区域。
source rect、aspect fit、DPI、clip 或资源版本改变时，修复完整旧/新 view 区域。
view 没有未消费更新时不重新捕获。多个更新可合并为最新稳定 epoch，但不能拼接
不同 epoch 的资源和 drawing。首批不承诺部分捕获或性能收益。

## 宿主背景是帧输入

建议在现行宿主/surface 适配边界增加 `HostBackgroundSnapshot`：背景 identity、
revision、宿主坐标到 Canvas 的变换、覆盖区域与不可变 drawing/resource closure。
背景导入使用相同资源闭包机制，但不假装属于 Figure 树。
背景提供者同样走 typed key + payload trait + registry，默认纯色、渐变/图像提供者与
外部宿主实现共用协议；不为内置背景增加 concrete 分支。

合成顺序为宿主背景、Canvas 场景、现有宿主 overlay。透明 root 保留 alpha，
opaque root 在其覆盖区域遮住背景。背景变换使用宿主坐标，不附加 Canvas 的滚动 origin。
背景 revision、窗口位置、DPI 或 surface repair 改变时请求完整修复。
首批由 Host 提供背景快照，backend 完整消费；不直接读取操作系统桌面或父控件像素。
不支持所需 alpha/背景合成的 backend 在提交前返回结构化错误，入口仍保留未完成状态。

## 外部消费者证据

实施必须由独立消费者 crate 注册自定义 view policy 和 host background provider，
不修改 Core，覆盖和默认 Thumbnail 相同的测量、clip、epoch、资源和释放路径。
一个 source 同时连接两个不同大小的 destination Runtime，验证 mutation 仅发生一次、
两个 view 同步更新，关闭一个 view 不影响另一个。禁止只用两个重复场景模拟共享源。

验收反例包括倒序更新、source 替换后迟到快照、image/font 更新、资源拒绝、capture
失败恢复、destination device reset、非单位 DPI、非零 source rect、嵌套 clip、
source/view 关闭及空尺寸。Native 提供拖拽前后、overview resize 和 opaque/transparent
背景对照截图。实际实现不得提前声明 Web/Windows/Linux 通过。

## 实施顺序与评审点

K07 保持单链推进：源快照/订阅与资源闭包 → destination 导入和普通 view →
Thumbnail 场景及 Native 验收 → 宿主背景帧输入 → InheritBackground 验收。
每个切片补 API 账本、外部消费者、书籍和执行证据后再推进下一项。

请求评审的关键取舍是：跨 Runtime 传递完整不可变快照并在 destination 导入资源；
view 的交互保持只读；宿主背景作为独立帧输入；首批只观察 committed source state。
批准后固化 ADR 并核对当前实现，不在本草案阶段冻结未经验证的 Rust 签名。
