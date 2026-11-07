import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve('/Users/calibro/Projects/leko-enforcer');

test('interface_renders_pure_minimalist_circle_with_no_external_assets', () => {
  const htmlPath = path.join(ROOT, 'ui/web/index.html');
  const cssPath = path.join(ROOT, 'ui/web/circle.css');
  const jsPath = path.join(ROOT, 'ui/web/app.js');

  assert.ok(fs.existsSync(htmlPath), 'index.html exists');
  assert.ok(fs.existsSync(cssPath), 'circle.css exists');
  assert.ok(fs.existsSync(jsPath), 'app.js exists');

  const html = fs.readFileSync(htmlPath, 'utf8');
  const css = fs.readFileSync(cssPath, 'utf8');

  // AC-001: No external CDN, fonts, or network URLs
  assert.equal(/https?:\/\//i.test(html), false, 'index.html must not load external scripts/stylesheets');
  assert.equal(/@import/i.test(css), false, 'circle.css must not use external @import');
  assert.equal(/url\(/i.test(css), false, 'circle.css must not load external font/image URLs');

  // Verify neutral background and core circle elements
  assert.ok(css.includes('#E2E1DD'), 'Uses target neutral background #E2E1DD');
  assert.ok(html.includes('class="ring-core"'), 'HTML contains the ring core element');
  assert.ok(html.includes('class="circle-stage'), 'HTML contains the circle stage');

  // Measure bundle weight (<50 KB total)
  const totalBytes = fs.statSync(htmlPath).size + fs.statSync(cssPath).size + fs.statSync(jsPath).size;
  assert.ok(totalBytes < 50 * 1024, `Total UI package size (${totalBytes} bytes) is well under 50 KB`);
});
