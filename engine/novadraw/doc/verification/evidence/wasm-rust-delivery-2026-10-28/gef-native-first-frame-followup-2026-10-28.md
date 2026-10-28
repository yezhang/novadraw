# GEF Native 首帧续验

新增 resize/DPI 回归已通过：连续改变窗口大小和 DPI 后保留 source bounds、
会话身份与撤销历史；拖动中 resize 不提前提交，释放后仅产生一次 command。
目前共 7 项消费者测试通过，定向 Clippy、格式及文档门禁通过。

用户将现有窗口置前台后确认“内容出现了”。同一窗口的新截图可见实际工具栏、
矩形与椭圆，确认 Native Scene 能够显示。自动前台激活仍返回 false；
具体启动/遮挡时序尚未完整取证，不声称已经解决所有首帧问题。

- [终端路径失败截图](gef-native-first-frame-assets/native-exposure-final.png)
- [LaunchServices 路径失败截图](gef-native-first-frame-assets/native-launchservices.png)
- [用户确认后的原生画面](gef-native-first-frame-assets/native-visible-confirmed.png)
- [项目内完整证据](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/novadraw/doc/verification/reviews/masonry-native-first-frame-followup-2026-10-28.md)
- [标准 macOS 启动脚本](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/novadraw/tools/scripts/run_gef_macos.py)

已登记标准 `.app` 启动的 `gef-shapes-macos` Run action，保留原有默认入口。
现有窗口保持运行，不需重启。下一步分别验收原生拖动、撤销/重做和 resize；
当前显示确认不替代这些交互结果。
