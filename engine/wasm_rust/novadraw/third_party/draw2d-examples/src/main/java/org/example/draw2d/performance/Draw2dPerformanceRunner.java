// SPDX-License-Identifier: EPL-2.0
package org.example.draw2d.performance;

import java.io.IOException;
import java.lang.management.ManagementFactory;
import java.lang.management.MemoryMXBean;
import java.lang.management.MemoryUsage;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

import org.eclipse.draw2d.Figure;
import org.eclipse.draw2d.FanRouter;
import org.eclipse.draw2d.IFigure;
import org.eclipse.draw2d.Label;
import org.eclipse.draw2d.ChopboxAnchor;
import org.eclipse.draw2d.ConnectionRouter;
import org.eclipse.draw2d.PolylineConnection;
import org.eclipse.draw2d.RectangleFigure;
import org.eclipse.draw2d.ScalableLayeredPane;
import org.eclipse.draw2d.SWTGraphics;
import org.eclipse.draw2d.Viewport;
import org.eclipse.draw2d.geometry.Dimension;
import org.eclipse.draw2d.geometry.Rectangle;
import org.eclipse.draw2d.text.FlowPage;
import org.eclipse.draw2d.text.TextFlow;
import org.eclipse.swt.SWT;
import org.eclipse.swt.graphics.Font;
import org.eclipse.swt.graphics.GC;
import org.eclipse.swt.graphics.Image;
import org.eclipse.swt.widgets.Display;

/**
 * Reproducible Draw2D reference runner for the GA-2 comparison.
 */
public final class Draw2dPerformanceRunner {

	private static final String DRAW2D_REVISION = "4463d9d0ce13c19d10fbe769d29f28b7345a8cba";
	private static final int REPORT_SCHEMA_VERSION = 1;
	private static final int DEFAULT_WARMUP = 5;
	private static final int DEFAULT_SAMPLES = 30;
	private static final int LARGE_TREE_FIGURES = 4_096;
	private static final int LARGE_TREE_COLUMNS = 64;
	private static final int SHALLOW_DEPTH = 1_000;
	private static final int MAX_DEPTH = 10_000;
	private static final int TEXT_FIGURES = 1_000;
	private static final int TEXT_FLOW_FIGURES = 512;
	private static final int INDEPENDENT_CONNECTIONS = 1_000;
	private static final int GROUPED_CONNECTIONS = 256;
	private static final int VIEWPORT_FIGURES = 1_024;
	private static final int VIEWPORT_COLUMNS = 32;
	private static final int WIDTH = 1_024;
	private static final int HEIGHT = 768;
	private static final int LOGICAL_DPI = 96;

	private Draw2dPerformanceRunner() {
	}

	public static void main(String[] arguments) throws Exception {
		Options options = Options.parse(arguments);
		Display display = new Display();
		try {
			run(display, options);
		} finally {
			display.dispose();
		}
	}

	private static void run(Display display, Options options) throws Exception {
		MemorySnapshot beforeSetup = memorySnapshot();
		long setupStart = System.nanoTime();
		PreparedScenario prepared = prepare(display, options.scenario());
		long setupNs = System.nanoTime() - setupStart;
		MemorySnapshot afterSetup = memorySnapshot();

		try (ScenarioOperation operation = prepared.operation()) {
			for (int index = 0; index < options.warmup(); index++) {
				operation.run();
			}

			long[] samples = new long[options.samples()];
			Map<String, Long> expectedWork = null;
			for (int index = 0; index < samples.length; index++) {
				long start = System.nanoTime();
				Map<String, Long> work = operation.run();
				samples[index] = System.nanoTime() - start;
				if (expectedWork != null && !expectedWork.equals(work)) {
					throw new IllegalStateException("scenario work counters changed between samples");
				}
				expectedWork = work;
			}
			MemorySnapshot afterSamples = memorySnapshot();
			writeReport(options, prepared, setupNs, samples, expectedWork, beforeSetup, afterSetup, afterSamples);
		}
	}

	private static PreparedScenario prepare(Display display, String scenario) {
		return switch (scenario) {
		case "wide_tree_full_paint_4096" -> prepareWidePaint(display);
		case "deep_tree_full_paint_1000" -> prepareDeepPaint(display, SHALLOW_DEPTH);
		case "deep_tree_full_paint_10000" -> prepareDeepPaint(display, MAX_DEPTH);
		case "deep_tree_validate_1000" -> prepareDeepValidation(display, SHALLOW_DEPTH);
		case "deep_tree_validate_10000" -> prepareDeepValidation(display, MAX_DEPTH);
		case "label_refresh_wide_1000" -> prepareLabelRefresh(display, false);
		case "label_refresh_deep_1000" -> prepareLabelRefresh(display, true);
		case "text_flow_full_paint_wide_512" -> prepareTextFlowPaint(display, false);
		case "text_flow_full_paint_deep_512" -> prepareTextFlowPaint(display, true);
		case "independent_routing_1000" -> prepareRouting(display, INDEPENDENT_CONNECTIONS, false);
		case "grouped_routing_256" -> prepareRouting(display, GROUPED_CONNECTIONS, true);
		case "local_update_full_paint_1pct_4096" ->
			prepareUpdateAndPaint(display, LARGE_TREE_FIGURES / 100);
		case "full_update_full_paint_100pct_4096" ->
			prepareUpdateAndPaint(display, LARGE_TREE_FIGURES);
		case "viewport_full_paint_1024" -> prepareViewportPaint(display);
		default -> throw new IllegalArgumentException("unknown scenario: " + scenario);
		};
	}

	private static PreparedScenario prepareWidePaint(Display display) {
		Tree tree = buildWideTree(display);
		PaintOperation operation = new PaintOperation(display, tree.root(), LARGE_TREE_FIGURES + 1);
		return new PreparedScenario(
				new ScenarioConfig(LARGE_TREE_FIGURES + 1, 2, 1.0, 1.0, "none"),
				operation,
				"Draw2D recursive Figure paint into an offscreen SWT Image.");
	}

	private static PreparedScenario prepareDeepPaint(Display display, int depth) {
		Tree tree = buildDeepTree(display, depth);
		PaintOperation operation = new PaintOperation(display, tree.root(), depth);
		return new PreparedScenario(
				new ScenarioConfig(depth, depth, 1.0, 1.0, "none"),
				operation,
				"Draw2D recursive Figure paint into an offscreen SWT Image.");
	}

	private static PreparedScenario prepareDeepValidation(Display display, int depth) {
		Tree tree = buildDeepTree(display, depth);
		tree.root().validate();
		ScenarioOperation operation = new ScenarioOperation() {
			private boolean alternate;

			@Override
			public Map<String, Long> run() {
				alternate = !alternate;
				int size = alternate ? 2 : 3;
				tree.leaf().setPreferredSize(new Dimension(size, size));
				tree.root().validate();
				return Map.of("validated_leaf", 1L);
			}
		};
		return new PreparedScenario(
				new ScenarioConfig(depth, depth, 1.0, 1.0 / depth, "deepest leaf size toggle"),
				operation,
				"Invalidates the deepest Draw2D Figure and validates the complete path.");
	}

	private static PreparedScenario prepareLabelRefresh(Display display, boolean deep) {
		Font font = loadInterFont(display);
		Figure root = rootFigure(display);
		root.setFont(font);
		List<Label> labels = new ArrayList<>(TEXT_FIGURES);
		IFigure parent = root;
		for (int index = 0; index < TEXT_FIGURES; index++) {
			int row = index / 20;
			int column = index % 20;
			Label label = new Label(String.format(Locale.ROOT, "label-%04d", index));
			label.setBounds(new Rectangle(column * 48, row * 15, 46, 14));
			parent.add(label);
			labels.add(label);
			if (deep) {
				parent = label;
			}
		}
		root.validate();
		ScenarioOperation operation = new ScenarioOperation() {
			@Override
			public Map<String, Long> run() {
				long width = 0;
				for (Label label : labels) {
					width += label.getPreferredSize().width();
				}
				Map<String, Long> work = new LinkedHashMap<>();
				work.put("label_figures_refreshed", (long) labels.size());
				work.put("preferred_width_checksum", width);
				return work;
			}

			@Override
			public void close() {
				font.dispose();
			}
		};
		return new PreparedScenario(
				new ScenarioConfig(
						TEXT_FIGURES + 1,
						deep ? TEXT_FIGURES + 1 : 2,
						1.0,
						0.0,
						"cached intrinsic refresh"),
				operation,
				"Queries cached Draw2D Label preferred sizes using the repository Inter font.");
	}

	private static PreparedScenario prepareTextFlowPaint(Display display, boolean deep) {
		Font font = loadInterFont(display);
		Figure root = rootFigure(display);
		root.setFont(font);
		IFigure parent = root;
		for (int index = 0; index < TEXT_FLOW_FIGURES; index++) {
			FlowPage page = new FlowPage();
			int row = deep ? 0 : index % 40;
			page.setBounds(new Rectangle(0, row * 18, 160, 18));
			page.add(new TextFlow(String.format(Locale.ROOT, "flow paragraph %04d", index)));
			parent.add(page);
			if (deep) {
				parent = page;
			}
		}
		root.validate();
		PaintOperation paint = new PaintOperation(
				display,
				root,
				1L + TEXT_FLOW_FIGURES * 2L);
		ScenarioOperation operation = closeWithFont(paint, font);
		return new PreparedScenario(
				new ScenarioConfig(
						TEXT_FLOW_FIGURES + 1,
						deep ? TEXT_FLOW_FIGURES + 1 : 2,
						1.0,
						1.0,
						"none"),
				operation,
				"Paints 512 logical text flows; Draw2D represents each as FlowPage plus TextFlow.");
	}

	private static PreparedScenario prepareRouting(Display display, int count, boolean grouped) {
		Figure root = rootFigure(display);
		RectangleFigure source = rectangleFigure(display, new Rectangle(20, 40, 80, 40));
		RectangleFigure target = rectangleFigure(display, new Rectangle(900, 600, 80, 40));
		root.add(source);
		root.add(target);
		ChopboxAnchor sourceAnchor = new ChopboxAnchor(source);
		ChopboxAnchor targetAnchor = new ChopboxAnchor(target);
		ConnectionRouter router;
		if (grouped) {
			FanRouter fan = new FanRouter();
			fan.setSeparation(16);
			router = fan;
		} else {
			router = ConnectionRouter.NULL;
		}
		List<PolylineConnection> connections = new ArrayList<>(count);
		for (int index = 0; index < count; index++) {
			PolylineConnection connection = new PolylineConnection();
			connection.setSourceAnchor(sourceAnchor);
			connection.setTargetAnchor(targetAnchor);
			connection.setConnectionRouter(router);
			root.add(connection);
			connections.add(connection);
		}
		root.validate();
		ScenarioOperation operation = new ScenarioOperation() {
			@Override
			public Map<String, Long> run() {
				for (PolylineConnection connection : connections) {
					router.invalidate(connection);
				}
				long pointCount = 0;
				for (PolylineConnection connection : connections) {
					router.route(connection);
					pointCount += connection.getPoints().size();
				}
				Map<String, Long> work = new LinkedHashMap<>();
				work.put("route_calculations", (long) connections.size());
				work.put("route_point_count", pointCount);
				return work;
			}
		};
		return new PreparedScenario(
				new ScenarioConfig(
						count + 3,
						2,
						1.0,
						1.0,
						grouped
								? "route one shared FanRouter anchor-pair group"
								: "route every independent direct connection"),
				operation,
				grouped
						? "Invalidates and routes the complete Draw2D FanRouter group."
						: "Invalidates and routes independent Draw2D direct connections.");
	}

	private static PreparedScenario prepareUpdateAndPaint(Display display, int changed) {
		Tree tree = buildWideTree(display);
		List<IFigure> figures = List.copyOf(tree.root().getChildren());
		PaintOperation paint = new PaintOperation(display, tree.root(), LARGE_TREE_FIGURES + 1);
		ScenarioOperation operation = new ScenarioOperation() {
			private boolean alternate;

			@Override
			public Map<String, Long> run() {
				alternate = !alternate;
				int offset = alternate ? 1 : 0;
				for (int index = 0; index < changed; index++) {
					int column = index % LARGE_TREE_COLUMNS;
					int row = index / LARGE_TREE_COLUMNS;
					figures.get(index).setBounds(new Rectangle(column * 8 + offset, row * 8, 7, 7));
				}
				tree.root().validate();
				paint.paint();
				Map<String, Long> work = new LinkedHashMap<>();
				work.put("mutated_figures", (long) changed);
				work.put("painted_figures", (long) LARGE_TREE_FIGURES + 1);
				return work;
			}

			@Override
			public void close() {
				paint.close();
			}
		};
		return new PreparedScenario(
				new ScenarioConfig(
						LARGE_TREE_FIGURES + 1,
						2,
						1.0,
						(double) changed / LARGE_TREE_FIGURES,
						"alternate selected Figure x coordinate by one logical pixel"),
				operation,
				"Measures Draw2D mutation, validation, and full offscreen SWT paint.");
	}

	private static PreparedScenario prepareViewportPaint(Display display) {
		Figure root = rootFigure(display);
		Viewport viewport = new Viewport();
		viewport.setBounds(new Rectangle(100, 80, 800, 560));
		ScalableLayeredPane scalable = new ScalableLayeredPane();
		scalable.setBounds(new Rectangle(0, 0, 2_048, 2_048));
		for (int index = 0; index < VIEWPORT_FIGURES; index++) {
			int column = index % VIEWPORT_COLUMNS;
			int row = index / VIEWPORT_COLUMNS;
			scalable.add(rectangleFigure(
					display,
					new Rectangle(column * 56, row * 48, 48, 40)));
		}
		scalable.setScale(1.5);
		viewport.setContents(scalable);
		viewport.setViewLocation(160, 120);
		root.add(viewport);
		root.validate();
		PaintOperation operation = new PaintOperation(display, root, VIEWPORT_FIGURES + 3L);
		return new PreparedScenario(
				new ScenarioConfig(
						VIEWPORT_FIGURES + 3,
						4,
						0.25,
						1.0,
						"fixed scroll and 1.5x zoom"),
				operation,
				"Paints Draw2D Viewport clipping, translation, and ScalableLayeredPane scaling.");
	}

	private static Font loadInterFont(Display display) {
		String path = System.getProperty("inter.font");
		if (path == null || !display.loadFont(path)) {
			throw new IllegalStateException("failed to load repository Inter font: " + path);
		}
		return new Font(display, "Inter", 12, SWT.NORMAL);
	}

	private static ScenarioOperation closeWithFont(ScenarioOperation operation, Font font) {
		return new ScenarioOperation() {
			@Override
			public Map<String, Long> run() {
				return operation.run();
			}

			@Override
			public void close() {
				try {
					operation.close();
				} finally {
					font.dispose();
				}
			}
		};
	}

	private static Tree buildWideTree(Display display) {
		Figure root = rootFigure(display);
		IFigure leaf = root;
		for (int index = 0; index < LARGE_TREE_FIGURES; index++) {
			int column = index % LARGE_TREE_COLUMNS;
			int row = index / LARGE_TREE_COLUMNS;
			RectangleFigure figure = rectangleFigure(display, new Rectangle(column * 8, row * 8, 7, 7));
			root.add(figure);
			leaf = figure;
		}
		root.validate();
		return new Tree(root, leaf);
	}

	private static Tree buildDeepTree(Display display, int depth) {
		Figure root = rootFigure(display);
		IFigure parent = root;
		for (int index = 1; index < depth; index++) {
			RectangleFigure figure = rectangleFigure(display, new Rectangle(0, 0, 1, 1));
			parent.add(figure);
			parent = figure;
		}
		root.validate();
		return new Tree(root, parent);
	}

	private static Figure rootFigure(Display display) {
		Figure root = new Figure();
		root.setBounds(new Rectangle(0, 0, WIDTH, HEIGHT));
		root.setFont(display.getSystemFont());
		root.setBackgroundColor(display.getSystemColor(SWT.COLOR_WHITE));
		root.setForegroundColor(display.getSystemColor(SWT.COLOR_BLACK));
		root.setOpaque(true);
		return root;
	}

	private static RectangleFigure rectangleFigure(Display display, Rectangle bounds) {
		RectangleFigure figure = new RectangleFigure();
		figure.setBounds(bounds);
		figure.setBackgroundColor(display.getSystemColor(SWT.COLOR_BLUE));
		figure.setForegroundColor(display.getSystemColor(SWT.COLOR_BLACK));
		figure.setOpaque(true);
		return figure;
	}

	private static void writeReport(
			Options options,
			PreparedScenario prepared,
			long setupNs,
			long[] samples,
			Map<String, Long> work,
			MemorySnapshot beforeSetup,
			MemorySnapshot afterSetup,
			MemorySnapshot afterSamples) throws IOException, NoSuchAlgorithmException {
		long[] sorted = samples.clone();
		Arrays.sort(sorted);
		StringBuilder json = new StringBuilder(8_192);
		json.append("{\n");
		field(json, "schema_version", REPORT_SCHEMA_VERSION, true, 1);
		field(json, "generated_at", Instant.now().toString(), true, 1);
		field(json, "harness", "draw2d-offscreen-swt", true, 1);
		json.append("  \"environment\": {\n");
		field(json, "draw2d_revision", DRAW2D_REVISION, true, 2);
		field(json, "draw2d_jar_sha256", sha256(Path.of(System.getProperty("draw2d.jar"))), true, 2);
		field(json, "swt_jar_sha256", sha256(Path.of(System.getProperty("swt.jar"))), true, 2);
		field(json, "swt_version", SWT.getVersion(), true, 2);
		field(json, "java_version", System.getProperty("java.runtime.version"), true, 2);
		field(json, "java_vm", System.getProperty("java.vm.name"), true, 2);
		field(json, "jvm_arguments", String.join(" ", ManagementFactory.getRuntimeMXBean().getInputArguments()), true, 2);
		field(json, "operating_system", System.getProperty("os.name"), true, 2);
		field(json, "architecture", System.getProperty("os.arch"), false, 2);
		json.append("  },\n");
		json.append("  \"sampling\": {\n");
		field(json, "warmup_iterations", options.warmup(), true, 2);
		field(json, "sample_iterations", options.samples(), false, 2);
		json.append("  },\n");
		json.append("  \"measurement_scope\": {\n");
		field(json, "cpu_setup", true, true, 2);
		field(json, "cpu_operation", true, true, 2);
		field(json, "offscreen_swt_gc_paint", options.scenario().contains("paint"), true, 2);
		field(json, "native_window_present", false, true, 2);
		field(json, "input_to_present", false, true, 2);
		field(json, "process_rss_snapshot", beforeSetup.rssBytes() != null, false, 2);
		json.append("  },\n");
		json.append("  \"scenario\": {\n");
		field(json, "name", options.scenario(), true, 2);
		appendConfig(json, prepared.config());
		field(json, "setup_ns", setupNs, true, 2);
		field(json, "min_ns", sorted[0], true, 2);
		field(json, "p50_ns", percentile(sorted, 50), true, 2);
		field(json, "p95_ns", percentile(sorted, 95), true, 2);
		arrayField(json, "samples_ns", samples, true, 2);
		json.append("    \"memory\": {\n");
		field(json, "method", "ps-rss-snapshot-and-jvm-heap", true, 3);
		memoryFields(json, beforeSetup, afterSetup, afterSamples);
		json.append("    },\n");
		mapField(json, "work", work, true, 2);
		field(json, "notes", prepared.notes(), false, 2);
		json.append("  }\n");
		json.append("}\n");

		Path report = options.report();
		if (report.getParent() != null) {
			Files.createDirectories(report.getParent());
		}
		Files.writeString(report, json.toString(), StandardCharsets.UTF_8);
		System.out.printf(
				Locale.ROOT,
				"BENCH %s setup=%dns min=%dns p50=%dns p95=%dns work=%s%nREPORT %s%n",
				options.scenario(),
				setupNs,
				sorted[0],
				percentile(sorted, 50),
				percentile(sorted, 95),
				work,
				report);
	}

	private static void appendConfig(StringBuilder json, ScenarioConfig config) {
		json.append("    \"config\": {\n");
		field(json, "figure_count", config.figureCount(), true, 3);
		field(json, "maximum_depth", config.maximumDepth(), true, 3);
		json.append("      \"viewport_logical\": [1024, 768],\n");
		field(json, "logical_dpi", LOGICAL_DPI, true, 3);
		field(json, "visible_ratio", config.visibleRatio(), true, 3);
		field(json, "update_ratio", config.updateRatio(), true, 3);
		field(json, "input_trajectory", config.inputTrajectory(), false, 3);
		json.append("    },\n");
	}

	private static void memoryFields(
			StringBuilder json,
			MemorySnapshot beforeSetup,
			MemorySnapshot afterSetup,
			MemorySnapshot afterSamples) {
		nullableField(json, "before_setup_rss_bytes", beforeSetup.rssBytes(), true, 3);
		nullableField(json, "after_setup_rss_bytes", afterSetup.rssBytes(), true, 3);
		nullableField(json, "after_samples_rss_bytes", afterSamples.rssBytes(), true, 3);
		field(json, "before_setup_heap_used_bytes", beforeSetup.heapUsedBytes(), true, 3);
		field(json, "after_setup_heap_used_bytes", afterSetup.heapUsedBytes(), true, 3);
		field(json, "after_samples_heap_used_bytes", afterSamples.heapUsedBytes(), false, 3);
	}

	private static MemorySnapshot memorySnapshot() {
		MemoryMXBean memory = ManagementFactory.getMemoryMXBean();
		MemoryUsage heap = memory.getHeapMemoryUsage();
		return new MemorySnapshot(currentRssBytes(), heap.getUsed());
	}

	private static Long currentRssBytes() {
		try {
			Process process = new ProcessBuilder(
					"ps",
					"-o",
					"rss=",
					"-p",
					Long.toString(ProcessHandle.current().pid()))
					.start();
			String output = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8).trim();
			if (process.waitFor() != 0 || output.isEmpty()) {
				return null;
			}
			long rssKiB = Long.parseLong(output);
			return rssKiB > 0 ? rssKiB * 1_024L : null;
		} catch (IOException | InterruptedException | NumberFormatException error) {
			if (error instanceof InterruptedException) {
				Thread.currentThread().interrupt();
			}
			return null;
		}
	}

	private static long percentile(long[] sorted, int percentile) {
		int rank = Math.max(1, (sorted.length * percentile + 99) / 100);
		return sorted[Math.min(sorted.length - 1, rank - 1)];
	}

	private static String sha256(Path path) throws IOException, NoSuchAlgorithmException {
		MessageDigest digest = MessageDigest.getInstance("SHA-256");
		byte[] hash = digest.digest(Files.readAllBytes(path));
		StringBuilder text = new StringBuilder(hash.length * 2);
		for (byte value : hash) {
			text.append(String.format(Locale.ROOT, "%02x", value));
		}
		return text.toString();
	}

	private static void field(
			StringBuilder json,
			String name,
			Object value,
			boolean comma,
			int indentation) {
		indent(json, indentation);
		json.append('"').append(escape(name)).append("\": ");
		if (value instanceof String text) {
			json.append('"').append(escape(text)).append('"');
		} else {
			json.append(value);
		}
		json.append(comma ? ",\n" : "\n");
	}

	private static void nullableField(
			StringBuilder json,
			String name,
			Long value,
			boolean comma,
			int indentation) {
		indent(json, indentation);
		json.append('"').append(escape(name)).append("\": ");
		json.append(value == null ? "null" : value);
		json.append(comma ? ",\n" : "\n");
	}

	private static void arrayField(
			StringBuilder json,
			String name,
			long[] values,
			boolean comma,
			int indentation) {
		indent(json, indentation);
		json.append('"').append(escape(name)).append("\": [");
		for (int index = 0; index < values.length; index++) {
			if (index > 0) {
				json.append(", ");
			}
			json.append(values[index]);
		}
		json.append(comma ? "],\n" : "]\n");
	}

	private static void mapField(
			StringBuilder json,
			String name,
			Map<String, Long> values,
			boolean comma,
			int indentation) {
		indent(json, indentation);
		json.append('"').append(escape(name)).append("\": {");
		boolean first = true;
		for (Map.Entry<String, Long> entry : values.entrySet()) {
			if (!first) {
				json.append(", ");
			}
			json.append('"').append(escape(entry.getKey())).append("\": ").append(entry.getValue());
			first = false;
		}
		json.append(comma ? "},\n" : "}\n");
	}

	private static void indent(StringBuilder json, int indentation) {
		json.append("  ".repeat(indentation));
	}

	private static String escape(String value) {
		return value
				.replace("\\", "\\\\")
				.replace("\"", "\\\"")
				.replace("\n", "\\n")
				.replace("\r", "\\r")
				.replace("\t", "\\t");
	}

	private record Options(String scenario, int warmup, int samples, Path report) {

		static Options parse(String[] arguments) {
			String scenario = null;
			int warmup = DEFAULT_WARMUP;
			int samples = DEFAULT_SAMPLES;
			Path report = null;
			for (String argument : arguments) {
				if (argument.startsWith("--scenario=")) {
					scenario = argument.substring("--scenario=".length());
				} else if (argument.startsWith("--warmup=")) {
					warmup = positiveInteger("warmup", argument.substring("--warmup=".length()));
				} else if (argument.startsWith("--samples=")) {
					samples = positiveInteger("samples", argument.substring("--samples=".length()));
				} else if (argument.startsWith("--report=")) {
					report = Path.of(argument.substring("--report=".length()));
				} else {
					throw new IllegalArgumentException("unknown argument: " + argument);
				}
			}
			if (scenario == null || report == null) {
				throw new IllegalArgumentException("--scenario and --report are required");
			}
			return new Options(scenario, warmup, samples, report);
		}

		private static int positiveInteger(String name, String value) {
			int parsed = Integer.parseInt(value);
			if (parsed <= 0) {
				throw new IllegalArgumentException(name + " must be greater than zero");
			}
			return parsed;
		}
	}

	private record ScenarioConfig(
			int figureCount,
			int maximumDepth,
			double visibleRatio,
			double updateRatio,
			String inputTrajectory) {
	}

	private record PreparedScenario(
			ScenarioConfig config,
			ScenarioOperation operation,
			String notes) {
	}

	private record Tree(Figure root, IFigure leaf) {
	}

	private record MemorySnapshot(Long rssBytes, long heapUsedBytes) {
	}

	private interface ScenarioOperation extends AutoCloseable {

		Map<String, Long> run();

		@Override
		default void close() {
		}
	}

	private static final class PaintOperation implements ScenarioOperation {

		private final IFigure root;
		private final long figureCount;
		private final Image image;
		private final GC gc;

		PaintOperation(Display display, IFigure root, long figureCount) {
			this.root = root;
			this.figureCount = figureCount;
			image = new Image(display, WIDTH, HEIGHT);
			gc = new GC(image);
		}

		@Override
		public Map<String, Long> run() {
			paint();
			return Map.of("painted_figures", figureCount);
		}

		void paint() {
			SWTGraphics graphics = new SWTGraphics(gc);
			try {
				graphics.setClip(new Rectangle(0, 0, WIDTH, HEIGHT));
				root.paint(graphics);
			} finally {
				graphics.dispose();
			}
		}

		@Override
		public void close() {
			gc.dispose();
			image.dispose();
		}
	}
}
