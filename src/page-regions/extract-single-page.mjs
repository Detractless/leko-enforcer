import { readFileSync } from 'node:fs';
import { JSDOM } from '../../../../wae-source-reference/collector/node_modules/jsdom/lib/api.js';
import { extract } from '../../../../wae-source-reference/collector/src/extractor/index.ts';
import { renderPool } from '../../../../wae-source-reference/collector/src/describe/facts.ts';
import { context } from '../../../../wae-source-reference/collector/test/extractor/helpers.ts';
import { productRegions, regionFacts } from './regions.mjs';

const input = JSON.parse(readFileSync(0, 'utf8'));
let targetUrl = input.url || 'https://example.com';
if (!targetUrl.startsWith('http://') && !targetUrl.startsWith('https://')) {
  targetUrl = 'https://example.com';
}
const dom = new JSDOM(input.html, { url: targetUrl });
const ctx = context(targetUrl);
const record = extract(dom.window.document, ctx, { adapters: [] });
const pool = renderPool([record])[0];
const regions = productRegions(dom.window.document);
const facts = regions.status === 'separate_items' ? regionFacts(regions, pool.page.text) : pool.page.text;

const result = {
  title: record.title || dom.window.document.title || '',
  status: regions.status,
  method: regions.method || 'fallback_text',
  item_count: regions.item_count || 0,
  facts,
};

dom.window.close();
process.stdout.write(JSON.stringify(result) + '\n');
