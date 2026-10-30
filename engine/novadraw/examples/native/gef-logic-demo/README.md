# GEF Logic 首个可运行切片

运行：

```bash
cargo run -p gef-logic-demo
```

macOS 可通过系统应用窗口启动：

```bash
python3 tools/scripts/run_gef_macos.py --demo logic
```

首屏是半加器：左侧两个 LED 提供输入，中间 XOR/AND 计算 Sum/Carry，右侧 LED
显示结果。侧栏每个输入有四个 bit 按钮；最右边是最低位，切换它即可检查半加器
四种组合。绿色端口和导线表示高电平，灰色表示低电平。

- Select：选中、拖动节点；选择导线后拖动端点控制柄重连。
- Wire：点击输出端口，再点击输入端口。输入端口最多接一条线，输出可扇出。
- Components：选择 LED、AND、OR 或 XOR，再点击画布放置。
- 预设菜单：半加器、全加器、四位加法器。采用展开的门级拓扑，全加器展示两个
  半加器和 OR 门，四位电路展示进位链；四位初始输入为官方的 3 和 7。
  切换时自动缩放到画布，未保存修改需在窗口中确认丢弃。
- Live / Ground：固定高、低电平源，仅有一个输出端口。
- Routing：Manual、Manhattan、Shortest path，切换进入撤销历史。
  Manual 下选择导线，拖动线段中点手柄新增折点，拖动折点手柄移动；
  侧栏可删除指定折点。自动路由保留折点数据，回到 Manual 后恢复手柄。
- Delete、Undo、Redo：支持节点、相邻导线和端口关系恢复。
- Save / Open：保存结构和输入设置；重新打开后重算信号。存在未保存修改时 Open
  禁用。文本框获得焦点时，Delete 和撤销快捷键不会进入画布。
- 画布中键拖动平移，滚轮滚动，状态栏 `+ / −` 缩放，Esc 取消当前工具。

边界：当前支持无环组合电路；不是完整官方 Logic 对等交付。
预设的可编辑 Circuit 层级与边界端口、guide、嵌套容器、clone、鸟瞰、直接编辑、
DnD 和打印仍待后续切片。详见[行为契约](../../../doc/design/editor/gef-logic-first-slice.md)。

验证：

```bash
cargo test -p gef-logic-demo --lib
cargo clippy -p gef-logic-demo --lib -- -D warnings
```

手动采样模型信号求值：

```bash
cargo test -p gef-logic-demo --lib propagation_costs -- --ignored --nocapture
cargo test -p gef-logic-demo --lib preset_routing_costs -- --ignored --nocapture
```

当前 Rust 1.99 的严格 Clippy 会检查出既有 Core lint；
只检查本样例可加 `--no-deps`。验证结果见
[实施记录](../../../doc/verification/reviews/gef-logic-first-slice-2026-10-29.md)。
本轮预设、相对折点和路由结果见
[后续记录](../../../doc/verification/reviews/gef-logic-routing-2026-10-30.md)。
