"""Package the repository GPU demos; preserve shader text and curve algorithms."""
from pathlib import Path
import shutil
import re
import hashlib
import json

root = Path(__file__).resolve().parent
sources = next(parent / "tests/font" for parent in root.parents
               if (parent / "tests/font/evan_wallace/index_stencil_method.html").exists())
gpu = root / "assets/gpu"
vendor = gpu / "vendor"
vendor.mkdir(parents=True, exist_ok=True)
urls = {
    "https://cdnjs.cloudflare.com/ajax/libs/opentype.js/1.3.3/opentype.min.js": "../vendor/opentype.min.js",
    "https://webgpufundamentals.org/3rdparty/wgpu-matrix.module.js": "../vendor/wgpu-matrix.module.js",
    "https://greggman.github.io/webgpu-utils/dist/0.x/webgpu-utils.module.js": "../vendor/webgpu-utils.module.js",
}
style = """
<style>
body{margin:0;padding:14px;background:#fff;color:#172923;font:13px/1.6 system-ui,sans-serif}
.demo-controls{display:flex;align-items:center;gap:10px;flex-wrap:wrap;margin-bottom:12px}
select,button{padding:5px;border:1px solid #d5ded7;border-radius:5px;background:white;color:#172923}
#gpu-status{display:block;margin:10px 0;color:#52635c;overflow-wrap:anywhere}
#rulerX,#rulerY,#html_text{display:none}
</style>
"""
controls = """
<div class="demo-controls"><label>字形 <select id="demo-char">
<option>永</option><option>层</option><option>曲</option><option>国</option><option>A</option><option>8</option><option>g</option>
</select></label><label>栅格边长 <select id="demo-size"><option>32</option><option>64</option><option>100</option><option selected>256</option><option>512</option></select> px</label></div>
<span id="gpu-status" role="status">正在初始化 GPU…</span>
<script>
const demoParams = new URLSearchParams(location.search);
const demoChar = demoParams.get("char") || "永";
const demoSize = Number(demoParams.get("size")) || 256;
document.querySelector("#demo-char").value = demoChar;
document.querySelector("#demo-size").value = demoSize;
for (const id of ["demo-char","demo-size"]) document.getElementById(id).addEventListener("change", () => {
  location.search = new URLSearchParams({char:document.querySelector("#demo-char").value,size:document.querySelector("#demo-size").value});
});
window.demoStatus = text => document.getElementById("gpu-status").textContent = text;
window.addEventListener("error", e => demoStatus("GPU 错误：" + e.message));
window.addEventListener("unhandledrejection", e => demoStatus("GPU 错误：" + (e.reason?.message || e.reason)));
if (!navigator.gpu) demoStatus("此浏览器没有 WebGPU；请通过 localhost 在支持 WebGPU 的浏览器打开。");
</script>
"""
manifest = []
for method, files in {
    "evan_wallace": ["index_stencil_method.html", "index_blend_method.html", "STSong.ttf"],
    "lengyel_method": ["render_char.html", "STSong.ttf"],
    "will_dobbie_method": ["index.html", "font.frag", "web.vert", "atlas.bmp", "vertices.bmp", "pages.json"],
}.items():
    dst = gpu / method
    dst.mkdir(exist_ok=True)
    for name in files:
        src = sources / method / name
        raw = src.read_bytes()
        manifest.append({"source":f"tests/font/{method}/{name}", "sha256":hashlib.sha256(raw).hexdigest()})
        if not name.endswith(".html"):
            if raw.startswith(b"00000000:"):
                # The supplied atlas is an xxd text dump. Restore bytes, not glyphs.
                restored = bytearray()
                for line in raw.decode().splitlines():
                    offset, tail = line.split(":", 1)
                    assert int(offset, 16) == len(restored)
                    restored.extend(bytes.fromhex(tail[1:40].strip()))
                assert restored[:2] == b"BM"
                size = int.from_bytes(restored[2:6], "little")
                assert restored[size:] in (b"", b"\n")
                restored = restored[:size]
                (dst / name).write_bytes(restored)
                manifest[-1]["restored_sha256"] = hashlib.sha256(restored).hexdigest()
                manifest[-1]["integration"] = "Lossless xxd text dump decoding"
                continue
            shutil.copy2(src, dst / name)
            continue
        text = raw.decode()
        for url, local in urls.items():
            text = text.replace(url, local)
        text = text.replace("</head>", '<link rel="icon" href="data:,">' + style + "</head>")
        if method != "will_dobbie_method":
            text = text.replace("<body>", "<body>" + controls)
            text = text.replace('const device = await adapter.requestDevice();', '''if (!adapter) throw new Error("未找到 WebGPU adapter");
        const device = await adapter.requestDevice();
        device.addEventListener("uncapturederror", e => demoStatus("GPU validation：" + e.error.message));''')
            # Font parses and preprocessing remain the source implementation.
            if method == "evan_wallace":
                text = text.replace('let testChar = "永";', 'let testChar = demoChar;')
                text = text.replace('drawGlyph("./STSong.ttf", testChar);', '// The original Canvas2D outline helper is not invoked in the report.')
                text = text.replace('drawGlyphInHtml(testChar);', '// The browser text preview is not used.')
                text = text.replace('renderWithStencil(glyph);', 'await renderWithStencil(glyph);\n        demoStatus("WebGPU 已提交 · " + char + " · " + demoSize + "×" + demoSize);')
                text = text.replace('let testChar = demoChar;', '''for (const id of ["fontStencilCanvas", "fontRenderCanvas"]) {
        const canvas = document.getElementById(id);
        canvas.width = canvas.height = demoSize;
      }
      let testChar = demoChar;''')
                text = text.replace("</head>", """<style>
#container{display:none}#fontRenderCanvas,#fontStencilCanvas{display:block;position:static;width:100%;height:auto;aspect-ratio:1;max-width:290px;margin:10px auto;border:1px solid #d5ded7;image-rendering:pixelated}
#fontStencilCanvas{max-width:100px}
</style></head>""")
                text = text.replace('<canvas id="fontStencilCanvas"', '<p>GPU 中间覆盖附件</p><canvas id="fontStencilCanvas"')
                text = text.replace('<canvas id="fontRenderCanvas"', '<p>GPU 最终字形</p><canvas id="fontRenderCanvas"')
            else:
                text = text.replace('await drawGlyph("./STSong.ttf", "层", step.options);', '''await drawGlyph("./STSong.ttf", demoChar, step.options);
        demoStatus("WebGPU 已提交 · " + demoChar + " · " + demoSize + "×" + demoSize + " · " + step.label);''')
                text = text.replace('renderStep(0);', '''const demoCanvas = document.getElementById("fontCanvas");
      demoCanvas.width = demoCanvas.height = demoSize;
      renderStep(0);''')
                text = text.replace("</head>", """<style>
#container{position:static;width:100%;height:auto}#fontCanvas{position:static;width:100%;height:auto;aspect-ratio:1;max-width:290px;margin:10px auto;display:block;border:1px solid #d5ded7;image-rendering:pixelated}
#toolbar{gap:5px}#toolbar button{font-size:12px}#status{flex-basis:100%}
</style></head>""")
        else:
            text = text.replace('var positions = { x: [], y: [] };', '''var demoFirstGlyph;
      function focusFirstGlyph() {
        if (!demoFirstGlyph || !pageData) return;
        const b = demoFirstGlyph;
        const ratio = pageData[0].width / pageData[0].height;
        const half = Math.max((b.x1-b.x0) * canvas.height/canvas.width, (b.y1-b.y0)/ratio) / 1.1;
        transform.x = (b.x0+b.x1)/2 - pageData[0].x;
        transform.y = 1 - (b.y0+b.y1)/2 - pageData[0].y;
        transform.zoom = Math.log2(half);
        finishAnimations();
      }
      var positions = { x: [], y: [] };''')
            text = text.replace('var int16Array = new Int16Array(vertBuffer);', '''var int16Array = new Int16Array(vertBuffer);
        const unsigned = new Uint16Array(vertBuffer);
        const xs = [0,1,2,3].map(i => unsigned[i*int16PerVertex]/65535);
        const ys = [0,1,2,3].map(i => unsigned[i*int16PerVertex+1]/65535);
        demoFirstGlyph = {x0:Math.min(...xs),x1:Math.max(...xs),y0:Math.min(...ys),y1:Math.max(...ys)};''')
            text = text.replace('if (firstFrame) {', 'if (firstFrame) {\n          focusFirstGlyph();')
            text = text.replace('<label><input type="checkbox" id="autopan" />Auto zoom</label>', '<label hidden><input type="checkbox" id="autopan" />Auto zoom</label><button onclick="focusFirstGlyph()">聚焦首字形</button>')
            text = text.replace("</head>", """<style>
html,body{height:100%;margin:0;padding:0}#toolbar{left:8px;right:8px;top:8px;max-width:none;padding:8px;background:rgba(24,48,41,.9);color:white;font-size:12px}
#toolbar> a,#toolbar p:first-of-type,#reloadbutton,#frametime{display:none!important}#toolbar p{margin:4px 0}#loadinginfo{top:auto;bottom:0;font-size:11px;max-height:70px;overflow:auto}
</style></head>""")
            text = text.replace('href="/post/gpu-text-rendering-with-vector-textures/"', 'href="https://wdobbie.com/post/gpu-text-rendering-with-vector-textures/"')
        # Prove that embedded shader bodies are unchanged.
        shaders = lambda html: re.findall(r'<script[^>]*type="x-shader"[^>]*>(.*?)</script>', html, re.S)
        assert shaders(raw.decode()) == shaders(text), name
        (dst / name).write_text(text)
(gpu / "source-manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
