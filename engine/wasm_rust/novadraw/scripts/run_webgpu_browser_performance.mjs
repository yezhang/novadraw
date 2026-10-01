#!/usr/bin/env node

import { once } from "node:events";
import { createServer } from "node:http";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { basename, dirname, extname, join, normalize, sep } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const POLL_INTERVAL_MS = 100;
const CDP_STARTUP_TIMEOUT_MS = 15_000;
const CDP_COMMAND_TIMEOUT_MS = 10_000;
const NAVIGATION_TIMEOUT_MS = 30_000;
const BENCHMARK_TIMEOUT_MS = 120_000;
const RESULT_GLOBAL = "__NOVADRAW_PERFORMANCE_RESULT__";
const RESULT_ELEMENT_ID = "novadraw-performance-result";

export class CdpClient {
  constructor(url) {
    this.url = url;
    this.nextId = 1;
    this.pending = new Map();
    this.eventWaiters = new Map();
  }

  async connect() {
    this.socket = new WebSocket(this.url);
    await Promise.race([
      once(this.socket, "open"),
      timeout(10_000, `timed out connecting to CDP websocket ${this.url}`),
    ]);
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(String(event.data));
      if (!message.id) {
        const waiters = this.eventWaiters.get(message.method) ?? [];
        this.eventWaiters.delete(message.method);
        for (const waiter of waiters) {
          clearTimeout(waiter.timer);
          waiter.resolve(message.params ?? {});
        }
        return;
      }
      const pending = this.pending.get(message.id);
      if (!pending) {
        return;
      }
      this.pending.delete(message.id);
      clearTimeout(pending.timer);
      if (message.error) {
        pending.reject(
          new Error(
            `${pending.method} failed: ${message.error.message ?? JSON.stringify(message.error)}`,
          ),
        );
      } else {
        pending.resolve(message.result ?? {});
      }
    });
    this.socket.addEventListener("close", () => {
      for (const pending of this.pending.values()) {
        clearTimeout(pending.timer);
        pending.reject(new Error(`CDP websocket closed during ${pending.method}`));
      }
      this.pending.clear();
      for (const waiters of this.eventWaiters.values()) {
        for (const waiter of waiters) {
          clearTimeout(waiter.timer);
          waiter.reject(new Error(`CDP websocket closed before ${waiter.method}`));
        }
      }
      this.eventWaiters.clear();
    });
  }

  send(method, params = {}, timeoutMs = CDP_COMMAND_TIMEOUT_MS) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${method} exceeded ${timeoutMs} ms`));
      }, timeoutMs);
      this.pending.set(id, { method, resolve, reject, timer });
      try {
        this.socket.send(JSON.stringify({ id, method, params }));
      } catch (error) {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(error);
      }
    });
  }

  waitForEvent(method, timeoutMs) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        const waiters = this.eventWaiters.get(method) ?? [];
        const remaining = waiters.filter((waiter) => waiter.timer !== timer);
        if (remaining.length) {
          this.eventWaiters.set(method, remaining);
        } else {
          this.eventWaiters.delete(method);
        }
        reject(new Error(`${method} event exceeded ${timeoutMs} ms`));
      }, timeoutMs);
      const waiters = this.eventWaiters.get(method) ?? [];
      waiters.push({ method, resolve, reject, timer });
      this.eventWaiters.set(method, waiters);
    });
  }

  close() {
    this.socket?.close();
  }
}

function parseArguments(argv) {
  const options = {
    cdpUrl: "http://127.0.0.1:9222",
    dist: "",
    report: "",
    expectedRevision: "",
    expectedChromeMajor: 154,
    allowDirty: false,
  };
  for (const argument of argv) {
    if (argument.startsWith("--cdp-url=")) {
      options.cdpUrl = argument.slice("--cdp-url=".length);
    } else if (argument.startsWith("--dist=")) {
      options.dist = argument.slice("--dist=".length);
    } else if (argument.startsWith("--report=")) {
      options.report = argument.slice("--report=".length);
    } else if (argument.startsWith("--expected-revision=")) {
      options.expectedRevision = argument.slice("--expected-revision=".length);
    } else if (argument.startsWith("--expected-chrome-major=")) {
      options.expectedChromeMajor = Number(
        argument.slice("--expected-chrome-major=".length),
      );
    } else if (argument === "--allow-dirty") {
      options.allowDirty = true;
    } else {
      throw new Error(`unknown argument: ${argument}`);
    }
  }
  assert(options.dist, "--dist is required");
  assert(options.report, "--report is required");
  assert(
    /^[0-9a-f]{40}$/.test(options.expectedRevision),
    "--expected-revision must be a full Git SHA",
  );
  assert(
    Number.isInteger(options.expectedChromeMajor) &&
      options.expectedChromeMajor > 0,
    "--expected-chrome-major must be a positive integer",
  );
  return options;
}

async function startStaticServer(distDirectory) {
  const root = await realpath(distDirectory);
  const server = createServer(async (request, response) => {
    try {
      const requestUrl = new URL(request.url ?? "/", "http://127.0.0.1");
      const relative =
        requestUrl.pathname === "/"
          ? "index.html"
          : normalize(decodeURIComponent(requestUrl.pathname)).replace(
              /^[/\\]+/,
              "",
            );
      const path = join(root, relative);
      if (path !== root && !path.startsWith(`${root}${sep}`)) {
        throw new Error("path escapes static root");
      }
      const content = await readFile(path);
      response.writeHead(200, {
        "Cache-Control": "no-store",
        "Content-Type": contentType(path),
        "Cross-Origin-Opener-Policy": "same-origin",
      });
      response.end(content);
    } catch {
      response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
      response.end("Not Found");
    }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  assert(address && typeof address !== "string", "static server has no TCP address");
  return {
    server,
    url: `http://127.0.0.1:${address.port}/?mode=performance&backend=vello`,
  };
}

function contentType(path) {
  switch (extname(path)) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".css":
      return "text/css; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    case ".wasm":
      return "application/wasm";
    default:
      return "application/octet-stream";
  }
}

async function fetchJsonWithRetry(url) {
  const deadline = Date.now() + CDP_STARTUP_TIMEOUT_MS;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, {
        cache: "no-store",
        signal: AbortSignal.timeout(Math.max(1, deadline - Date.now())),
      });
      if (response.ok) {
        return await response.json();
      }
      lastError = new Error(`${url} returned HTTP ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(POLL_INTERVAL_MS);
  }
  throw new Error(`CDP endpoint unavailable: ${lastError}`);
}

async function evaluate(client, expression, awaitPromise = false) {
  const result = await client.send("Runtime.evaluate", {
    expression,
    awaitPromise,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(
      `browser evaluation failed: ${result.exceptionDetails.text ?? "unknown exception"}`,
    );
  }
  return result.result?.value;
}

async function waitForBenchmark(client) {
  const deadline = Date.now() + BENCHMARK_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const state = await evaluate(
      client,
      `JSON.stringify({
        ready: document.body?.dataset.ready ?? null,
        state: document.body?.dataset.performanceState ?? null,
        error: document.body?.dataset.performanceError ?? null,
        hidden: document.hidden,
        focused: document.hasFocus(),
        samples: Number(document.body?.dataset.performanceSamples ?? 0)
      })`,
    );
    const parsed = JSON.parse(state);
    if (parsed.state === "error" || parsed.ready === "error") {
      throw new Error(`browser benchmark failed: ${parsed.error ?? "unknown error"}`);
    }
    if (parsed.state === "complete" && parsed.ready === "true") {
      return parsed;
    }
    await delay(POLL_INTERVAL_MS);
  }
  throw new Error(`browser benchmark exceeded ${BENCHMARK_TIMEOUT_MS} ms`);
}

async function capturePageEvidence(client) {
  const expression = `(async () => {
    const adapter = await navigator.gpu?.requestAdapter({
      powerPreference: "high-performance"
    });
    const info = adapter?.info;
    const script = document.getElementById(${JSON.stringify(RESULT_ELEMENT_ID)});
    const canvas = document.getElementById("novadraw-canvas");
    const rect = canvas?.getBoundingClientRect();
    const domResult = script?.textContent ? JSON.parse(script.textContent) : null;
    const globalResult = globalThis[${JSON.stringify(RESULT_GLOBAL)}] ?? null;
    return {
      location: location.href,
      visibility: {
        hidden: document.hidden,
        state: document.visibilityState,
        focused: document.hasFocus()
      },
      viewport: {
        innerWidth,
        innerHeight,
        outerWidth,
        outerHeight,
        devicePixelRatio
      },
      canvas: rect ? {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
        width: rect.width,
        height: rect.height,
        backingWidth: canvas.width,
        backingHeight: canvas.height
      } : null,
      browserGpuAdapter: info ? {
        vendor: info.vendor ?? "",
        architecture: info.architecture ?? "",
        device: info.device ?? "",
        description: info.description ?? "",
        isFallbackAdapter: adapter.isFallbackAdapter ?? false
      } : null,
      domResult,
      globalResult
    };
  })()`;
  return await evaluate(client, expression, true);
}

export function validateEvidence({
  options,
  version,
  systemInfo,
  page,
  benchmarkState,
}) {
  const chromeMatch = /^Chrome\/(\d+)\./.exec(version.Browser ?? "");
  assert(chromeMatch, `unexpected CDP browser identity: ${version.Browser}`);
  assert(
    Number(chromeMatch[1]) === options.expectedChromeMajor,
    `expected Chrome ${options.expectedChromeMajor}, got ${version.Browser}`,
  );
  assert(
    version["Protocol-Version"],
    "CDP version response omitted Protocol-Version",
  );
  assert(benchmarkState.hidden === false, "benchmark page became hidden");
  assert(
    benchmarkState.samples === 30,
    `expected 30 samples, got ${benchmarkState.samples}`,
  );
  assert(page.visibility.hidden === false, "captured page is hidden");
  assert(page.visibility.state === "visible", "captured page is not visible");
  assert(page.visibility.focused === true, "captured page is not focused");
  const pageUrl = new URL(page.location);
  assert(pageUrl.hostname === "127.0.0.1", "benchmark did not use the local server");
  assert(
    pageUrl.searchParams.get("mode") === "performance",
    "benchmark page is not in performance mode",
  );
  assert(
    pageUrl.searchParams.get("backend") === "vello",
    "benchmark page is not using backend=vello",
  );
  assert(page.viewport.devicePixelRatio === 1, "browser DPR must be exactly 1");
  assert(page.canvas, "performance canvas is missing");
  assert(page.canvas.width === 1024, "canvas CSS width must be 1024");
  assert(page.canvas.height === 768, "canvas CSS height must be 768");
  assert(page.canvas.backingWidth === 1024, "canvas backing width must be 1024");
  assert(page.canvas.backingHeight === 768, "canvas backing height must be 768");
  assert(page.canvas.left >= 0 && page.canvas.top >= 0, "canvas starts off-screen");
  assert(
    page.canvas.right <= page.viewport.innerWidth &&
      page.canvas.bottom <= page.viewport.innerHeight,
    "canvas is not fully contained by the visible viewport",
  );
  assert(page.browserGpuAdapter, "navigator.gpu returned no adapter");
  assert(
    page.browserGpuAdapter.isFallbackAdapter === false,
    "browser selected a fallback WebGPU adapter",
  );
  assert(
    page.browserGpuAdapter.vendor.toLowerCase() === "apple",
    `expected Apple WebGPU vendor, got ${page.browserGpuAdapter.vendor}`,
  );
  assert(
    page.browserGpuAdapter.architecture.toLowerCase().startsWith("metal"),
    `expected Metal WebGPU architecture, got ${page.browserGpuAdapter.architecture}`,
  );

  const gpuText = JSON.stringify(systemInfo.gpu ?? {}).toLowerCase();
  assert(gpuText.includes("apple"), "CDP GPU diagnostics do not identify Apple GPU");
  assert(gpuText.includes("metal"), "CDP GPU diagnostics do not identify Metal");
  assert(
    !/(swiftshader|software only|llvmpipe)/.test(gpuText),
    "CDP GPU diagnostics identify a software renderer",
  );

  assert(page.domResult, "performance DOM report is missing");
  assert(page.globalResult, "performance global report is missing");
  assert(
    JSON.stringify(page.domResult) === JSON.stringify(page.globalResult),
    "DOM and global performance reports differ",
  );
  const report = page.globalResult;
  assert(report.schema_version === 1, "unexpected performance schema version");
  assert(report.benchmark === "ga2-webgpu-browser", "unexpected benchmark name");
  assert(report.status === "pass", "performance report did not pass");
  assert(
    report.browser.document_hidden === false,
    "performance report captured a hidden document",
  );
  assert(
    report.browser.document_has_focus === true,
    "performance report captured an unfocused document",
  );
  assert(
    report.build.git_revision === options.expectedRevision,
    `report revision ${report.build.git_revision} does not match ${options.expectedRevision}`,
  );
  if (!options.allowDirty) {
    assert(report.build.git_dirty === false, "formal evidence requires a clean build");
  }
  assert(report.adapter.backend === "BrowserWebGpu", "Vello did not use BrowserWebGpu");
  assert(
    !/(swiftshader|software|llvmpipe)/i.test(report.adapter.name),
    `invalid Vello adapter description: ${report.adapter.name}`,
  );
  assert(report.surface.pixel_width === 1024, "report surface width must be 1024");
  assert(report.surface.pixel_height === 768, "report surface height must be 768");
  assert(report.surface.scale_factor === 1, "report surface scale must be 1");
  assert(report.sampling.warmup_iterations === 5, "expected 5 warmups");
  assert(report.sampling.sample_iterations === 30, "expected 30 report samples");
  assert(report.sampling.rectangle_count === 4096, "expected 4096 rectangles");
  assert(report.sampling.figure_count === 4097, "expected 4097 total figures");
  assert(
    report.measurement_scope.gpu_queue_completion_callback === true &&
      report.measurement_scope.first_raf_after_gpu_completion === true,
    "report omits queue completion or post-completion RAF",
  );
  assert(
    report.measurement_scope.browser_compositor_present === false &&
      report.measurement_scope.physical_display_scanout === false,
    "report overclaims compositor or display presentation",
  );
  assert(Array.isArray(report.samples), "report samples are missing");
  assert(report.samples.length === 30, "report must retain exactly 30 raw samples");
  const commandCounts = new Set();
  for (const [index, sample] of report.samples.entries()) {
    commandCounts.add(sample.command_count);
    for (const field of [
      "prepare_submission_ns",
      "backend_submit_cpu_ns",
      "submit_return_to_gpu_completion_callback_ns",
      "gpu_completion_callback_to_next_raf_ns",
      "frame_start_to_next_raf_ns",
    ]) {
      assertFiniteNonNegative(sample[field], `samples[${index}].${field}`);
    }
  }
  assert(commandCounts.size === 1, "render command count changed across samples");
  assert([...commandCounts][0] > 0, "render command count must be positive");
  return report;
}

function assertFiniteNonNegative(value, label) {
  assert(
    Number.isFinite(value) && value >= 0,
    `${label} must be a finite non-negative number`,
  );
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function timeout(milliseconds, message) {
  return new Promise((_, reject) => {
    setTimeout(() => reject(new Error(message)), milliseconds);
  });
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const { server, url } = await startStaticServer(options.dist);
  let browserClient;
  let pageClient;
  try {
    const version = await fetchJsonWithRetry(`${options.cdpUrl}/json/version`);
    assert(version.webSocketDebuggerUrl, "CDP browser websocket URL is missing");
    browserClient = new CdpClient(version.webSocketDebuggerUrl);
    await browserClient.connect();
    const systemInfo = await browserClient.send("SystemInfo.getInfo");

    const targets = await fetchJsonWithRetry(`${options.cdpUrl}/json/list`);
    const pageTarget = targets.find(
      (target) => target.type === "page" && target.webSocketDebuggerUrl,
    );
    assert(pageTarget, "Chrome exposed no page target");
    pageClient = new CdpClient(pageTarget.webSocketDebuggerUrl);
    await pageClient.connect();
    await pageClient.send("Page.enable");
    await pageClient.send("Runtime.enable");
    const window = await browserClient.send("Browser.getWindowForTarget", {
      targetId: pageTarget.id,
    });
    await browserClient.send("Browser.setWindowBounds", {
      windowId: window.windowId,
      bounds: {
        left: 0,
        top: 0,
        width: 1280,
        height: 900,
        windowState: "normal",
      },
    });
    await pageClient.send("Page.bringToFront");
    await evaluate(pageClient, "window.focus(); true");
    const loaded = pageClient.waitForEvent(
      "Page.loadEventFired",
      NAVIGATION_TIMEOUT_MS,
    );
    await Promise.all([loaded, pageClient.send("Page.navigate", { url })]);
    await pageClient.send("Page.bringToFront");
    const benchmarkState = await waitForBenchmark(pageClient);
    await pageClient.send("Page.bringToFront");
    await evaluate(pageClient, "window.focus(); true");
    const page = await capturePageEvidence(pageClient);
    const benchmark = validateEvidence({
      options,
      version,
      systemInfo,
      page,
      benchmarkState,
    });
    const evidence = {
      schema_version: 1,
      suite: "backend.webgpu-browser-performance",
      status: "pass",
      captured_at_utc: new Date().toISOString(),
      runner: {
        node: process.version,
        chrome: version.Browser,
        protocol_version: version["Protocol-Version"],
        expected_chrome_major: options.expectedChromeMajor,
        allow_dirty: options.allowDirty,
      },
      page: {
        location: page.location,
        visibility: page.visibility,
        viewport: page.viewport,
        canvas: page.canvas,
        browser_gpu_adapter: page.browserGpuAdapter,
      },
      cdp_system_info: systemInfo,
      benchmark,
    };
    await mkdir(dirname(options.report), { recursive: true });
    await writeFile(options.report, `${JSON.stringify(evidence, null, 2)}\n`);
    console.log(`REPORT ${options.report}`);
  } finally {
    pageClient?.close();
    browserClient?.close();
    await new Promise((resolve, reject) => {
      server.close((error) => error ? reject(error) : resolve());
      server.closeAllConnections();
    });
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main().catch((error) => {
    console.error(`${basename(process.argv[1])}: ${error.stack ?? error}`);
    process.exitCode = 1;
  });
}
