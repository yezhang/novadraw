/*******************************************************************************
 * Draw2D Triangle Bounds Demo
 *
 * 实现思路：
 * 由于 Triangle 类是 final 的，无法继承。因此采用组合模式：
 * 1. 创建 TriangleWrapper 类继承 Figure
 * 2. 内部包含 Triangle 实例，并转发相关方法调用
 * 3. 在 paintFigure 中先绘制 Triangle，再绘制 bounds 虚线框
 *******************************************************************************/
package org.example.draw2d;

import org.eclipse.swt.SWT;
import org.eclipse.swt.graphics.Color;
import org.eclipse.swt.layout.FillLayout;
import org.eclipse.swt.widgets.Display;
import org.eclipse.swt.widgets.Shell;

import org.eclipse.draw2d.Figure;
import org.eclipse.draw2d.FigureCanvas;
import org.eclipse.draw2d.Graphics;
import org.eclipse.draw2d.Label;
import org.eclipse.draw2d.PositionConstants;
import org.eclipse.draw2d.Triangle;
import org.eclipse.draw2d.XYLayout;
import org.eclipse.draw2d.geometry.Rectangle;

/**
 * TriangleBoundsDemo - 验证 Triangle 的 bounds 和 outline clip 效果
 *
 * 键盘控制：
 * - ESC: 退出程序
 */
public class TriangleBoundsDemo {

    private static Shell shell;
    private static Display display;

    public static void main(String[] args) {
        System.out.println("========================================");
        System.out.println("Triangle Bounds Demo - Draw2D");
        System.out.println("========================================");
        System.out.println();

        display = new Display();
        shell = new Shell(display);
        shell.setLayout(new FillLayout());
        shell.setText("Triangle Bounds Demo - Draw2D (ESC=退出)");
        shell.setSize(950, 750);

        FigureCanvas canvas = new FigureCanvas(shell);

        // 创建根图形
        Figure root = new Figure();
        root.setLayoutManager(new XYLayout());

        // 添加标题标签
        Label titleLabel = new Label("Triangle Bounds Demo - 红色虚线框表示 getBounds() 返回的矩形");
        titleLabel.setForegroundColor(new Color(null, 50, 50, 50));
        root.add(titleLabel, new Rectangle(10, 10, 800, 20));

        // 创建各种三角形并添加到场景
        createTriangles(root);

        canvas.setContents(root);

        // 添加键盘监听
        shell.addListener(SWT.KeyDown, event -> {
            if (event.keyCode == SWT.ESC) {
                shell.dispose();
            }
        });

        shell.open();
        System.out.println("[INFO] 窗口已打开，按 ESC 退出");
        System.out.println();

        while (!shell.isDisposed()) {
            if (!display.readAndDispatch()) {
                display.sleep();
            }
        }

        System.out.println("[INFO] 程序结束");
        display.dispose();
    }

    /**
     * 创建各种三角形并添加到根图形
     */
    private static void createTriangles(Figure root) {
        System.out.println("========================================");
        System.out.println("初始化三角形 - 输出每个三角形的 bounds");
        System.out.println("========================================");

        int yPos = 50;

        // 第1行：NORTH 方向，不同线宽
        yPos = createTriangleRow(root, "NORTH", yPos, PositionConstants.NORTH,
            new float[]{1f, 3f, 5f, 10f, 20f});

        // 第2行：SOUTH 方向，不同线宽
        yPos = createTriangleRow(root, "SOUTH", yPos + 20, PositionConstants.SOUTH,
            new float[]{1f, 3f, 5f, 10f, 20f});

        // 第3行：EAST 方向，不同线宽
        yPos = createTriangleRow(root, "EAST", yPos + 20, PositionConstants.EAST,
            new float[]{1f, 3f, 5f, 10f, 20f});

        // 第4行：WEST 方向，不同线宽
        yPos = createTriangleRow(root, "WEST", yPos + 20, PositionConstants.WEST,
            new float[]{1f, 3f, 5f, 10f, 20f});

        // 第5行：大尺寸，测试 clip 效果
        yPos = createLargeTriangleRow(root, "Large (测试 clip)", yPos + 40,
            new float[]{5f, 15f, 30f, 50f});

        System.out.println("========================================");
        System.out.println("所有三角形初始化完成");
        System.out.println("========================================");
        System.out.println();
    }

    private static int createTriangleRow(Figure root, String directionName, int yPos, int direction, float[] lineWidths) {
        System.out.println("\n--- 行: " + directionName + " (y=" + yPos + ") ---");

        int xPos = 50;
        int size = 60;

        for (float lineWidth : lineWidths) {
            // 使用 TriangleWrapper 替代普通 Triangle
            TriangleWrapper t = new TriangleWrapper(directionName + "_w" + (int) lineWidth);
            t.setDirection(direction);
            t.setLineWidthFloat(lineWidth);
            t.setBackgroundColor(new Color(null, 255, 150, 100));
            t.setForegroundColor(new Color(null, 200, 80, 50));

            Rectangle bounds = new Rectangle(xPos, yPos, size, size);
            t.setBounds(bounds);

            // 添加到根图形
            root.add(t, bounds);

            // 输出 bounds 信息
            System.out.println("Triangle [" + t.getLabel() + "]:");
            System.out.println("  Bounds: " + t.getBounds());
            System.out.println("  Direction: " + directionName);
            System.out.println("  LineWidth: " + lineWidth);
            System.out.println();

            xPos += 100;
        }

        return yPos + size + 30;
    }

    private static int createLargeTriangleRow(Figure root, String label, int yPos, float[] lineWidths) {
        System.out.println("\n--- 行: " + label + " (y=" + yPos + ") ---");

        int xPos = 50;
        int size = 100;

        for (float lineWidth : lineWidths) {
            // 使用 TriangleWrapper 替代普通 Triangle
            TriangleWrapper t = new TriangleWrapper("Large_w" + (int) lineWidth);
            t.setDirection(PositionConstants.NORTH);
            t.setLineWidthFloat(lineWidth);
            t.setBackgroundColor(new Color(null, 100, 200, 255));
            t.setForegroundColor(new Color(null, 50, 150, 200));

            Rectangle bounds = new Rectangle(xPos, yPos, size, size);
            t.setBounds(bounds);

            // 添加到根图形
            root.add(t, bounds);

            // 输出 bounds 信息
            System.out.println("Triangle [" + t.getLabel() + "]:");
            System.out.println("  Bounds: " + t.getBounds());
            System.out.println("  Direction: NORTH");
            System.out.println("  LineWidth: " + lineWidth);
            System.out.println();

            xPos += 150;
        }

        return yPos + size + 50;
    }

    /**
     * TriangleWrapper - 包装 Triangle 类，在 outlineShape 中添加 bounds 虚线框绘制
     *
     * 由于 Triangle 是 final 类，无法继承，因此采用组合模式：
     * - 继承 Figure 类
     * - 内部包含 Triangle 实例
     * - 将图形相关方法转发给 Triangle 实例
     * - 在 paintFigure 中先绘制 Triangle，再绘制 bounds 虚线框
     */
    static class TriangleWrapper extends Figure {
        private final Triangle triangle;
        private final String label;

        public TriangleWrapper(String label) {
            this.label = label;
            this.triangle = new Triangle();
        }

        public String getLabel() {
            return label;
        }

        // 转发方法给内部的 Triangle
        public void setDirection(int direction) {
            triangle.setDirection(direction);
        }

        public void setLineWidthFloat(float lineWidth) {
            triangle.setLineWidthFloat(lineWidth);
        }

        @Override
        public void setBackgroundColor(Color bg) {
            triangle.setBackgroundColor(bg);
        }

        @Override
        public void setForegroundColor(Color fg) {
            triangle.setForegroundColor(fg);
        }

        @Override
        public void setBounds(Rectangle rect) {
            super.setBounds(rect);
            triangle.setBounds(rect);
        }

        @Override
        protected void paintFigure(Graphics graphics) {
            // 确保 triangle 的 bounds 与 wrapper 同步
            if (!triangle.getBounds().equals(getBounds())) {
                triangle.setBounds(getBounds());
            }

            // 使用 Triangle 的 paint 方法绘制（包括 fill 和 outline）
            // 这会完整绘制三角形，包括 fillShape 和 outlineShape
            triangle.paint(graphics);
        }

        @Override
        protected void paintBorder(Graphics graphics) {
            // 在 border 阶段绘制 bounds 虚线框
            Rectangle bounds = getBounds();

            graphics.pushState();
            graphics.setForegroundColor(new Color(null, 255, 0, 0));
            graphics.setLineStyle(Graphics.LINE_DOT);
            graphics.setLineWidth(1);
            graphics.drawRectangle(bounds.x, bounds.y, bounds.width - 1, bounds.height - 1);
            graphics.popState();

            // 绘制标签
            if (label != null) {
                graphics.pushState();
                graphics.setForegroundColor(new Color(null, 0, 0, 0));
                graphics.drawString(label, bounds.x, bounds.y - 15);
                graphics.popState();
            }
        }
    }
}
