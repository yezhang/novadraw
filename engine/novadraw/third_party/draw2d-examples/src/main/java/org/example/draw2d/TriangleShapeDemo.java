/*******************************************************************************
 * Draw2D Triangle Shape 测试
 * 测试 org.eclipse.draw2d.Triangle 类（Shape 子类）的各种效果
 * 支持截图功能（按 S 键）和场景选择
 *******************************************************************************/
package org.example.draw2d;

import org.eclipse.swt.SWT;
import org.eclipse.swt.graphics.Color;
import org.eclipse.swt.graphics.GC;
import org.eclipse.swt.graphics.Image;
import org.eclipse.swt.graphics.ImageData;
import org.eclipse.swt.graphics.ImageLoader;
import org.eclipse.swt.layout.FillLayout;
import org.eclipse.swt.widgets.Display;
import org.eclipse.swt.widgets.Event;
import org.eclipse.swt.widgets.Listener;
import org.eclipse.swt.widgets.Shell;

import org.eclipse.draw2d.FigureCanvas;
import org.eclipse.draw2d.Label;
import org.eclipse.draw2d.PositionConstants;
import org.eclipse.draw2d.Triangle;
import org.eclipse.draw2d.XYLayout;
import org.eclipse.draw2d.geometry.Rectangle;

import java.io.File;
import java.text.SimpleDateFormat;
import java.util.Date;

/**
 * TriangleShapeDemo - 测试 Triangle 类（Shape 子类）
 *
 * 键盘控制：
 * - ESC: 退出程序
 * - S: 截图保存到 screenshots 目录
 */
public class TriangleShapeDemo {

    private static Shell shell;
    private static Display display;
    private static int screenshotCounter = 0;

    public static void main(String[] args) {
        System.out.println("[DEBUG] 程序启动");
        System.out.println("[INFO] 键盘控制: ESC=退出, S=截图");

        display = new Display();
        shell = new Shell(display);
        shell.setLayout(new FillLayout());
        shell.setText("Triangle Shape Demo - Draw2D (ESC=退出, S=截图)");
        shell.setSize(900, 700);

        FigureCanvas canvas = new FigureCanvas(shell);
        org.eclipse.draw2d.Figure root = new org.eclipse.draw2d.Figure();
        root.setLayoutManager(new XYLayout());

        int yPos = 20;

        // ========== NORTH 方向 ==========
        createDirectionLabel(root, "Direction: NORTH (箭头向上)", yPos);
        yPos += 30;
        createTriangleRow(root, new int[]{1, 3, 5, 10, 20}, yPos, PositionConstants.NORTH,
                          new Color(null, 255, 100, 100), new Color(null, 200, 50, 50));
        yPos += 100;

        // ========== SOUTH 方向 ==========
        createDirectionLabel(root, "Direction: SOUTH (箭头向下)", yPos);
        yPos += 30;
        createTriangleRow(root, new int[]{1, 3, 5, 10, 20}, yPos, PositionConstants.SOUTH,
                          new Color(null, 100, 150, 255), new Color(null, 50, 100, 200));
        yPos += 100;

        // ========== EAST 方向 ==========
        createDirectionLabel(root, "Direction: EAST (箭头向右)", yPos);
        yPos += 30;
        createTriangleRow(root, new int[]{1, 3, 5, 10, 20}, yPos, PositionConstants.EAST,
                          new Color(null, 100, 220, 100), new Color(null, 50, 180, 50));
        yPos += 100;

        // ========== WEST 方向 ==========
        createDirectionLabel(root, "Direction: WEST (箭头向左)", yPos);
        yPos += 30;
        createTriangleRow(root, new int[]{1, 3, 5, 10, 20}, yPos, PositionConstants.WEST,
                          new Color(null, 255, 180, 80), new Color(null, 200, 130, 30));

        canvas.setContents(root);

        // 添加全局键盘监听器
        setupKeyboardListeners();

        shell.open();
        System.out.println("[DEBUG] Shell 已打开");

        // 主循环
        while (!shell.isDisposed()) {
            if (!display.readAndDispatch()) {
                display.sleep();
            }
        }

        System.out.println("[DEBUG] 程序结束");
        display.dispose();
    }

    /**
     * 设置键盘监听器
     */
    private static void setupKeyboardListeners() {
        // 使用全局过滤器捕获所有键盘事件
        display.addFilter(SWT.KeyDown, new Listener() {
            @Override
            public void handleEvent(Event event) {
                System.out.println("[DEBUG] 按键事件: keyCode=" + event.keyCode
                    + ", char='" + event.character + "'"
                    + ", ESC=" + SWT.ESC + ", 's'=" + (int)'s');

                // ESC 键退出
                if (event.keyCode == SWT.ESC) {
                    System.out.println("[INFO] ESC 键被按下，准备退出");
                    shell.dispose();
                }
                // S 键截图
                else if (event.character == 's' || event.character == 'S') {
                    System.out.println("[INFO] S 键被按下，准备截图");
                    takeScreenshot();
                }
            }
        });

        System.out.println("[DEBUG] 键盘监听器已设置");
    }

    /**
     * 截图功能
     */
    private static void takeScreenshot() {
        try {
            // 创建截图目录
            File screenshotDir = new File("screenshots");
            if (!screenshotDir.exists()) {
                screenshotDir.mkdirs();
            }

            // 生成文件名
            String timestamp = new SimpleDateFormat("yyyyMMdd_HHmmss").format(new Date());
            String filename = String.format("screenshots/triangle_demo_%s_%03d.png", timestamp, ++screenshotCounter);

            System.out.println("[DEBUG] 正在截图: " + filename);

            // 获取 Shell 的尺寸
            org.eclipse.swt.graphics.Rectangle bounds = shell.getBounds();

            // 创建图像
            Image image = new Image(display, bounds.width, bounds.height);
            GC gc = new GC(display);
            gc.copyArea(image, bounds.x, bounds.y);
            gc.dispose();

            // 保存图像
            ImageLoader loader = new ImageLoader();
            loader.data = new ImageData[]{image.getImageData()};
            loader.save(filename, SWT.IMAGE_PNG);

            image.dispose();

            System.out.println("[INFO] 截图已保存: " + filename);

        } catch (Exception e) {
            System.err.println("[ERROR] 截图失败: " + e.getMessage());
            e.printStackTrace();
        }
    }

    /**
     * 创建方向标签
     */
    private static void createDirectionLabel(org.eclipse.draw2d.Figure parent,
                                              String text, int y) {
        Label label = new Label(text);
        label.setForegroundColor(new Color(null, 50, 50, 50));
        parent.add(label, new Rectangle(20, y, 300, 20));
    }

    /**
     * 创建一行不同线宽的 Triangle
     */
    private static void createTriangleRow(org.eclipse.draw2d.Figure parent,
                                         int[] lineWidths, int y, int direction,
                                         Color fillColor, Color outlineColor) {
        int x = 50;

        for (int lineWidth : lineWidths) {
            // 创建 Triangle 对象
            Triangle triangle = new Triangle();
            triangle.setDirection(direction);
            triangle.setLineWidth(lineWidth);
            triangle.setForegroundColor(outlineColor);
            triangle.setBackgroundColor(fillColor);

            // 设置大小
            int size = 60;
            parent.add(triangle, new Rectangle(x, y, size, size));

            // 添加线宽标签
            Label label = new Label("w=" + lineWidth);
            label.setForegroundColor(new Color(null, 100, 100, 100));
            parent.add(label, new Rectangle(x, y + size + 5, 50, 15));

            x += 100;
        }
    }
}
