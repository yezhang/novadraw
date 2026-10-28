/*******************************************************************************
 * Draw2D Polyline Miter 尖角裁剪测试
 *******************************************************************************/
package org.example.draw2d;

import org.eclipse.swt.SWT;
import org.eclipse.swt.graphics.Color;
import org.eclipse.swt.graphics.LineAttributes;
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

public class PolylineMiterClipDemo {

    public static void main(String[] args) {
        Display d = new Display();
        Shell shell = new Shell(d);
        shell.setLayout(new FillLayout());
        shell.setText("Polyline Miter Clip Test - Draw2D (按 ESC 退出)");
        shell.setSize(1100, 700);

        FigureCanvas canvas = new FigureCanvas(shell);
        org.eclipse.draw2d.Figure root = new org.eclipse.draw2d.Figure();
        root.setLayoutManager(new XYLayout());

        int yPos = 30;
        int xStart = 50;

        addTitle(root, "Miter Join 尖角裁剪测试", xStart, yPos - 20);
        addNote(root, "红色虚线框 = 严格 Bounds (padding=0)", xStart + 300, yPos - 10);

        createSharpAngleTest(root, "30 deg 尖角", 30, 8, xStart, yPos + 30);
        yPos += 140;
        createSharpAngleTest(root, "20 deg 尖角", 20, 10, xStart, yPos + 30);
        yPos += 140;
        createSharpAngleTest(root, "10 deg 尖角", 10, 12, xStart, yPos + 30);
        yPos += 140;
        createLineWidthComparison(root, "线宽对比", 20, xStart, yPos + 30);

        canvas.setContents(root);

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

    private static void createSharpAngleTest(org.eclipse.draw2d.Figure parent, String label,
                                              int angleDegrees, int lineWidth, int x, int y) {
        // 标签
        Label lbl = new Label(label);
        lbl.setForegroundColor(new Color(null, 50, 50, 50));
        parent.add(lbl, new Rectangle(x, y, 100, 20));

        // 计算点位置
        int centerX = x + 140;
        int centerY = y + 50;
        int armLength = 60;
        double halfAngle = Math.toRadians(angleDegrees / 2.0);

        int leftX = (int) (centerX - armLength * Math.cos(halfAngle));
        int leftY = (int) (centerY - armLength * Math.sin(halfAngle));
        int apexX = centerX;
        int apexY = centerY;
        int rightX = (int) (centerX + armLength * Math.cos(halfAngle));
        int rightY = (int) (centerY - armLength * Math.sin(halfAngle));

        // 计算严格的 bounds (padding = 0)
        int strictBoundsMinX = Math.min(leftX, Math.min(apexX, rightX));
        int strictBoundsMinY = Math.min(leftY, Math.min(apexY, rightY));
        int strictBoundsMaxX = Math.max(leftX, Math.max(apexX, rightX));
        int strictBoundsMaxY = Math.max(leftY, Math.max(apexY, rightY));

        // 创建 PolylineShape (使用本地相对坐标)
        PolylineShape polyline = new PolylineShape();
        PointList points = new PointList();
        // 转换为本地坐标 (相对于 bounds 左上角)
        points.addPoint(leftX - strictBoundsMinX, leftY - strictBoundsMinY);
        points.addPoint(apexX - strictBoundsMinX, apexY - strictBoundsMinY);
        points.addPoint(rightX - strictBoundsMinX, rightY - strictBoundsMinY);
        polyline.setPoints(points);
        polyline.setLineWidth(lineWidth);
        LineAttributes la = new LineAttributes(lineWidth, SWT.CAP_FLAT, SWT.JOIN_MITER);
        polyline.setLineAttributes(la);
        polyline.setForegroundColor(new Color(null, 0, 100, 200));
        // 使用 bounds 作为约束添加到 parent
        parent.add(polyline, new Rectangle(
            strictBoundsMinX, strictBoundsMinY,
            strictBoundsMaxX - strictBoundsMinX,
            strictBoundsMaxY - strictBoundsMinY));

        // 红色虚线框显示严格的 bounds (padding = 0)
        RectangleFigure boundsRect = new RectangleFigure();
        boundsRect.setFill(false);
        boundsRect.setOutline(true);
        boundsRect.setLineStyle(SWT.LINE_DASH);
        boundsRect.setForegroundColor(new Color(null, 200, 50, 50));
        parent.add(boundsRect, new Rectangle(
            strictBoundsMinX, strictBoundsMinY,
            strictBoundsMaxX - strictBoundsMinX,
            strictBoundsMaxY - strictBoundsMinY));

        // 右侧显示详细的坐标和 bounds 信息
        int infoX = x + 320;
        int infoY = y - 10;

        // 标题行：角度和线宽
        addInfoLabel(parent, "Angle: " + angleDegrees + " deg, Width: " + lineWidth + "px",
                     infoX, infoY, new Color(null, 0, 0, 128), true);

        // Points 表格头
        addInfoLabel(parent, "Points (x, y):", infoX, infoY + 18, new Color(null, 80, 80, 80), false);
        addInfoLabel(parent, "  Left:  (" + leftX + ", " + leftY + ")",
                     infoX + 10, infoY + 33, new Color(null, 100, 100, 100), false);
        addInfoLabel(parent, "  Apex:  (" + apexX + ", " + apexY + ")",
                     infoX + 10, infoY + 48, new Color(null, 100, 100, 100), false);
        addInfoLabel(parent, "  Right: (" + rightX + ", " + rightY + ")",
                     infoX + 10, infoY + 63, new Color(null, 100, 100, 100), false);

        // Bounds 计算值
        addInfoLabel(parent, "Bounds (strict, padding=0):", infoX, infoY + 83, new Color(null, 80, 80, 80), false);
        addInfoLabel(parent, "  Min: (" + strictBoundsMinX + ", " + strictBoundsMinY + ")",
                     infoX + 10, infoY + 98, new Color(null, 100, 100, 100), false);
        addInfoLabel(parent, "  Max: (" + strictBoundsMaxX + ", " + strictBoundsMaxY + ")",
                     infoX + 10, infoY + 113, new Color(null, 100, 100, 100), false);
        addInfoLabel(parent, "  Size: " + (strictBoundsMaxX - strictBoundsMinX) + " x " +
                     (strictBoundsMaxY - strictBoundsMinY),
                     infoX + 10, infoY + 128, new Color(null, 100, 100, 100), false);
    }

    private static void createLineWidthComparison(org.eclipse.draw2d.Figure parent, String label,
                                                   int angleDegrees, int x, int y) {
        Label lbl = new Label(label);
        lbl.setForegroundColor(new Color(null, 50, 50, 50));
        parent.add(lbl, new Rectangle(x, y, 100, 20));

        int startX = x + 110;
        int spacing = 130;
        int[] lineWidths = {2, 5, 10, 15};
        String[] labels = {"2px", "5px", "10px", "15px"};

        for (int i = 0; i < lineWidths.length; i++) {
            int lineWidth = lineWidths[i];
            int bx = startX + i * spacing;
            int by = y + 10;

            Label bl = new Label(labels[i]);
            bl.setForegroundColor(new Color(null, 100, 100, 100));
            parent.add(bl, new Rectangle(bx, by - 15, 40, 15));

            int armLength = 30;
            double halfAngle = Math.toRadians(angleDegrees / 2.0);
            int apexX = bx + 35;
            int apexY = by + 50;
            int leftX = (int) (apexX - armLength * Math.cos(halfAngle));
            int leftY = (int) (apexY - armLength * Math.sin(halfAngle));
            int rightX = (int) (apexX + armLength * Math.cos(halfAngle));
            int rightY = (int) (apexY - armLength * Math.sin(halfAngle));

            // 计算严格的 bounds
            int strictMinX = Math.min(leftX, Math.min(apexX, rightX));
            int strictMinY = Math.min(leftY, Math.min(apexY, rightY));
            int strictMaxX = Math.max(leftX, Math.max(apexX, rightX));
            int strictMaxY = Math.max(leftY, Math.max(apexY, rightY));

            // 创建 PolylineShape，使用本地相对坐标
            PolylineShape polyline = new PolylineShape();
            PointList points = new PointList();
            points.addPoint(leftX - strictMinX, leftY - strictMinY);
            points.addPoint(apexX - strictMinX, apexY - strictMinY);
            points.addPoint(rightX - strictMinX, rightY - strictMinY);
            polyline.setPoints(points);
            polyline.setLineWidth(lineWidth);
            LineAttributes la = new LineAttributes(lineWidth, SWT.CAP_FLAT, SWT.JOIN_MITER);
            polyline.setLineAttributes(la);
            polyline.setForegroundColor(new Color(null, 0, 100, 200));
            // 使用 bounds 作为约束
            parent.add(polyline, new Rectangle(strictMinX, strictMinY,
                strictMaxX - strictMinX, strictMaxY - strictMinY));

            // 红色虚线框 - 严格 bounds
            RectangleFigure boundsRect = new RectangleFigure();
            boundsRect.setFill(false);
            boundsRect.setOutline(true);
            boundsRect.setLineStyle(SWT.LINE_DASH);
            boundsRect.setLineWidth(1);
            boundsRect.setForegroundColor(new Color(null, 220, 60, 60));
            parent.add(boundsRect, new Rectangle(strictMinX, strictMinY,
                strictMaxX - strictMinX, strictMaxY - strictMinY));

            // 显示简要坐标信息
            int infoX = bx + 55;
            int infoY = by + 20;
            addInfoLabel(parent, "w=" + lineWidth, infoX, infoY,
                        new Color(null, 80, 80, 80), false);
        }
    }

    private static void addTitle(org.eclipse.draw2d.Figure parent, String text, int x, int y) {
        Label lbl = new Label(text);
        lbl.setForegroundColor(new Color(null, 0, 0, 0));
        parent.add(lbl, new Rectangle(x, y, 300, 20));
    }

    private static void addNote(org.eclipse.draw2d.Figure parent, String text, int x, int y) {
        Label lbl = new Label(text);
        lbl.setForegroundColor(new Color(null, 150, 50, 50));
        parent.add(lbl, new Rectangle(x, y, 400, 20));
    }

    private static void addInfoLabel(org.eclipse.draw2d.Figure parent, String text,
                                     int x, int y, Color color, boolean bold) {
        Label lbl = new Label(text);
        lbl.setForegroundColor(color);
        parent.add(lbl, new Rectangle(x, y, 200, 15));
    }
}
