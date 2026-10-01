import assert from "node:assert/strict";
import test from "node:test";

import { validateEvidence } from "./run_webgpu_browser_performance.mjs";

const REVISION = "0123456789abcdef0123456789abcdef01234567";

function validInput() {
  const samples = Array.from({ length: 30 }, () => ({
    prepare_submission_ns: 1,
    backend_submit_cpu_ns: 2,
    submit_return_to_gpu_completion_callback_ns: 3,
    gpu_completion_callback_to_next_raf_ns: 4,
    frame_start_to_next_raf_ns: 10,
    command_count: 4_097,
  }));
  const report = {
    schema_version: 1,
    benchmark: "ga2-webgpu-browser",
    status: "pass",
    build: {
      git_revision: REVISION,
      git_dirty: false,
    },
    browser: {
      document_hidden: false,
      document_has_focus: true,
    },
    adapter: {
      name: "",
      backend: "BrowserWebGpu",
    },
    surface: {
      pixel_width: 1_024,
      pixel_height: 768,
      scale_factor: 1,
    },
    sampling: {
      warmup_iterations: 5,
      sample_iterations: 30,
      rectangle_count: 4_096,
      figure_count: 4_097,
    },
    measurement_scope: {
      gpu_queue_completion_callback: true,
      first_raf_after_gpu_completion: true,
      browser_compositor_present: false,
      physical_display_scanout: false,
    },
    samples,
  };
  return {
    options: {
      expectedChromeMajor: 154,
      expectedRevision: REVISION,
      allowDirty: false,
    },
    version: {
      Browser: "Chrome/154.0.8037.58",
      "Protocol-Version": "1.3",
    },
    systemInfo: {
      gpu: {
        devices: [
          {
            vendorString: "Apple",
            deviceString: "ANGLE Metal Renderer: Apple M1 Pro",
          },
        ],
      },
    },
    benchmarkState: {
      hidden: false,
      focused: true,
      samples: 30,
    },
    page: {
      location:
        "http://127.0.0.1:49152/?mode=performance&backend=vello",
      visibility: {
        hidden: false,
        state: "visible",
        focused: true,
      },
      viewport: {
        innerWidth: 1_280,
        innerHeight: 813,
        devicePixelRatio: 1,
      },
      canvas: {
        left: 128,
        top: 22,
        right: 1_152,
        bottom: 790,
        width: 1_024,
        height: 768,
        backingWidth: 1_024,
        backingHeight: 768,
      },
      browserGpuAdapter: {
        vendor: "apple",
        architecture: "metal-3",
        isFallbackAdapter: false,
      },
      domResult: structuredClone(report),
      globalResult: report,
    },
  };
}

test("accepts complete hardware WebGPU evidence", () => {
  const input = validInput();
  assert.equal(validateEvidence(input), input.page.globalResult);
});

test("rejects a hidden browser page", () => {
  const input = validInput();
  input.page.visibility.hidden = true;
  assert.throws(() => validateEvidence(input), /captured page is hidden/);
});

test("rejects a benchmark completed without focus", () => {
  const input = validInput();
  input.page.globalResult.browser.document_has_focus = false;
  input.page.domResult.browser.document_has_focus = false;
  assert.throws(() => validateEvidence(input), /unfocused document/);
});

test("rejects a software GPU", () => {
  const input = validInput();
  input.systemInfo.gpu.devices[0].deviceString =
    "ANGLE Metal Renderer: Apple SwiftShader";
  assert.throws(
    () => validateEvidence(input),
    /software renderer/,
  );
});

test("rejects dirty formal evidence", () => {
  const input = validInput();
  input.page.globalResult.build.git_dirty = true;
  input.page.domResult.build.git_dirty = true;
  assert.throws(() => validateEvidence(input), /clean build/);
});

test("rejects compositor or display overclaims", () => {
  const input = validInput();
  input.page.globalResult.measurement_scope.browser_compositor_present = true;
  input.page.domResult.measurement_scope.browser_compositor_present = true;
  assert.throws(() => validateEvidence(input), /overclaims compositor/);
});
