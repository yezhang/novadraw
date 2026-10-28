const {chromium} = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
(async () => {
  const browser = await chromium.launch({
    executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless:true,
    args:['--enable-unsafe-webgpu'],
  });
  const context = await browser.newContext({viewport:{width:1280,height:1000}});
  const base = process.env.REPORT_URL || 'http://localhost:8782/';
  const records=[];
  for(const [label, url] of [
    ['evan','assets/gpu/evan_wallace/index_stencil_method.html'],
    ['blend','assets/gpu/evan_wallace/index_blend_method.html'],
    ['dobbie','assets/gpu/will_dobbie_method/index.html'],
    ['lengyel','assets/gpu/lengyel_method/render_char.html'],
  ]) {
    const page = await context.newPage();
    const errors=[];
    page.on('pageerror', e=>errors.push(e.message));
    page.on('console', msg=>{if(msg.type()==='error')errors.push(msg.text())});
    page.on('response', r=>{if(r.status()>=400)errors.push(`${r.status()} ${r.url()}`)});
    await page.goto(base+url);
    try {
      if(label==='dobbie') await page.waitForFunction(()=>typeof glyphProgram!=='undefined' && glyphProgram && atlasTexture && vertexCount>0 && pageData);
      else await page.locator('#gpu-status').filter({hasText:/已提交|错误|validation/}).waitFor({timeout:25000});
    } catch(e) { errors.push(e.message); }
    const info=await page.evaluate(()=>{
      const gl=document.querySelector('#beziercanvas')?.getContext('webgl');
      const debug=gl?.getExtension('WEBGL_debug_renderer_info');
      return {
        gpu:!!navigator.gpu,
        renderer:debug?gl.getParameter(debug.UNMASKED_RENDERER_WEBGL):null,
        status:document.querySelector('#gpu-status')?.textContent,
        loading:document.querySelector('#loadinginfo')?.textContent,
        glError:gl?.getError(),
        canvases:[...document.querySelectorAll('canvas')].map(c=>({id:c.id,width:c.width,height:c.height})),
      };
    });
    await page.screenshot({path:path.join(__dirname,`${label}-gpu.png`)});
    const interactions=[];
    if(label==='dobbie') {
      const before=await page.locator('#beziercanvas').screenshot();
      await page.locator('#showgrids').uncheck();
      await page.waitForFunction(()=>!renderRequired);
      const after=await page.locator('#beziercanvas').screenshot();
      interactions.push({action:'hide grid',pixelsChanged:!before.equals(after)});
      await page.mouse.move(640,500);
      await page.mouse.wheel(0,120);
      await page.waitForFunction(()=>Math.abs(transform.zoom-animTransform.zoom)<0.005);
      interactions.push({action:'wheel zoom',pixelsChanged:!after.equals(await page.locator('#beziercanvas').screenshot())});
      await page.getByRole('button',{name:'聚焦首字形'}).click();
      await page.locator('#showgrids').check();
      await page.screenshot({path:path.join(__dirname,'dobbie-focused-gpu.png')});
    } else {
      const target=label==='lengyel'?'#fontCanvas':'#fontRenderCanvas';
      const before=await page.locator(target).screenshot();
      await page.locator('#demo-char').selectOption('层');
      await page.locator('#gpu-status').filter({hasText:/已提交.*层/}).waitFor();
      interactions.push({action:'glyph change',pixelsChanged:!before.equals(await page.locator(target).screenshot())});
      await page.locator('#demo-size').selectOption('32');
      await page.locator('#gpu-status').filter({hasText:/已提交.*32×32/}).waitFor();
      interactions.push({action:'raster size',size:await page.locator(target).evaluate(c=>c.width)});
      if(label==='lengyel') {
        for(const step of [1,2,3,4]) {
          await page.locator(`[data-step="${step}"]`).click();
          await page.locator('#gpu-status').filter({hasText:new RegExp(`已提交.*step${step}`)}).waitFor();
          interactions.push({action:`step${step}`,submitted:true});
        }
      }
    }
    console.log(JSON.stringify({label,info,interactions,errors}));
    records.push({label,info,interactions,errors});
    await page.close();
  }
  const report=await context.newPage();
  const reportErrors=[];
  report.on('pageerror',e=>reportErrors.push(e.message));
  await report.goto(base+'font-rendering-comparison-2026-10-23.html');
  await report.locator('#experiment').scrollIntoViewIfNeeded();
  await report.waitForFunction(()=>[...document.querySelectorAll('iframe')].every(f=>{
    const d=f.contentDocument;
    return d?.querySelector('#gpu-status')?.textContent.includes('已提交') || d?.querySelector('#loadinginfo')?.style.visibility==='hidden';
  }));
  await report.screenshot({path:path.join(__dirname,'report-gpu.png')});
  const desktop=await report.evaluate(()=>({overflow:document.documentElement.scrollWidth>innerWidth,iframes:document.querySelectorAll('iframe').length}));
  await report.setViewportSize({width:390,height:844});
  await report.screenshot({path:path.join(__dirname,'report-mobile-gpu.png')});
  const mobile=await report.evaluate(()=>({overflow:document.documentElement.scrollWidth>innerWidth}));
  records.push({label:'report',desktop,mobile,errors:reportErrors});
  fs.writeFileSync(path.join(__dirname,'gpu-validation.json'),JSON.stringify(records,null,2));
  await browser.close();
  const failed=records.filter(r=>r.errors.length || r.info?.glError ||
    r.interactions?.some(i=>i.pixelsChanged===false || (i.size!==undefined && i.size!==32)) ||
    r.desktop?.overflow || r.mobile?.overflow);
  if(failed.length) throw new Error('GPU verification failed: '+failed.map(r=>r.label).join(', '));
})();
