# TextFlowLarge 独立性能研究应用

类型：`verification-plan`

能力归属：应用／宿主（`textflow-large-app`），位于 Core Native examples。
Core、Vello backend 与 Winit adapter 提供既有协议，本片不新增引擎能力。
状态：2026-11-03 已实现，定向测试、无窗口及 Native 自动回放通过；
人工交互未验收。结果见
[实施与验证](../reviews/textflow-large-app-implementation-2026-11-03.md)。
`api_semantics`：消费既有 `text.flow`、`text.interaction`、
`viewport.scroll_zoom`、`notification.layout_update`。

## 目标与输入

把 TextFlowLarge 从综合示例导航中独立出来，固定字体、文本与操作，方便反复
观察首次排版、追加、滚动、resize 和绘制成本。
研究应用使用真实 Runtime、Flow 和 Vello；不在应用层复制文本布局、
缓存、命中、坐标转换或事件仲裁。

独立 Cargo package 放在
`crates/novadraw/examples/native/textflow-large-app/`。
直接复用官方示例库的 `text_flow_large_example` 工厂；
综合示例和既有 `textflow-perf` 已使用同一函数，不迁移或复制 Large 正文。
初始逻辑视口为 600 × 500，固定追加 `Z`、滚动及宽度变化保留实际执行配置。
场景为 bundled Inter／JetBrains Mono 和原工厂的字体注册，不另选系统字体。

## 第一版行为

- 独立桌面窗口，保留原 Large 输入和滚动行为。
- 快捷操作：重建、追加、固定滚动、宽度切换、稳定查询、自动回放、导出。
- 自动回放在每轮新 Runtime 上执行固定操作；支持连续 resize 序列。
- 同一应用提供无窗口模式，验证场景、操作、IR 录制与导出，不初始化 GPU 或窗口。
- JSONL 保存配置、构建／源码版本信息、操作、阶段耗时、调用计数、失败、
  缓存命中、布局分母和实际录制命令／glyph 数。
- 默认关闭 Core timing，允许显式启用；Core 完整请求诊断与 timing 共用现有开关，
  应用可选择是否导出完整请求正文。
- 导出有界缓冲，明确记录 dropped；每轮重建前、关闭和回放退出时写文件，
  显式导出不覆盖旧文件。

只接入现有诊断与后端开关，不添加尚未实现的缓存、贡献复用或 shaping 策略开关。
场景操作与平台事件适配属于宿主编排，通用输入行为继续使用平台 adapter／Core。

## 测量口径

分别记录操作及稳定化、CPU IR 录制、backend submit 和可选 GPU queue wait。
研究帧统一请求 full redraw，使稳定查询也能产生完整帧；导出明确标注，
不将该成本解释为默认增量 damage 路径的成本。CPU IR 区间覆盖
`prepare_submission`，内部稳定化等工作通过可选阶段 timing 区分。
submit 返回 `Presented` 是后端提交结果；队列等待只是诊断同步，
两者均不证明 WindowServer 合成完成或物理屏幕 scanout。
无窗口模式标为 CPU recorded，不能写成 native presented。

阶段 timing 使用 Runtime 的 inclusive／exclusive 值，不相加不同阶段的分位数。
布局调用与缓存计数独立于墙钟耗时；录制 glyph 数不等于实际可见 glyph 数。
导出序列化和文件写入放在测量区间之外；GPU wait 单独开启并记录配置。
失败和重试保留，不以旧快照或跳过帧代替成功样本。
本片交付研究入口，不宣布新的延迟门禁通过。

## 验证与交付

1. 检查新 package、场景库导出与 workspace 注册，运行 workspace quick check。
2. 用无窗口回放验证初始正文、固定追加、纯滚动排版计数、resize 宽度、
   reset 后状态恢复，以及失败记录与预算失败后的新 Runtime 重试。
3. 检查导出可读、配置齐全、操作与帧能关联、缓冲溢出及文件不覆盖。
4. 构建 native app，启动真实窗口；执行一次自动回放及 GPU 队列等待，
   检查窗口／后端失败被报告；按可用能力保存实际画面并检查。
5. 更新 app README、样例导航和本计划状态，按范围执行格式、Clippy、文档及 diff 检查。

原始验证日志和导出放在忽略的 `target/verification/textflow-large-app/`。
README 给出正常运行、自动回放、无窗口和导出命令，以及各项测量边界。
FR-02 的贡献复用协议与 FR-04 Runtime 调度仍是后续独立研究。
