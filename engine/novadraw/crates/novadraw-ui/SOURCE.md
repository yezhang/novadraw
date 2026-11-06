# 源码来源

能力归属：其他——本地 UI 工作台。

迁入源为 [craft-libs](https://github.com/storytold/craft-libs/tree/9be9a63782298ab59cd9b7a42ee9cba4f42a5ccb/crates/ui)
的固定提交 `9be9a63782298ab59cd9b7a42ee9cba4f42a5ccb`。
本包直接维护源码，不依赖 craft-ui 或其他 craft crate。
迁入模块与版权见 [NOTICE](NOTICE)，双许可证全文随包保留。

本地修改包括包名与文档引用、依赖裁剪、浮窗身份生成器及重复身份校验。
身份改用 UUID 随机源，不再使用全局计数器。反序列化沿用上游身份去重，
直接构造的布局新增 `DuplicateWindow` 校验，避免两个浮窗共享原生窗口。
原生诊断配置与环境会话缓存改为 Context 持有。
重复重建原生浮窗的调试诊断通过 `DockEvent::RebuiltNativeWindows` 返回，
不在绘制路径打印日志。
egui／winit 使用 crates.io 官方发布版，不保留定制分支依赖。
IME 使用官方描述符，编辑目标身份只在本地记录；移除定制 generation／reset
协议及通过 Debug 字符串识别字节删除事件的逻辑。RTL 使用官方 Ui Layout。
原分支专用测试改为验证官方字段、焦点顺序及字符删除事件，
不把官方缺少的原生重置确认能力标为已验证。
按钮、标签、布局、停靠、浮窗、面板、IME 的上游测试随代码迁入；
v1 JSON 夹具保持原内容。未迁入字体、国际化、印刷和屏幕快照资产。

后续修复在本包实施；对比上游时以此提交为起点，逐项审核协议和测试变化，
避免用上游最新代码覆盖本地修改。

`canvas.rs`／`editor_canvas.rs` 是本项目新增的宿主适配，不来自 craft-ui。
Composer 的有序输入接口用于同一条 Editor 会话链路，沿用原有 composition 状态机。
边界见 [Canvas 契约](../../doc/design/egui-editor-canvas.md)。
