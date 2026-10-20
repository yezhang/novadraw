package org.eclipse.draw2d.examples.graph;

import org.eclipse.draw2d.PositionConstants;
import org.eclipse.draw2d.graph.DirectedGraph;
import org.eclipse.draw2d.graph.Edge;
import org.eclipse.draw2d.graph.Node;
import org.eclipse.draw2d.graph.Subgraph;
import org.eclipse.swt.widgets.Display;

/** Executes the official fixture factories in the demo's actual reflection menu order. */
public final class GraphFixtureOracle {
    public static void main(String[] args) throws Exception {
        Display display = Display.getDefault();
        try {
            capture("directed", GraphTests.class, new DirectedGraphDemo().getGraphMethods(), 16);
            capture("compound", CompoundGraphTests.class, new CompoundGraphDemo().getGraphMethods(), 10);
        } finally {
            display.dispose();
        }
    }

    private static void capture(String kind, Class<?> factories, String[] menu, int expected)
            throws Exception {
        int count = 0;
        for (String method : menu) {
            if (method == null) {
                continue;
            }
            System.out.println("menu\t" + kind + "\t" + count + "\t" + method);
            count++;
            for (int direction : new int[] {PositionConstants.SOUTH, PositionConstants.EAST}) {
                String key = kind + "\t" + method + "\t" + direction;
                DirectedGraph graph;
                try {
                    graph = (DirectedGraph) factories.getMethod(method, int.class)
                            .invoke(null, direction);
                } catch (java.lang.reflect.InvocationTargetException failure) {
                    Throwable cause = failure.getCause();
                    System.out.println("failure\t" + key + "\t" + cause.getClass().getName()
                            + "\t" + String.valueOf(cause.getMessage()));
                    cause.printStackTrace(System.err);
                    continue;
                }
                System.out.println("fixture\t" + key + "\t" + graph.nodes.size() + "\t"
                        + graph.edges.size());
                for (int i = 0; i < graph.nodes.size(); i++) {
                    Node node = graph.nodes.get(i);
                    System.out.println("node\t" + key + "\t" + i + "\t"
                            + graph.nodes.indexOf(node.getParent()) + "\t"
                            + (node instanceof Subgraph) + "\t" + node.x + "," + node.y + ","
                            + node.width + "," + node.height + "\t"
                            + String.valueOf(node.data).replace("\t", " ").replace("\n", " "));
                }
                for (int i = 0; i < graph.edges.size(); i++) {
                    Edge edge = graph.edges.get(i);
                    StringBuilder points = new StringBuilder();
                    for (int p = 0; p < edge.getPoints().size(); p++) {
                        if (p > 0) {
                            points.append(";");
                        }
                        var point = edge.getPoints().getPoint(p);
                        points.append(point.x).append(",").append(point.y);
                    }
                    System.out.println("edge\t" + key + "\t" + i + "\t"
                            + graph.nodes.indexOf(edge.source) + "\t"
                            + graph.nodes.indexOf(edge.target) + "\t"
                            + points + "\t"
                            + (edge.vNodes == null ? 0 : edge.vNodes.size()));
                }
            }
        }
        if (count != expected) {
            throw new AssertionError(kind + " menu expected " + expected + ", got " + count);
        }
    }
}
