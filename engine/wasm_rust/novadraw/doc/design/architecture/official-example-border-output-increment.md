# 官方示例边框与图形输出增量

类型：`normative-design`

状态：`implemented / verified`。按现有官方示例 Core 计划和用户授权完成普通 Border、字体指标及资源扩展。
验证见[交付证据](../../../verification/evidence/official-examples-2026-10-23/implementation.md)。
`api_semantics`：`border.protocol`、`graphics.context`、`paint.protocol`、`connection.locator`。

本切片补齐现有 Border、资源和帧提交契约的消费者，不改变场景所有权或渲染主循环。

- K03：ConnectionAndAnchor 使用 `ConnectionEndpointLocator` 的 owner-side 语义，目标端默认距离为 14/4，源端为 4/-3；标签尺寸由 Core 的 preferred measurement 决定。
- K06：PageNode 选中后使用蓝色渐变和白色文字，切换选择时恢复原节点；主题颜色采用确定性配色，不宣称与各平台 SWT 系统颜色逐像素一致。
- K08：`SchemeBorder` 接受任意长度的 highlight/shadow 色带；`None` 表示保留该层空间但不绘制。top/left 和 bottom/right 的 inset 分别取各自色带长度。先画 bottom/right，再画 top/left；后者在外侧角点取得覆盖权。窄小矩形不产生倒置线段。透明色或未填满的角点不得声明 opaque。通过普通 `Border` trait 和 `CompoundBorder` 接入，外部消费者与官方样例使用同一路径。
- K09：PrintExample 构造三种字体标签、三种几何图形、Draw2D Button、两个连接节点和官方 brazil 图标。图形输出消费统一的 `prepare_submission`，覆盖文字、图片和连接。系统打印对话框和打印任务不在 Canvas 范围内。

文字装饰补充 `FontDecorationMetrics`，offset 明确定义为 Canvas 向下为正的 baseline 偏移，thickness 必须有限且大于零。`FontMetrics` 默认不带下划线数据，查询缺失指标返回 `FontMetricsUnsupported`；默认与外部字体引擎共用受检值和附加入口。Skrifa 的 font-space offset 在边界转换符号，并使用已解析字体的 size 与 normalized coordinates。Path 的单行文字使用这一指标绘制指定范围；此处不新增混合字体段的通用装饰排版器。

Zoom 使用已有 SVG 解码能力加载上游原始 SVG；当前仍在固有尺寸栅格化，不宣称缩放或 DPI 改变时自动重栅格化。

Print 使用内置字体替代系统字体，并明确保留视觉核验状态。ICO 资源按已有资源请求/完成契约解码；解码失败继续返回结构化错误。SchemeBorder 的外部集成测试覆盖不对称色带、透明层、CompoundBorder 累计 inset 和窄小矩形；样例测试覆盖拖拽、选择恢复、Print 的尺寸变化及输出命令。

Thumbnail 的共享树投影、透明 Canvas 背景输入、AA 模式和混合字体段的通用文字装饰仍按 K07/K08 推进，不能由本切片的局部完成状态替代。
