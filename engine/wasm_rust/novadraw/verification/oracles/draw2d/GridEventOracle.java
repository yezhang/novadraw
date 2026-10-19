package org.eclipse.draw2d.examples.layouts;

import org.eclipse.draw2d.GridData;
import org.eclipse.draw2d.IFigure;
import org.eclipse.draw2d.MouseEvent;
import org.eclipse.draw2d.Shape;
import org.eclipse.swt.SWT;
import org.eclipse.swt.graphics.Point;
import org.eclipse.swt.widgets.Button;
import org.eclipse.swt.widgets.Composite;
import org.eclipse.swt.widgets.Control;
import org.eclipse.swt.widgets.Display;
import org.eclipse.swt.widgets.Event;

/** Drives the unmodified official example's registered callbacks, not OS input injection. */
public final class GridEventOracle extends GridLayoutExample {
    private int updates;
    private Throwable failure;

    @Override
    protected void updateView() {
        updates++;
        super.updateView();
    }

    private static void record(String step, Object value) {
        System.out.println(step + "\t" + value);
    }

    private Button button(Composite parent, String text) {
        for (Control control : parent.getChildren()) {
            if (control instanceof Button candidate && text.equals(candidate.getText())) {
                return candidate;
            }
            if (control instanceof Composite child) {
                Button found = button(child, text);
                if (found != null) {
                    return found;
                }
            }
        }
        return null;
    }

    private void invoke(Button button) {
        if (button == null) {
            throw new AssertionError("Missing official button");
        }
        button.notifyListeners(SWT.Selection, new Event());
    }

    private void probe(Display display) {
        try {
            Point dpi = display.getDPI();
            record("platform", SWT.getPlatform());
            record("swt-version", SWT.getVersion());
            record("dpi", dpi.x + "," + dpi.y);
            record("font", getFigureCanvas().getFont().getFontData()[0]);
            record("initial-children", container.getChildren().size());
            record("initial-grid-data-enabled", widthHint.getEnabled());
            for (int i = 0; i < 9; i++) {
                invoke(button(composite, "Add"));
                IFigure child = container.getChildren().get(container.getChildren().size() - 1);
                record("add-" + i, child.getClass().getSimpleName() + ","
                        + child.getBackgroundColor().getRGB());
            }
            Shape shape = (Shape) container.getChildren().get(0);
            Event press = new Event();
            press.widget = getFigureCanvas();
            press.button = 1;
            shape.handleMousePressed(new MouseEvent(null, shape,
                    new org.eclipse.swt.events.MouseEvent(press)));
            record("selected-line-width", shape.getLineWidth());
            record("selected-grid-data-enabled", widthHint.getEnabled());
            GridData data = (GridData) gridLayout.getConstraint(shape);
            widthHint.setText("91");
            int before = updates;
            widthHint.notifyListeners(SWT.FocusOut, new Event());
            record("width-focus-out", data.widthHint + ",updates=" + (updates - before));
            widthHint.setText("92");
            before = updates;
            widthHint.notifyListeners(SWT.DefaultSelection, new Event());
            record("width-return", data.widthHint + ",updates=" + (updates - before));
            widthHint.setText("invalid");
            widthHint.notifyListeners(SWT.DefaultSelection, new Event());
            record("width-invalid", data.widthHint);
            invoke(button(composite, "Clear All"));
            record("clear-children", container.getChildren().size());
            record("clear-grid-data-enabled", widthHint.getEnabled());
            record("clear-selected-reference-retained", selectedShape == shape);
            invoke(button(composite, "Add"));
            record("add-after-clear", container.getChildren().get(0).getClass().getSimpleName());
        } catch (Throwable error) {
            failure = error;
        } finally {
            getShell().dispose();
        }
    }

    public static void main(String[] args) {
        Display display = Display.getDefault();
        GridEventOracle oracle = new GridEventOracle();
        try {
            display.asyncExec(() -> oracle.probe(display));
            oracle.run();
            if (oracle.failure != null) {
                throw new AssertionError("Official Grid event oracle failed", oracle.failure);
            }
        } finally {
            display.dispose();
        }
    }
}
