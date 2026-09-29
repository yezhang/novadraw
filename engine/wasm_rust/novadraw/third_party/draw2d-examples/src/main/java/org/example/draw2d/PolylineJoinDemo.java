/*******************************************************************************
 * Draw2D 折线拐角描边宽度测试
 * 测试不同 line width 下折线拐角的表现
 *******************************************************************************/
package org.example.draw2d;

import org.eclipse.swt.SWT;
import org.eclipse.swt.graphics.Color;
import org.eclipse.swt.layout.FillLayout;
import org.eclipse.swt.widgets.Display;
import org.eclipse.swt.widgets.Shell;

import org.eclipse.draw2d.FigureCanvas;
import org.eclipse.draw2d.Label;
import org.eclipse.draw2d.PolylineShape;
import org.eclipse.draw2d.RectangleFigure;
import org.eclipse.draw2d.XYLayout;
import org.eclipse.draw2d.geometry.PointList;
import org.eclipse.draw2d.geometry.Rectangle;

/**
 * PolylineJoinDemo - 测试折线拐角描边宽度
 */
public class PolylineJoinDemo {

    public static void main(String[] args) {
        Display d = new Display();
        Shell shell = new Shell(d);
        shell.setLayout(new FillLayout());
        shell.setText("Polyline Join Test - Draw2D (按 ESC 退出)");
        shell.setSize(800, 600);

        FigureCanvas canvas = new FigureCanvas(shell);

        // 创建根图形
        org.eclipse.draw2d.Figure root = new org.eclipse.draw2d.Figure();
        root.setLayoutManager(new XYLayout());

        int yPos = 20;
        int xStart = 50;

        // 测试 1: line width = 1
        createTestRow(root, "Line Width = 1", 1, xStart, yPos);
        yPos += 80;

        // 测试 2: line width = 3
        createTestRow(root, "Line Width = 3", 3, xStart, yPos);
        yPos += 80;

        // 测试 3: line width = 5
        createTestRow(root, "Line Width = 5", 5, xStart, yPos);
        yPos += 80;

        // 测试 4: line width = 10
        createTestRow(root, "Line Width = 10", 10, xStart, yPos);
        yPos += 80;

        // 测试 5: line width = 20（对比明显）
        createTestRow(root, "Line Width = 20", 20, xStart, yPos);

        canvas.setContents(root);

        // 添加全局 ESC 键监听（使用 filter 实现全局拦截）
        d.addFilter(SWT.KeyDown, event -> {
            if (event.keyCode == SWT.ESC) {
                shell.dispose();
            }
        });

        shell.open();
        while (!shell.isDisposed()) {
            while (!d.readAndDispatch()) {
                d.sleep();
            }
        }

        d.dispose();
    }

    /**
     * 创建一行测试：包含标签和折线
     */
    private static void createTestRow(org.eclipse.draw2d.Figure parent, String label,
                                      int lineWidth, int x, int y) {
        // 标签
        Label lbl = new Label(label);
        lbl.setForegroundColor(new Color(null, 50, 50, 50));
        parent.add(lbl, new Rectangle(x, y, 120, 20));

        // 折线形状：L 形（90度拐角）
        PolylineShape polyline = new PolylineShape();
        PointList points = new PointList();
        // 绘制 L 形的三个点
        points.addPoint(x + 150, y + 10);   // 起点
        points.addPoint(x + 250, y + 10);   // 拐角点
        points.addPoint(x + 250, y + 50);   // 终点
        polyline.setPoints(points);
        polyline.setLineWidth(lineWidth);
        polyline.setForegroundColor(new Color(null, 0, 100, 200));
        polyline.setBackgroundColor(new Color(null, 100, 180, 255));
        parent.add(polyline, new Rectangle(0, 0, 0, 0));

        // 添加另一个形状：Z 形（多个拐角）
        PolylineShape zShape = new PolylineShape();
        PointList zPoints = new PointList();
        zPoints.addPoint(x + 300, y + 50);   // 起点
        zPoints.addPoint(x + 350, y + 10);   // 第一个拐角
        zPoints.addPoint(x + 400, y + 50);   // 第二个拐角
        zShape.setPoints(zPoints);
        zShape.setLineWidth(lineWidth);
        zShape.setForegroundColor(new Color(null, 200, 50, 50));
        zShape.setBackgroundColor(new Color(null, 255, 150, 150));
        parent.add(zShape, new Rectangle(0, 0, 0, 0));
    }
}
