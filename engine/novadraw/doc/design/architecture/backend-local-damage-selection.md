# Vello 后端的局部 damage 选择

类型：`normative-design`

能力归属：Vello backend。Core 命令记录、Editor 会话与 richtext 排版不变。

本切片使用现有 damage 和 clip 协议，在调用 Vello 编码字形之前选择绘制贡献。
普通字形使用实际轮廓边界，位图字形读取 strike 的尺寸和 bearing 元数据，
不解码 PNG。变换、字号、字体 variation 与 skew 都参与边界计算；
设备空间边界额外保留抗锯齿余量。仅在证明绘制范围与 damage 或当前 clip
不相交时跳过。COLR、stroke 和无法确定的边界继续绘制。

所有状态命令保持原顺序执行，Save／Restore／ResetClip 不得改变外层 damage
裁剪。边界缓存有固定条目与 variation 维度上限；session、snapshot 和资源
更新沿用后端资源生命周期清理，超出缓存容量时直接计算，不能漏画。

局部修补使用按 Vello 16 像素 tile 对齐的设备空间包围矩形。场景平移整数像素，
临时纹理按修补尺寸分配；各 damage 区域从相对原点复制到 retained texture。
全帧、截图和原生呈现共用该规则。整数 tile 平移保持设备像素采样相位。

验收包括实际位图和轮廓编码筛选、变换与 clip 状态恢复、多个 DPI 下的局部修补
与全帧像素比较，以及完整输出点击到 GPU 完成的 release 复测。普通轮廓要求
像素相等；旋转位图的 f32 重定位容许各通道最多 1/255 的舍入差异。此改动不复用
Figure 绘制回调，也不替代尚未接受的局部录制协议；Core 全文记录成本单独报告。
