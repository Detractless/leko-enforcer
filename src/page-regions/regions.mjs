// Extract separate listing items without asserting a page type.
const clean = (value) => (value ?? '').replace(/\s+/g, ' ').trim().slice(0, 180);
const EXCLUDED = 'nav,header,footer,aside,[role="navigation"],[aria-hidden="true"]';
const HEADING_EXCLUDED = 'nav,aside,footer,[role="navigation"]';
const BLOCKED = /(?:^|[\s_-])(nav|menu|breadcrumb|sidebar|toc|toctree|contents|pagination|related|recommend|carousel|distributors?|download|history|revision|metadata|formats)(?:$|[\s_-])/i;
const ITEM_MARKER = /(?:^|[\s_-])(story|post|item|card|entry|result|product|book|article|listing|tile)(?:$|[\s_-])/i;
const PRICE = /(?:Rs\.?|USD|US\$|\$|\u20ac|\u00a3|\u00a5)\s?\d[\d,.]*/i;
const READ_MORE = /full (story|article)|read (the )?(full|more)|continue reading|read more/i;
const ACTION = /^(add to cart|view product|buy now|details|more|read more)$/i;
const CARD = 'shreddit-post, [class*="post"], [class*="card"], [class*="entry"], [class*="result"], [itemtype], [typeof]';
const AD_CARD = 'shreddit-ad-post, [class*="promoted"], [class*="sponsored"], [data-testid*="promoted"], [data-testid*="ad-slot"]';
const TITLEISH = /(?:^|[\s_-])(title|headline)(?:$|[\s_-])/i;
const CARD_TAG = /^(DIV|ARTICLE|LI|SECTION|FIGURE|TR|SHREDDIT-POST)$/;
const AUTHOR_SCOPE = '[itemprop="author"], [itemtype$="/Person"], [itemtype$="/Organization"], [property="schema:author"], [typeof$="Person"], [typeof$="Organization"], [class*="author"], [class*="byline"]';
function blocked(el) {
  return !!el.closest?.(EXCLUDED) || blockedNear(el, Infinity);
}
function blockedNear(el, levels = 3) {
  let node = el;
  for (let i = 0; node && node.tagName !== 'BODY' && i < levels; i++, node = node.parentElement) {
    if (BLOCKED.test(`${node.getAttribute?.('class') || ''} ${node.id || ''}`)) return true;
  }
  return false;
}

function titled(anchor) {
  if (!anchor) return null;
  const title = clean(anchor.getAttribute('title') || anchor.textContent);
  const href = anchor.getAttribute('href');
  return title && href ? { title, href } : null;
}
function itemLink(node) {
  const prop = node.querySelector('[itemprop="name"], [property="schema:name"]');
  const propAnchor = prop?.closest('a[href]') ?? (prop?.getAttribute('href') ? prop : null);
  if (prop && propAnchor && !prop.closest(AUTHOR_SCOPE)) {
    const entry = { title: clean(prop.getAttribute('title') || prop.textContent), href: propAnchor.getAttribute('href') };
    if (entry.title && entry.href) return entry;
  }
  const headingLinks = [...node.querySelectorAll('h1 a[href], h2 a[href], h3 a[href], h4 a[href], .titleline a[href]')];
  if (headingLinks.length) {
    headingLinks.sort((a, b) => clean(b.getAttribute('title') || b.textContent).length
      - clean(a.getAttribute('title') || a.textContent).length);
    const entry = titled(headingLinks[0]);
    if (entry) return entry;
  }
  const headingEl = [...node.querySelectorAll('h1,h2,h3,h4')]
    .find((heading) => clean(heading.textContent).length >= 8);
  if (headingEl) {
    const anchor = headingEl.closest('a[href]') ?? headingEl.querySelector('a[href]') ?? node.querySelector('a[href]');
    if (anchor) return { title: clean(headingEl.textContent), href: anchor.getAttribute('href') };
  }
  const longest = [...node.querySelectorAll('a[href]')].map((anchor) => titled(anchor))
    .filter((entry) => entry && entry.title.length >= 12)
    .sort((a, b) => b.title.length - a.title.length)[0];
  return longest ?? null;
}
function uniqueEntries(nodes) {
  const seen = new Set();
  const items = [];
  for (const node of nodes) {
    const entry = itemLink(node);
    if (!entry || seen.has(entry.href)) continue;
    seen.add(entry.href);
    items.push(entry);
  }
  return items;
}
function itemMarked(node) {
  return node.hasAttribute('typeof') || node.hasAttribute('itemtype')
    || ITEM_MARKER.test(node.getAttribute('class') || '');
}
function commonParent(root, nodes, minimum) {
  for (let parent = nodes[0]?.parentElement; parent && root.contains(parent); parent = parent.parentElement) {
    const contained = nodes.filter((node) => parent.contains(node));
    const branches = [...parent.children].filter((child) => contained.some((node) => child.contains(node)));
    if (contained.length >= minimum && branches.length >= minimum) return { parent, contained };
  }
  return null;
}

function nearestHeading(root, first) {
  const h1s = [...root.querySelectorAll('h1')].filter((heading) => !heading.closest(HEADING_EXCLUDED));
  const before = first ? h1s.filter((heading) => heading.compareDocumentPosition(first) & 4) : [];
  const chosen = before.at(-1) ?? h1s[0];
  return chosen ? clean(chosen.textContent) : '';
}
function breadcrumbsOf(doc) {
  return [...doc.querySelectorAll('nav[aria-label*="breadcrumb" i] a, [class~="breadcrumb"] a')]
    .map((link) => clean(link.textContent)).filter((label) => label && label.toLowerCase() !== 'home').slice(0, 5);
}
function regionResult(heading, items, method, doc) {
  const kept = items.filter((item) => !/click\?|[?&]pk=|utm_(source|medium)=/i.test(item.href));
  const unique = [...new Map(kept.map((item) => [item.title.toLowerCase().replace(/[^a-z0-9]+/g, ' '), item])).values()];
  return { status: 'separate_items', page_heading: heading, item_count: unique.length,
    breadcrumbs: breadcrumbsOf(doc), items: unique.slice(0, 5), method,
    notice: 'DOM grouping is heuristic; browser visibility and completeness unverified.' };
}
function describeProducts(root, doc, nodes) {
  const group = commonParent(root, nodes, 3);
  if (!group || blocked(group.parent) || group.contained.some(blocked)) return null;
  const items = uniqueEntries(group.contained);
  const headingEl = group.contained[0] ? [...root.querySelectorAll('h1')]
    .filter((heading) => (heading.compareDocumentPosition(group.contained[0]) & 4)
      && !heading.closest(HEADING_EXCLUDED)) : [];
  const nearest = headingEl.at(-1) ?? root.querySelector('h1');
  if (nearest?.closest('article, [itemscope][itemtype$="/Product"]')) return null;
  const heading = clean(nearest?.textContent) || clean(doc.title);
  if (items.length < 3 || !heading) return null;
  return regionResult(heading, items, 'product_cards', doc);
}
function describeArticles(root, doc) {
  const articles = [...root.querySelectorAll('article')].filter((article) => !blocked(article));
  if (articles.length < 5) return null;
  const group = commonParent(root, articles, 5);
  if (!group || blocked(group.parent)) return null;
  const items = uniqueEntries(group.contained);
  if (items.length < 5 || !distinctEnough(items, 0.7)) return null;
  const heading = nearestHeading(root, group.contained[0]) || clean(doc.title);
  if (!heading) return null;
  return regionResult(heading, items, 'article_list', doc);
}
function describeLinked(root, doc) {
  const groups = [];
  for (const list of root.querySelectorAll('ul,ol')) {
    if (blocked(list)) continue;
    const nodes = [...list.children].filter((child) => child.tagName === 'LI');
    if (nodes.length < 5) continue;
    if (nodes.filter(itemMarked).length < nodes.length * 0.6) continue;
    const items = uniqueEntries(nodes);
    if (items.length < 5) continue;
    if (medianTitleLength(items) < 12) continue;
    groups.push({ items, method: 'linked_list' });
  }
  for (const table of root.querySelectorAll('table')) {
    if (blocked(table)) continue;
    const nodes = [...table.querySelectorAll('tr')].filter((row) =>
      row.querySelector('.titleline a[href], td.title a[href]'));
    if (nodes.length < 5) continue;
    const items = uniqueEntries(nodes);
    if (items.length < 5) continue;
    groups.push({ items, method: 'title_table' });
  }
  if (!groups.length) return null;
  groups.sort((a, b) => b.items.length - a.items.length);
  const chosen = groups[0];
  const heading = nearestHeading(root) || clean(doc.title);
  if (!heading) return null;
  return regionResult(heading, chosen.items, chosen.method, doc);
}
const PRICE_HINT = '[class*="price"], [id*="price"], [itemprop="price"]';
const NUMERIC = /^[\d.,\s]+$/;
const priceHinted = (el) => el.matches(PRICE_HINT) || !!el.querySelector(PRICE_HINT);
function priceCardTitle(card) {
  const usable = (text) => text.length >= 3 && text.length <= 120 && /[A-Za-z]/.test(text)
    && !NUMERIC.test(text) && !PRICE.test(text) && !ACTION.test(text);
  const anchors = [...card.querySelectorAll('a[href]')].filter((el) => !el.closest(PRICE_HINT))
    .map((el) => clean(el.textContent)).filter((text) => text.length >= 8 && usable(text)
      && !/^see all\b/i.test(text));
  if (anchors.length) return anchors.sort((a, b) => b.length - a.length)[0];
  const candidates = [...card.querySelectorAll('p,span,h1,h2,h3,h4,strong,b,a')]
    .filter((el) => !el.querySelector('*') && !el.closest(PRICE_HINT)).map((el) => clean(el.textContent))
    .filter(usable);
  return candidates.sort((a, b) => a.length - b.length)[0] ?? '';
}
function describePriceCards(root, doc) {
  const candidates = [...root.querySelectorAll('div,li,article,figure')].filter((el) =>
    !blocked(el) && (PRICE.test(clean(el.textContent)) || priceHinted(el)) && el.querySelector('a[href]'));
  const inner = candidates.filter((el) => !candidates.some((other) => other !== el && el.contains(other)));
  const chosen = [...groupByKind(inner).values()].filter((nodes) => nodes.length >= 4)
    .sort((a, b) => b.length - a.length)[0];
  if (!chosen) return null;
  const group = commonParent(root, chosen, 4);
  if (!group || blocked(group.parent)) return null;
  const items = [];
  const seen = new Set();
  for (const card of group.contained) {
    const title = priceCardTitle(card);
    const links = [...card.querySelectorAll('a[href]')].filter((anchor) =>
      !(anchor.getAttribute('href') || '').startsWith('#'));
    const href = links[0]?.getAttribute('href');
    if (!title || !href || seen.has(href)) continue;
    seen.add(href);
    items.push({ title, href });
  }
  if (items.length < 4) return null;
  const heading = nearestHeading(root, group.contained[0]) || clean(doc.title);
  if (!heading) return null;
  return regionResult(heading, items, 'price_cards', doc);
}
function adCard(card) {
  return !!card.closest(AD_CARD) || /^(ad|promoted|sponsored)\b/i.test(clean(card.textContent || ''));
}
function feedCardish(card) {
  if (!CARD_TAG.test(card.tagName)) return false;
  return card.matches('shreddit-post, [class*="post"], [class*="card"], [class*="entry"], [class*="result"]')
    || card.hasAttribute('itemtype') || card.hasAttribute('typeof')
    || !!card.getAttribute('post-title') || !!card.getAttribute('data-title');
}
function cardTitle(card) {
  const anchors = [...card.querySelectorAll('a[href]')]
    .filter((a) => !a.closest(EXCLUDED) && !(a.getAttribute('href') || '').startsWith('#'));
  const preferred = anchors.find((a) => a.getAttribute('slot') === 'title')
    ?? anchors.find((a) => TITLEISH.test(a.getAttribute('class') || ''));
  const entry = titled(preferred);
  if (entry && entry.title.length >= 12 && !ACTION.test(entry.title)) return entry;
  const attrTitle = clean(card.getAttribute('post-title') || card.getAttribute('data-title') || '');
  if (attrTitle.length >= 12) {
    const match = anchors.find((a) => clean(a.textContent) === attrTitle);
    if (match) return { title: attrTitle, href: match.getAttribute('href') };
  }
  const candidates = anchors.map(titled)
    .filter((e) => e && e.title.length >= 12 && !ACTION.test(e.title) && !PRICE.test(e.title));
  return candidates.sort((a, b) => b.title.length - a.title.length)[0] ?? null;
}
function groupByKind(nodes) {
  const kinds = new Map();
  for (const node of nodes) {
    const first = (node.getAttribute('class') || '').split(/\s+/).filter(Boolean)[0] || '';
    const kind = `${node.tagName}.${first}`;
    if (!kinds.has(kind)) kinds.set(kind, []);
    kinds.get(kind).push(node);
  }
  return kinds;
}

const medianTitleLength = (items) => items.map((item) => item.title.length)
  .sort((a, b) => a - b)[Math.floor(items.length / 2)] ?? 0;
function lowestAncestor(root, nodes, minimum) {
  for (let parent = nodes[0]?.parentElement; parent && root.contains(parent); parent = parent.parentElement) {
    const contained = nodes.filter((node) => parent.contains(node));
    if (contained.length >= minimum) return { parent, contained };
  }
  return null;
}
function describeFeedCards(root, doc) {
  const cards = [];
  const seenCards = new Set();
  for (const anchor of root.querySelectorAll('a[href]')) {
    if (anchor.closest(EXCLUDED)) continue;
    const href = anchor.getAttribute('href') || '';
    if (href.startsWith('#') || anchor.closest(AD_CARD)) continue;
    let card = anchor.closest(CARD);
    if (card === anchor) card = anchor.parentElement?.closest(CARD) ?? null;
    if (!card || !card.contains(anchor) || !root.contains(card) || seenCards.has(card)
      || blockedNear(card) || !feedCardish(card) || adCard(card)) continue;
    seenCards.add(card);
    cards.push(card);
  }
  const groups = [...groupByKind(cards).values()].filter((nodes) => nodes.length >= 5)
    .sort((a, b) => b.length - a.length);
  for (const cards of groups) {
    const group = lowestAncestor(root, cards, 5);
    if (!group) continue;
    const items = [];
    const seen = new Set();
    for (const card of group.contained) {
      if (adCard(card)) continue;
      const entry = cardTitle(card);
      if (!entry || seen.has(entry.href)) continue;
      seen.add(entry.href);
      items.push(entry);
    }
    if (items.length < 5) continue;
    if (medianTitleLength(items) < 12) continue;
    if (!distinctEnough(items)) continue;
    const heading = nearestHeading(root, group.contained[0]) || clean(doc.title);
    if (!heading) continue;
    return regionResult(heading, items, 'feed_cards', doc);
  }
  return null;
}
function headingAnchor(heading) {
  const usable = (anchor) => {
    const href = anchor?.getAttribute('href');
    return !!href && !href.startsWith('#');
  };
  const own = heading.querySelector('a[href]');
  if (usable(own)) return own;
  for (let node = heading.nextSibling; node; node = node.nextSibling) {
    if (node.nodeType === 1 && /^H[1-4]$/.test(node.tagName)) return null;
    if (node.nodeType !== 1) continue;
    const anchors = [...node.querySelectorAll('a[href]')];
    if (node.matches?.('a[href]')) anchors.unshift(node);
    const preferred = anchors.find((anchor) => usable(anchor) && READ_MORE.test(clean(anchor.textContent)));
    if (preferred) return preferred;
  }
  return null;
}
function headingTitle(heading) {
  const leaves = [...heading.querySelectorAll('*')].filter((el) => !el.querySelector('*'));
  const direct = leaves.length ? [...heading.childNodes].filter((n) => n.nodeType === 3) : [heading];
  return [...leaves, ...direct].map((el) => clean(el.textContent))
    .filter((text) => text.length >= 12 && /[A-Za-z]/.test(text))
    .sort((a, b) => b.length - a.length)[0] ?? '';
}
function distinctEnough(items, floor = 0.6) {
  return new Set(items.map((i) => i.title.toLowerCase().replace(/[^a-z0-9]+/g, ' '))).size
    >= items.length * floor;
}
function describeHeadingCards(root, doc) {
  const entries = [];
  for (const heading of root.querySelectorAll('h2,h3')) {
    const around = heading.closest('a[href]');
    const anchor = [heading.querySelector('a[href]'), around, headingAnchor(heading)]
      .find((a) => a && !(a.getAttribute('href') || '').startsWith('#'));
    if (!anchor || heading.closest(HEADING_EXCLUDED) || blockedNear(heading, 2)) continue;
    const title = headingTitle(heading).replace(/^\[[^\]]{1,8}\]?\s*|^\]\s*/, '');
    const card = around ?? heading.parentElement;
    if (title.length < 16 || (around && clean(card?.textContent).length > 700)) continue;
    entries.push({ title, href: anchor.getAttribute('href'), heading });
  }
  let parent = entries.length >= 5 && lowestAncestor(root, entries.map((e) => e.heading), 5)?.parent;
  let contained = parent && entries.filter((e) => parent.contains(e.heading));
  for (let p = parent?.parentElement; p && root.contains(p); p = p.parentElement) {
    const next = entries.filter((e) => p.contains(e.heading));
    if (next.length > contained.length) [parent, contained] = [p, next];
  }
  if (!parent || !contained || contained.length < 5) return null;
  const items = [];
  const seen = new Set();
  for (const entry of contained) {
    if (seen.has(entry.href)) continue;
    seen.add(entry.href);
    items.push({ title: entry.title, href: entry.href });
  }
  if (items.length < 5 || !distinctEnough(items) || medianTitleLength(items) < 20) return null;
  const heading = nearestHeading(root, contained[0].heading) || clean(doc.title);
  return heading ? regionResult(heading, items, 'heading_cards', doc) : null;
}

export function productRegions(doc) {
  const root = doc.querySelector('main, [role="main"]') ?? doc.body;
  if (!root) return { status: 'no_regions', items: [] };
  const productNodes = [...root.querySelectorAll('[itemscope][itemtype$="/Product"]')]
    .filter((el) => !el.parentElement?.closest('[itemscope][itemtype$="/Product"]') && !blocked(el));
  if (productNodes.length >= 3) {
    const productGroup = describeProducts(root, doc, productNodes);
    if (productGroup) return productGroup;
  }
  return describeArticles(root, doc) ?? describeLinked(root, doc)
    ?? describePriceCards(root, doc) ?? describeHeadingCards(root, doc)
    ?? describeFeedCards(root, doc) ?? { status: 'no_regions', items: [] };
}

export function regionFacts(regions, fallbackFacts) {
  if (regions.status !== 'separate_items') return fallbackFacts;
  return [`Page title: ${regions.page_heading}`,
    ...(regions.breadcrumbs?.length ? [`Category labels: ${regions.breadcrumbs.join(' > ')}`] : []),
    ...regions.items.map((item, index) => `Item ${index + 1}: ${item.title}`)].join('\n');
}
