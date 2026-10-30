const {chromium}=require('playwright');
const assert=require('assert/strict');
const fs=require('fs');
(async()=>{
 const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});
 const receipts=[];
 for(const dpr of [1,2]){
  const context=await browser.newContext({viewport:{width:1100,height:850},deviceScaleFactor:dpr});
  const page=await context.newPage();await page.goto('http://127.0.0.1:8743');await page.waitForFunction(()=>window.e03);
  for(const creating of [false,true]){
   await page.getByRole('button',{name:creating?'Reset create':'Reset move'}).click();
   const r=await page.locator('#surface').boundingBox();
   await page.mouse.move(r.x+(creating?400:90),r.y+(creating?120:85));await page.mouse.down();
   await page.mouse.move(r.x+440,r.y+160,{steps:10});
   await page.keyboard.press('z');await page.keyboard.press('s');
   const before=await page.evaluate(()=>window.e03);
   assert(before.state.active);assert.equal(before.state.history,0);assert.equal(before.state.scale,1.25);
   assert(before.log.some(e=>e.kind==='zoom'&&e.trusted));
   assert(before.log.some(e=>e.kind==='scroll'&&e.trusted));
   assert.equal(before.log.slice(before.log.findLastIndex(e=>e.kind==='zoom')).filter(e=>e.kind==='move').length,0);
   await page.mouse.up();
   const after=await page.evaluate(()=>window.e03);
   assert.equal(after.state.history,1);assert.equal(after.state.builds,1);assert.equal(after.state.feedback,0);
   const bounds=after.state.visuals.find(v=>v.model===(creating?20:4)).bounds;
   const feedback=before.state.feedback_bounds[0];
   for(let i=0;i<4;i++) assert(Math.abs(bounds[i]-feedback[i])<1e-9);
   receipts.push({dpr,creating,before,after});
  }
  await context.close();
 }
 fs.writeFileSync('/tmp/novadraw-e03-browser-viewport-20261030.json',JSON.stringify(receipts,null,2));
 console.log('PASS move/create, DPR 1/2: stationary pointer zoom/scroll feedback equals committed geometry');
 await browser.close();
})().catch(e=>{console.error(e);process.exitCode=1});
