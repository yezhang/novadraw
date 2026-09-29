/*******************************************************************************
 * Copyright (c) 2005 IBM Corporation and others.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Eclipse Public License 2.0 which is available at
 * http://www.eclipse.org/legal/epl-2.0.
 *
 * SPDX-License-Identifier: EPL-2.0
 *
 * Contributors:
 *     IBM Corporation - initial API and implementation
 *******************************************************************************/
package org.example.draw2d;

import org.eclipse.swt.SWT;
import org.eclipse.swt.layout.FillLayout;
import org.eclipse.swt.widgets.Display;
import org.eclipse.swt.widgets.Shell;

import org.eclipse.draw2d.FigureCanvas;
import org.eclipse.draw2d.Label;

/**
 * Draw2D HelloWorld 示例
 * 这是最基本的 Draw2D 程序，显示一个简单的标签
 */
public class HelloWorld {

    public static void main(String[] args) {
        Display d = new Display();
        Shell shell = new Shell(d);
        shell.setLayout(new FillLayout());

        FigureCanvas canvas = new FigureCanvas(shell);
        canvas.setContents(new Label("Hello World"));

        shell.setText("Draw2d HelloWorld (按 ESC 退出)");
        shell.setSize(400, 300);

        // 添加 ESC 键监听
        shell.addListener(SWT.KeyDown, event -> {
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
    }

}
