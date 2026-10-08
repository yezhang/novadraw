# 6. 平台、资源与渲染后端

> **本章解决的问题**：如何修改窗口、浏览器、资源和 GPU 集成，同时保持 Core 平台
> 无关。

## 6.1 三层边界

```text
平台宿主
-> 归一化输入、时间、DPI 和绘制表面
-> Core Runtime
-> backend-neutral RenderSubmission
-> 渲染后端
-> 完成或失败结果
-> Runtime acknowledgement
```

平台宿主不重复命中、坐标父链、滚动目标选择或编辑语义。渲染后端不修改 Figure 树、
布局或应用模型。

## 6.2 平台适配器

平台层负责：

- 把物理像素转换为逻辑表面坐标；
- 把系统输入转换为 Core 事件；
- 管理窗口和绘制表面生命周期；
- 提供单调时间与唤醒；
- 实现文本输入和输入法桥接；
- 根据 Runtime 的帧状态请求下一次绘制。

Native 与 Web 可以采用不同事件循环，但必须把相同语义交给 Runtime。平台专有类型
只能停留在 adapter 内部。

## 6.3 文本输入是独立协议

键盘按键不等于文本。输入法预编辑、提交、取消、选择范围和候选窗位置构成独立状态
机。平台桥接器持有系统对象，Editor 持有草稿会话，Core TextFlow 提供实际排版几何。

修改文本输入时必须验证：

- UTF-8 与 UTF-16 offset 转换；
- composition session 身份；
- 迟到事件隔离；
- 光标与候选窗表面坐标；
- Native 与 Web 的行为等价；
- headless replay 的确定性。

## 6.4 渲染后端

Core 输出不包含 Vello 类型的 `RenderSubmission`。后端负责：

- 把 backend-neutral 命令 lowering 为 Vello 或其他后端命令；
- 管理后端资源缓存和 session；
- 提交绘制；
- 返回可供宿主和 Runtime 决策的结果。

后端失败不能只通过 panic 表达。设计错误类型时应区分输入错误、可重试、surface
失效、device lost、资源耗尽和不可恢复错误，并保留后端原始 source。

边界规范：

- [`ADR-022：第三方类型与渲染依赖边界`](../../../doc/adr/adr-022-third-party-type-and-render-dependency-boundary.md)
- [`ADR-025：统一 Graphics 与 glyph 预处理`](../../../doc/adr/adr-025-unified-graphics-and-glyph-preparation.md)
- [`Vello 后端`](../../../novadraw-backend-vello/src/lib.rs)

## 6.5 资源交接

文件、URL、权限和平台缓存属于应用或平台 provider。Core 应接收 bytes、已解码资源或
平台无关完成结果，再把资源变化纳入有序 Runtime 事务。

不要因为 Native 可以访问文件系统，就在 Core 公共 API 中加入文件路径便利方法；
这会制造 Native/Web 不同的核心表面。

## 6.6 修改平台或后端的验证

至少覆盖：

- 无窗口命令和资源序列；
- surface resize、suspend 和恢复；
- DPI 与逻辑坐标；
- 后端失败分类及 Runtime acknowledgement；
- Native/Web 输入等价；
- 真实平台截图或交互验收。

只有真实 GPU、WindowServer 或浏览器行为无法由平台无关测试证明时，才依赖人工平台
验收。
