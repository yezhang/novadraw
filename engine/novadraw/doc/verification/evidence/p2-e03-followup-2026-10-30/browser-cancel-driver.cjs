const {chromium}=require('playwright');
const fs=require('fs');
const assert=require('assert/strict');
(async()=>{
 const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});
 const context=await browser.newContext({viewport:{width:1100,height:850},deviceScaleFactor:2,hasTouch:true});
 const page=await context.newPage();await page.goto('http://127.0.0.1:8743');await page.waitForFunction(()=>window.e03);
 const rect=await page.locator('#surface').boundingBox();
 const cdp=await context.newCDPSession(page);
 const result=[];
 for(const creating of [false,true]){
  await page.getByRole('button',{name:creating?'Reset create':'Reset move'}).click();
  const start=creating?[400,120]:[90,85];
  await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:rect.x+start[0],y:rect.y+start[1]}]});
  await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:rect.x+440,y:rect.y+160}]});
  const before=await page.evaluate(()=>window.e03.state);assert.equal(before.active,true);
  await cdp.send('Input.dispatchTouchEvent',{type:'touchCancel',touchPoints:[]});
  await page.waitForFunction(()=>window.e03.state.active===false);
  const receipt=await page.evaluate(()=>window.e03);
  assert.equal(receipt.state.feedback,0);assert.equal(receipt.state.history,0);
  assert(receipt.log.some(e=>e.kind==='cancel'&&e.trusted===true));
  result.push({creating,before,...receipt});
 }
 fs.writeFileSync('/tmp/novadraw-e03-browser-cancel-20261030.json',JSON.stringify(result,null,2));
 console.log('PASS trusted browser touch pointercancel for move and create');
 await browser.close();
})().catch(e=>{console.error(e);process.exitCode=1});
