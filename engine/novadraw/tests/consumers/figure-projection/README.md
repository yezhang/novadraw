# FigureTree 受检投影消费者

能力归属：Core FigureTree。独立 workspace 只依赖公开 `novadraw` API，
提供共享平移 provider 和未知容器，验证五个坐标入口的 contribution 消费。
provider 的旧 container 回调会 panic，用于检查受检贡献确实成为唯一来源。

```sh
cargo test --manifest-path tests/consumers/figure-projection/Cargo.toml --locked --offline
cargo check --manifest-path tests/consumers/figure-projection/Cargo.toml --locked --offline --target wasm32-unknown-unknown
```

十七项测试覆盖共享模型变化而 component revision 不变、同值省 provider 回调、
后续未知容器回调冲突及失败后重试、非交换矩阵和非对称 insets、有限奇异、
逆结果不可表示时不安装部分缓存，以及 work／allocation 和留存错误通道。
新增命中策略回调改值后的拒绝与恢复、命中的唯一受检来源及预算失败、
公开 Runtime 事件回调的输入错误与合法 effects 提交、末尾 Activity 改值后的
最终检查与实际 capture，以及 Layout／Frame 坐标错误 source 包装。
新增完整 descriptor 候选的非法关联拒绝、旧组件／版本／缓存保留、
合法替换、显式移除后的实时回调，以及重新准入后的唯一来源。

新增真实 submission 录制的唯一投影来源、paint 回调晚期改值后的 source 错误、
帧身份与旧无障碍快照保留、完整重绘重试，以及 source 移动前不重新读取旧共享投影。
动画缓存反例确认共享投影改值但 source epoch／sample 不变时仍重新物化几何，
保留真实 local presentation transform，未知外部视觉包络使用 Full。
新增独立查询不发布稳定投影或坐标通知、source revision 不变的显式请求发布、
Skipped 后 Full 重试不重复事实，以及下一次共享变化的必要通知。
这些测试验证独立查询、来源更新、Runtime 输入及帧准备；
成功 Presented 的投影证据、旧像素擦除及性能采样仍归通用投影方案的后续验收。
