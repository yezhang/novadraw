# ADR-061 验证矩阵

类型：`verification-plan`

能力归属：Editor 会话与历史；richtext 领域编辑；应用集成；platform 原生输入。
状态：接口与样例已实施，自动验证和平台／性能验收分列，2026-11-06。
实际结果与本地证据见[实施记录](../reviews/specialized-edit-controller-2026-11-06.md)。

| 用例 | 要求 | 证据／状态 |
|---|---|---|
| plain 回归 | 旧接口走同一会话槽，IME、选择、NoChange、失败恢复不退化 | 10 项 direct-edit 回归通过 |
| 独立外部 controller | 单独 Cargo workspace 仅使用公开 API，类型身份拒绝错误查询 | 外部数字控制器通过；plain 查询返回 None |
| 外部领域状态 | 不包含 DirectTextEditState/TextFlow，单一草稿与 composition | 外部 Counter 和 richtext 适配器通过 |
| richtext 独立 controller | Emoji 字素删除、同段替换、选区与组合文字、旧文档不变 | 3 项领域测试及独立样例通过，覆盖相同文字替换后的选区收起 |
| 嵌入模型事务 | 接受生成一项历史；取消和 NoChange 不改模型；undo/redo 恢复投影 | 外部消费者与嵌入样例通过 |
| 失效与冲突 | revision 失配、retire、dispose、lease lost、迟到输入 | revision、retire、lease、fault 清理通过；完整 dispose 组合待补 |
| 输入互斥 | 文本指针/键盘与 Tool 不重复消费；拒绝切换保留旧会话 | 会话身份拖选、拒绝切换与 G3/G4 回归通过 |
| 失败渠道 | controller/plan 可恢复拒绝、未知状态、panic；反馈失败回滚 | 输入/plan/geometry 拒绝、未知状态与 panic 通过 |
| 几何 | caret/selection/preedit 同版；scroll、resize、zoom、DPI | 样例 scroll/resize/zoom 通过；完整 Bidi/DPI 组合待补 |
| Native | Winit bridge lease、中文 preedit/commit、候选窗和截图 | mixed 注入回放图像通过；人工系统候选窗未验 |
| 成本 | 相同负载下 mutation/layout/coordinator/accept 分段数据 | mixed/large 各宿主 216 样本；协调与反馈合计，独占协调器耗时未分离；giant 失败保留 |
| 容量 | 控制器取消后无历史外的文档／布局资产留存 | 外部资产引用与反馈清理通过；任意文档容量未验 |

长文增量排版、FR-02 性能门禁、既有黑帧与平台发布验收不由本矩阵关闭。
