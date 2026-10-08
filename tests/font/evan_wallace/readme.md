文章地址：
[《Easy Scalable Text Rendering on the GPU》](https://medium.com/@evanwallace/easy-scalable-text-rendering-on-the-gpu-c3f4d782c5ac)

### 核心原理：

- GPU stencil 具有'invert' 特征，可以翻转像素。
- 本文避免了 stencil buffer 的方法：使用加法混合（additive blending），颜色使用 1/255（原因是 GPU 中的颜色是 8 bit，从 0 到 1）。

### 具体步骤

- [x] 读取 glyph 的控制点数据（在线点、离线点）
  - glyph.path.commands
- [x] 准备三角形绘图数据（存储缓冲区）

  - 多个轮廓
  - topology = triangle-list，使用 drawIndexed 绘制。

- [x] 使用 stencil 方法绘制“多边形”字体（在线点）

  - [x] 阶段1，绘制 stencil 模板

    - render pass 配置
      - stencilLoadOp = 'clear'; // 如果是初次 renderpass 的 stencil 附件，默认是0，可以使用 'load' 配置。
      - stencilClearValue = 0
      - stencilLoadOp = 'store'
    - pipeline 配置
      - depthStencil 配置
        - depthCompare = 'always'
        - depthWriteEnabled = false, // 如果 format 中没有深度，可以省略这个参数；本案例中不涉及 depth 的写入，所以考虑设置 false。
        - stencilWriteMask = 0x1
        - stencilReadMask = 0x1
        - stencilFront/stencilBack
          - compare = 'always'
          - passOp = 'invert'
    - 执行绘制
      所有三角形只调用一次 draw，并触发多次 stencil 翻转

  - [x] 阶段2，绘制字体颜色
    - render pass 配置
      - stencilLoadOp = 'load'
      - stencilStoreOp = 'discard' // passOp = keep 时，StoreOp 可以保持为 store，具有相同效果（都是不写入）
      - stencilReadOnly = true
    - pipeline 配置
      - depthStencil 配置
        - depthCompare = 'always'
        - depthWriteEnabled = false
        - stencilWriteMask = 0x1
        - stencilReadMask = 0x1
        - stencilFront/stencilBack
          - compare = 'equal'
          - passOp = 'keep'
    - 执行
      pass.setStencilReference(1.0)
      pass.draw(), 绘制一个超出范围的大三角形
  - [ ] 抗锯齿，给绘制字体添加抗锯齿效果
    - [ ] 只在最后上屏幕的时候，添加抗锯齿效果；前面绘制 stencil 过程中，不使用抗锯齿
      - [x] 在 pipeline desc 上配置 multisample 多重采样
      - [x] 创建 msaa 纹理（sampleCount = 3)
      - [x] 在 render pass 中配置 resolve target
      - [x] 深度纹理配置 sampleCount = 4；在 MSAA 技术中，深度附件采样数需要与view中的msaa Texture 采样数是相同的。

- [x] 使用 add blending 方法绘制“多边形”字体（在线点）
  - [x] 阶段1，绘制颜色 blend 创建顶点缓冲区
    - 使用 纹理坐标 $u^2 -v < 0$ 修复曲线边缘
    - pipeline 配置 blend 模式
    - 着色器，输出 1/255 颜色值
    -
  - [x] 阶段2，绘制字体颜色

### 一些发现

使用 encoder 创建 renderpass 时，在colorAttachments 处填写的 view，其所在 context 必须与 encoder 所在 device 一致。即，

- colorAttachments[0].view = context.getCurrentTexture().createView()
  - 条件：context.configure({device,...}) <-- 这里的 device-A
- encoder = device.createCommandEncoder() <-- 这里的 device-B
- device-A，必须与 device-B 一致。

片段着色器中，纹理写入的几种方法：

- 使用 color colorAttachments, 片段着色器返回值。
- 在片段着色器中，使用 textureStore 函数，写入 bind group 中的纹理。
  - 与 textureStore 配合使用的是 textureLoad，textureLoad 函数直接从纹理中返回纹素，而不进行采样（sample）或过滤（Filter）。

context.configure 配置时的 alphaMode:

- 当设置为 premultiplied 时，片段着色器返回的颜色必须是 RGB\*A 的预乘颜色值 vec4f。
  为了达到每次混合时，RGB 新增 1/255, alpha 通道新增 1/255，那么，需要配置 blend 模式中的 dstFactor 为 “one-minus-src-alpha”。

```
context.configure({
  device: device,
  format: presentationFormat,
  alphaMode: "premultiplied", // 启用 alpha 通道后才能实现透明效果
});

```

```json
blend: {
  color: {
    operation: 'add',   // 源颜色 + 目标颜色
    srcFactor: 'one',   // 源权重 = 1.0（因已预乘）
    dstFactor: 'one-minus-src-alpha' // 目标权重 = 1 - 源Alpha
  },
  alpha: {
    operation: 'add',   // 透明度叠加
    srcFactor: 'one',   // 源透明度权重 = 1.0
    dstFactor: 'one'    // 目标透明度权重 = 1.0
  }
}
```

### 参考资料

- webgpu-utils 工具使用说明，https://greggman.github.io/webgpu-utils/docs/
