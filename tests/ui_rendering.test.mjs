import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve('/Users/calibro/Projects/leko-enforcer');

test('interface_renders_pure_minimalist_circle_with_no_external_assets', () => {
  const htmlPath = path.join(ROOT, 'ui/web/index.html');
  const cssPath = path.join(ROOT, 'ui/web/styles/circle.css');
  const tokenPath = path.join(ROOT, 'ui/web/styles/tokens.css');
  const jsPath = path.join(ROOT, 'ui/web/app.js');
  const controllerPath = path.join(ROOT, 'ui/web/circle-controller.js');

  assert.ok(fs.existsSync(htmlPath), 'index.html exists');
  assert.ok(fs.existsSync(cssPath), 'circle.css exists');
  assert.ok(fs.existsSync(tokenPath), 'shared design tokens exist');
  assert.ok(fs.existsSync(jsPath), 'app.js exists');
  assert.ok(fs.existsSync(controllerPath), 'controller module exists');

  const html = fs.readFileSync(htmlPath, 'utf8');
  const css = fs.readFileSync(cssPath, 'utf8');
  const tokens = fs.readFileSync(tokenPath, 'utf8');

  // AC-001: No external CDN, fonts, or network URLs
  assert.equal(/https?:\/\//i.test(html), false, 'index.html must not load external scripts/stylesheets');
  assert.equal(/@import/i.test(css + tokens), false, 'stylesheets must not use external @import');
  assert.equal(/url\(/i.test(css + tokens), false, 'stylesheets must not load external font/image URLs');

  // Verify neutral background and core circle elements
  assert.ok(tokens.includes('#E2E1DD'), 'Uses target neutral background #E2E1DD');
  assert.ok(html.includes('<script type="module" src="circle-controller.js"></script>'));
  assert.ok(html.includes('class="ring-core"'), 'HTML contains the ring core element');
  assert.ok(html.includes('class="circle-stage'), 'HTML contains the circle stage');

  // Measure bundle weight (<50 KB total)
  const totalBytes = fs.statSync(htmlPath).size + fs.statSync(cssPath).size + fs.statSync(tokenPath).size + fs.statSync(jsPath).size + fs.statSync(controllerPath).size;
  assert.ok(totalBytes < 50 * 1024, `Total UI package size (${totalBytes} bytes) is well under 50 KB`);
});
