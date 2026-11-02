// Real browser pointer, keyboard, resize and HTML drag/drop verification.
// HTML drag/drop is reported separately from native operating-system file DnD.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');

const url = process.argv[2] || 'http://127.0.0.1:8743';
const output = process.argv[3];
if (!output) throw new Error('Usage: node verify_browser.cjs <url> <receipt.json>');
const receipts = [];
const persist = () => fs.writeFileSync(output, JSON.stringify(receipts, null, 2));
const node = (state, id) => state.nodes.find(value => value.model === id);

(async () => {
  const browser = await chromium.launch({
    executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless: true,
  });
  try {
    for (const dpr of [1, 2]) {
      const context = await browser.newContext({
        viewport: { width: 1100, height: 850 }, deviceScaleFactor: dpr,
      });
      const page = await context.newPage();
      await page.goto(url);
      await page.waitForFunction(() => window.e03?.state);
      const point = async (x, y) => {
        const bounds = await page.locator('#surface').boundingBox();
        const border = await page.locator('#surface').evaluate(element =>
          [element.clientLeft, element.clientTop]);
        return [bounds.x + border[0] + x, bounds.y + border[1] + y];
      };
      const snapshot = async scenario => {
        const value = await page.evaluate(() => window.e03);
        receipts.push({ scenario, dpr, ...value });
        persist();
        return value.state;
      };
      const reset = async creating => {
        await page.getByRole('button', {
          name: creating ? 'Reset create' : 'Reset move', exact: true,
        }).click();
        await page.locator('#surface').focus();
      };
      const dragStart = async creating => {
        const start = creating ? [400, 120] : [90, 85];
        await page.mouse.move(...await point(...start));
        await page.mouse.down();
        await page.mouse.move(...await point(440, 160), { steps: 12 });
      };
      const emptyHistory = (state, creating) => {
        assert.equal(state.active, false);
        assert.equal(state.history, 0);
        assert.equal(state.builds, 0);
        assert.equal(state.feedback, 0);
        assert.equal(state.faulted, false);
        if (creating) assert.equal(node(state, 20), undefined);
        else assert.equal(node(state, 4).parent, 2);
      };

      await reset(false);
      await page.keyboard.press('n');
      await page.keyboard.press('o');
      await dragStart(false);
      let state = await snapshot('batch-ordered-preview');
      assert.equal(state.history, 0);
      assert.equal(state.feedback, 2);
      await page.mouse.up();
      state = await snapshot('batch-ordered-release');
      assert.equal(state.history, 1);
      assert.equal(state.builds, 1);
      assert.deepEqual(state.child_orders.find(([id]) => id === 3)[1], [4, 5, 8]);
      assert.equal(node(state, 5).bounds[0] - node(state, 4).bounds[0], 80);
      assert.equal(node(state, 5).bounds[1] - node(state, 4).bounds[1], 10);
      await page.getByRole('button', { name: 'Undo', exact: true }).click();
      state = await snapshot('batch-ordered-undo');
      assert.equal(node(state, 4).parent, 2);
      assert.equal(node(state, 5).parent, 2);
      await page.getByRole('button', { name: 'Redo', exact: true }).click();
      state = await snapshot('batch-ordered-redo');
      assert.deepEqual(state.child_orders.find(([id]) => id === 3)[1], [4, 5, 8]);

      for (const creating of [false, true]) {
        const mode = creating ? 'create' : 'move';
        for (const [reason, key] of [
          ['external-revision', 'e'], ['policy-invalidated', 'i'], ['tool-switch', 't'],
        ]) {
          await reset(creating);
          await dragStart(creating);
          state = await snapshot(`${mode}-${reason}-preview`);
          assert.equal(state.active, true);
          await page.keyboard.press(key);
          await page.mouse.up();
          state = await snapshot(`${mode}-${reason}-release`);
          emptyHistory(state, creating);
        }
        await reset(creating);
        await dragStart(creating);
        await page.keyboard.press('z');
        await page.keyboard.press('s');
        await page.setViewportSize({ width: 950, height: 700 });
        await page.waitForFunction(() =>
          window.e03.log.some(event => event.kind === 'resize' && event.state.active));
        state = await snapshot(`${mode}-stationary-resize-preview`);
        const feedback = state.feedback_bounds[0];
        assert.equal(state.active, true);
        assert.equal(state.builds, 0);
        await page.mouse.up();
        state = await snapshot(`${mode}-stationary-resize-release`);
        assert.equal(state.history, 1);
        assert.equal(state.builds, 1);
        assert.equal(state.feedback, 0);
        const visual = state.visuals.find(value => value.model === (creating ? 20 : 4)).bounds;
        visual.forEach((value, index) => assert(Math.abs(value - feedback[index]) < 1e-8));
        await page.setViewportSize({ width: 1100, height: 850 });
      }

      await reset(false);
      const target = await page.locator('#surface').boundingBox();
      await page.locator('#template').dragTo(page.locator('#surface'), {
        targetPosition: { x: 100, y: 240 },
      });
      state = await snapshot('html-template-drop-nested');
      assert.equal(state.history, 1);
      assert.equal(state.builds, 1);
      assert.equal(node(state, 20).parent, 6);
      assert.deepEqual(node(state, 20).bounds, [25, 36, 30, 20]);
      const dragEvents = receipts.at(-1).log.filter(event =>
        ['dragstart', 'drop'].includes(event.kind));
      assert.equal(dragEvents.length, 2);
      assert(dragEvents.every(event => event.trusted === true));
      assert(target.width > 0);
      await page.getByRole('button', { name: 'Undo', exact: true }).click();
      state = await snapshot('html-template-drop-undo');
      assert.equal(node(state, 20), undefined);
      await page.getByRole('button', { name: 'Redo', exact: true }).click();
      state = await snapshot('html-template-drop-redo');
      assert.deepEqual(node(state, 20).bounds, [25, 36, 30, 20]);
      const events = receipts.filter(receipt => receipt.dpr === dpr)
        .flatMap(receipt => receipt.log)
        .filter(event => ['press', 'move', 'release'].includes(event.kind));
      assert(events.length > 0 && events.every(event => event.trusted === true));
      console.log(`PASS E03 expanded browser DPR ${dpr}: ${
        receipts.filter(receipt => receipt.dpr === dpr).length
      } checkpoints`);
      await context.close();
    }
  } finally {
    persist();
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
