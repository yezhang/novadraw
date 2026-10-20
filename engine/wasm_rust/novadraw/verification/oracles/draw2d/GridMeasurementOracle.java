package org.eclipse.draw2d.examples.layouts;

import org.eclipse.draw2d.Figure;
import org.eclipse.draw2d.GridData;
import org.eclipse.draw2d.GridLayout;
import org.eclipse.draw2d.RectangleFigure;
import org.eclipse.draw2d.geometry.Dimension;
import org.eclipse.draw2d.geometry.Rectangle;
import org.eclipse.swt.widgets.Display;

/** Fixed, font-independent layout inputs using the official Grid implementation. */
public final class GridMeasurementOracle {
    public static void main(String[] args) {
        Display display = Display.getDefault();
        try {
            System.out.println("# case\twidth\theight\tpreferred\tminimum\tchildren");
            for (int mode = 0; mode < 9; mode++) {
                for (int width : new int[] {0, 80, 155, 300}) {
                    Figure container = new Figure();
                    GridLayout layout = new GridLayout();
                    layout.numColumns = 2;
                    layout.makeColumnsEqualWidth = mode == 1 || mode == 4;
                    container.setLayoutManager(layout);
                    container.setBounds(new Rectangle(0, 0, width, 160));
                    for (int i = 0; i < 3; i++) {
                        RectangleFigure child = new RectangleFigure();
                        child.setSize(70, 70);
                        child.setPreferredSize(new Dimension(70, 70));
                        GridData data = new GridData();
                        if (mode == 2 && i == 0) {
                            data.horizontalSpan = 2;
                            data.horizontalIndent = 9;
                            data.widthHint = 90;
                            data.heightHint = 30;
                        }
                        if (mode == 3 || mode == 4 || (mode == 5 && i != 1)) {
                            data.horizontalAlignment = GridData.FILL;
                            data.verticalAlignment = GridData.FILL;
                            data.grabExcessHorizontalSpace = true;
                            data.grabExcessVerticalSpace = true;
                        }
                        if ((mode == 6 || mode == 7) && i == 0) {
                            data.horizontalSpan = 2;
                            data.widthHint = 200;
                        }
                        if (mode == 7 && i == 2) {
                            data.grabExcessHorizontalSpace = true;
                            data.horizontalAlignment = GridData.FILL;
                        }
                        if (mode == 8 && i == 0) {
                            data.verticalSpan = 2;
                            data.heightHint = 200;
                        }
                        container.add(child, data);
                    }
                    Dimension preferred = container.getPreferredSize();
                    Dimension minimum = container.getMinimumSize();
                    container.validate();
                    StringBuilder children = new StringBuilder();
                    for (var child : container.getChildren()) {
                        if (children.length() > 0) {
                            children.append(";");
                        }
                        Rectangle bounds = child.getBounds();
                        children.append(bounds.x).append(",").append(bounds.y).append(",")
                                .append(bounds.width).append(",").append(bounds.height);
                    }
                    System.out.println(mode + "\t" + width + "\t160\t" + preferred.width + ","
                            + preferred.height + "\t" + minimum.width + "," + minimum.height
                            + "\t" + children);
                }
            }
        } finally {
            display.dispose();
        }
    }
}
