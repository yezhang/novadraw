const {chromium} = require('playwright');
const fs = require('fs');
const assert = require('assert/strict');
(async()=>{
 const browser = await chromium.launch({
   executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
   headless:true
 });
 const receipts=[];
 for(const dpr of [1,2]){
  const context=await browser.newContext({viewport:{width:1100,height:850},deviceScaleFactor:dpr});
  const page=await context.newPage();
  await page.goto('http://127.0.0.1:8743');
  await page.waitForFunction(()=>window.e03?.state);
  const rect=await page.locator('#surface').boundingBox();
  const point=(x,y)=>[rect.x+x,rect.y+y];
  const snap=async(name)=>{
   const data=await page.evaluate(()=>window.e03);
   receipts.push({scenario:name,dpr,...data});
   fs.writeFileSync('/tmp/novadraw-e03-browser-receipt-20261030.json',JSON.stringify(receipts,null,2));
   return data.state;
  };
  const node=(s,id)=>s.nodes.find(n=>n.model===id);
  const start=async(x,y)=>{await page.mouse.move(...point(x,y));await page.mouse.down();};
  const move=async(x,y)=>page.mouse.move(...point(x,y),{steps:12});
  await start(90,85);await move(440,160);
  let s=await snap('move-preview');assert.equal(s.active,true);assert.equal(s.history,0);assert.equal(s.builds,0);assert.equal(node(s,4).parent,2);
  await page.mouse.up();s=await snap('move-release');assert.equal(s.active,false);assert.equal(s.history,1);assert.equal(s.builds,1);assert.equal(s.feedback,0);assert.equal(node(s,4).parent,3);
  assert.deepEqual(node(s,4).bounds,[70,95,40,30]);
  await page.getByRole('button',{name:'Undo',exact:true}).click();s=await snap('move-undo');assert.equal(node(s,4).parent,2);
  await page.getByRole('button',{name:'Redo',exact:true}).click();s=await snap('move-redo');assert.equal(node(s,4).parent,3);
  await page.getByRole('button',{name:'Reset move'}).click();
  await start(90,85);await move(440,160);await page.keyboard.press('Escape');await page.mouse.up();
  s=await snap('move-escape');assert.equal(s.active,false);assert.equal(s.feedback,0);assert.equal(s.history,0);assert.equal(node(s,4).parent,2);
  await page.getByRole('button',{name:'Reset move'}).click();
  await start(90,85);await move(870,180);await page.mouse.up();
  s=await snap('move-captured-outside-surface');assert.equal(s.active,false);assert.equal(s.history,1);assert.equal(node(s,4).parent,1);
  await page.getByRole('button',{name:'Reset create'}).click();
  await page.mouse.click(...point(100,240));
  s=await snap('nested-create-click');assert.equal(s.history,1);assert.equal(s.builds,1);assert.equal(node(s,20).parent,6);assert.deepEqual(node(s,20).bounds,[30,40,30,20]);
  await page.getByRole('button',{name:'Undo',exact:true}).click();s=await snap('create-undo');assert.equal(node(s,20),undefined);
  await page.getByRole('button',{name:'Redo',exact:true}).click();s=await snap('create-redo');assert.equal(node(s,20).parent,6);
  await page.getByRole('button',{name:'Reset create'}).click();
  await start(400,120);await move(440,160);s=await snap('create-preview');assert.equal(s.history,0);assert.equal(s.active,true);
  await page.mouse.up();s=await snap('create-release');assert.equal(s.history,1);assert.equal(node(s,20).parent,3);
  await page.getByRole('button',{name:'Reset create'}).click();
  await start(400,120);await move(440,160);await page.keyboard.press('Escape');await page.mouse.up();
  s=await snap('create-escape');assert.equal(s.history,0);assert.equal(s.feedback,0);assert.equal(s.active,false);assert.equal(node(s,20),undefined);
  const pointerEvents=receipts.filter(r=>r.dpr===dpr).flatMap(r=>r.log).filter(e=>['press','move','release'].includes(e.kind));
  assert(pointerEvents.length>0&&pointerEvents.every(e=>e.trusted===true));
  await page.screenshot({path:`/tmp/novadraw-e03-web-dpi-${dpr}-20261030.png`});
  console.log(`PASS E03 browser DPR ${dpr}: ${receipts.filter(r=>r.dpr===dpr).length} scenarios, trusted pointer events`);
  await context.close();
 }
 await browser.close();
})().catch(error=>{console.error(error);process.exitCode=1});
