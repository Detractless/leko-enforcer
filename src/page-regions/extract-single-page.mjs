// Repo-relative page extraction wrapper for the Leko daemon.
// regions.mjs is vendored verbatim; this wrapper uses a dependency-free text
// fallback so the unified host does not require the external collector tree.

import { readFileSync } from 'node:fs';

const input = JSON.parse(readFileSync(0, 'utf8'));
const html = input.html || '';
const titleMatch = html.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
const title = (titleMatch ? titleMatch[1] : '').replace(/<[^>]+>/g, '').trim();

function decode(text) {
  return text
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");
}

let facts = html
  .replace(/<(script|style|noscript|svg)[\s\S]*?<\/\1>/gi, ' ')
  .replace(/<[^>]+>/g, ' ')
  .replace(/\s+/g, ' ')
  .trim();
facts = decode(facts).slice(0, 20000);

const lines = facts.split(/(?<=[.!?])\s+/).filter((line) => line.trim().length > 1);
process.stdout.write(JSON.stringify({
  title: title || input.url || '',
  status: 'fallback_text',
  method: 'fallback_text',
  item_count: lines.length,
  facts: facts,
}) + '\n');
