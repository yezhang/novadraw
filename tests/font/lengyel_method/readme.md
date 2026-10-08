## 贝塞尔曲线的网格划分方法

在 WebGPU 中设计线性存储结构（如 `storage buffer`）以管理动态数据（如贝塞尔曲线网格划分和索引），需要结合 **内存对齐**、**数据压缩** 和 **间接索引** 等策略。以下是具体的设计思路和步骤：

---

### **1. 贝塞尔曲线控制点的存储**

假设每个贝塞尔曲线用 **控制点列表** 表示（例如二次曲线有 3 个点，三次曲线有 4 个点），可以将所有曲线的控制点按以下方式存储到 `storage buffer` 中：

```wgsl
// 每个曲线的元数据（类型 + 控制点偏移）
struct CurveMeta {
    type: u32,       // 0: 直线，1: 二次曲线，2: 三次曲线
    pointOffset: u32 // 控制点数组的起始索引
};

// 控制点数据
struct Point {
    x: f32,
    y: f32
};

// 存储缓冲区布局
[[group(0), binding(0)]] var<storage> curveMeta: array<CurveMeta>;
[[group(0), binding(1)]] var<storage> curvePoints: array<Point>;
```

- **内存对齐**：确保 `CurveMeta` 和 `Point` 的字节对齐（例如 `CurveMeta` 使用 `u32` 而非 `u16` 以满足对齐要求）。
- **动态扩展**：新增曲线时，追加到 `curveMeta` 和 `curvePoints` 数组末尾。

---

### **2. 网格划分与相交索引存储**

将屏幕或空间划分为固定大小的网格（如 `GridSize = 16x16`），每个网格单元需要存储 **相交的贝塞尔曲线索引列表**。由于每个网格单元的曲线数量可能动态变化，需使用 **间接索引** 结构：

```wgsl
// 网格单元元数据（存储曲线索引的偏移和数量）
struct GridCell {
    indexOffset: u32,  // 在 indicesBuffer 中的起始位置
    indexCount: u32    // 相交曲线的数量
};

// 存储所有网格单元的索引列表
[[group(0), binding(2)]] var<storage> gridCells: array<GridCell>;
[[group(0), binding(3)]] var<storage> indicesBuffer: array<u32>;
```

#### **数据结构设计**

1. **网格单元元数据 (`GridCell`)**:

   - `indexOffset`: 在 `indicesBuffer` 中该网格单元的曲线索引起始位置。
   - `indexCount`: 该网格单元相交的曲线数量。

2. **索引缓冲区 (`indicesBuffer`)**:
   - 紧凑存储所有网格单元的曲线索引，例如：
     ```
     [网格0的曲线1, 曲线2, ..., 网格1的曲线3, 曲线4, ...]
     ```

#### **查询示例**

要获取第 `i` 个网格单元的曲线索引列表：

```wgsl
let cell = gridCells[i];
for (var j = 0; j < cell.indexCount; j++) {
    let curveIndex = indicesBuffer[cell.indexOffset + j];
    // 使用 curveIndex 读取曲线数据
}
```

---

### **3. 动态数据更新与内存管理**

为了支持动态变化的贝塞尔曲线和网格数据，需要设计 **内存分配策略**：

#### **方案1：预分配固定大小**

- **优点**：实现简单，性能稳定。
- **缺点**：内存浪费或限制最大数据量。

```wgsl
// 预分配最大容量
const MAX_CURVES = 1024;
const MAX_INDICES = 65536;

[[group(0), binding(0)]] var<storage> curveMeta: array<CurveMeta, MAX_CURVES>;
[[group(0), binding(1)]] var<storage> curvePoints: array<Point, MAX_CURVES * 4>;
[[group(0), binding(2)]] var<storage> gridCells: array<GridCell, GRID_SIZE>;
[[group(0), binding(3)]] var<storage> indicesBuffer: array<u32, MAX_INDICES>;
```

#### **方案2：动态分块管理**

- **优点**：内存利用率高，支持动态扩展。
- **缺点**：需要手动管理内存块，实现复杂。

```wgsl
// 分块存储（类似内存池）
struct MemoryChunk {
    offset: u32,
    size: u32
};

// 管理 indicesBuffer 的空闲块
[[group(0), binding(4)]] var<storage> freeChunks: array<MemoryChunk>;
```

---

### **4. 通用线性存储设计原则**

对于动态数据结构，可采用以下通用策略：

#### **(1) 数据分块 + 间接索引**

- 将数据分为 **元数据区** 和 **数据区**：
  - **元数据区**：存储每个元素的起始偏移和大小（类似 `GridCell`）。
  - **数据区**：紧凑存储动态内容（如 `indicesBuffer`）。

#### **(2) 长度前缀压缩**

- 变长数组可以用 **长度前缀** + **数据内容** 表示：
  ```
  [长度][数据1][数据2]...[数据N]
  ```
- 在 WGSL 中需手动解析偏移。

#### **(3) 内存池（Memory Pool）**

- 预分配一大块内存，动态分配和释放小块内存。
- 使用空闲链表（`freeChunks`）跟踪可用内存块。

---

### **5. 完整示例代码**

#### **CPU 端预处理（TypeScript）**

```typescript
// 定义网格和曲线数据
const gridSize = 16;
const curves: BezierCurve[] = [...]; // 所有贝塞尔曲线
const gridCells: GridCell[] = new Array(gridSize * gridSize).fill({ indexOffset: 0, indexCount: 0 });
const indices: number[] = [];

// 遍历所有曲线，填充网格索引
curves.forEach((curve, curveIndex) => {
    const bbox = calculateBoundingBox(curve);
    const gridCellsCovered = getCoveredGridCells(bbox, gridSize);
    gridCellsCovered.forEach(cell => {
        const cellIndex = cell.y * gridSize + cell.x;
        gridCells[cellIndex].indexCount++;
    });
});

// 计算索引偏移
let offset = 0;
gridCells.forEach(cell => {
    cell.indexOffset = offset;
    offset += cell.indexCount;
    cell.indexCount = 0; // 重置为0，用于后续填充
});

// 填充索引
curves.forEach((curve, curveIndex) => {
    const bbox = calculateBoundingBox(curve);
    const gridCellsCovered = getCoveredGridCells(bbox, gridSize);
    gridCellsCovered.forEach(cell => {
        const cellIndex = cell.y * gridSize + cell.x;
        const pos = gridCells[cellIndex].indexOffset + gridCells[cellIndex].indexCount;
        indices[pos] = curveIndex;
        gridCells[cellIndex].indexCount++;
    });
});

// 上传到 WebGPU Buffer
device.queue.writeBuffer(curveMetaBuffer, 0, curveMetaData);
device.queue.writeBuffer(gridCellsBuffer, 0, gridCells);
device.queue.writeBuffer(indicesBuffer, 0, new Uint32Array(indices));
```

#### **WGSL 着色器查询**

```wgsl
// 查询某个网格单元内的曲线
fn queryCurvesInCell(cellIndex: u32) -> array<u32> {
    let cell = gridCells[cellIndex];
    var curves: array<u32>;
    for (var i = 0; i < cell.indexCount; i++) {
        curves.push(indicesBuffer[cell.indexOffset + i]);
    }
    return curves;
}
```

---

### **总结**

- **内存对齐**：确保数据结构满足 WebGPU 的 `std430` 或 `std140` 对齐要求。
- **间接索引**：通过元数据区（`GridCell`）和数据区（`indicesBuffer`）分离动态内容。
- **动态管理**：使用预分配或内存池策略平衡内存利用率和性能。
- **通用性**：该设计可扩展至其他动态数据结构（如粒子系统、场景图等）。
