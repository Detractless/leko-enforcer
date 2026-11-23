// adapter_runner.js - Declarative & Site-Specific Extraction Adapters

(function () {
  function querySelectorAllDeep(selector, root = document) {
    const results = [];
    const search = (node) => {
      if (!node) return;
      if (node.querySelectorAll) {
        try {
          const matched = node.querySelectorAll(selector);
          for (const m of matched) results.push(m);
        } catch (_) {}
      }
      const all = node.querySelectorAll ? node.querySelectorAll("*") : [];
      for (const el of all) {
        if (el.shadowRoot) search(el.shadowRoot);
      }
    };
    search(root);
    return results;
  }

  function extractDeepText(node) {
    if (!node) return "";
    if (node.nodeType === Node.TEXT_NODE) return node.nodeValue || "";
    let text = "";
    if (node.shadowRoot) {
      for (const child of node.shadowRoot.childNodes) text += " " + extractDeepText(child);
    }
    for (const child of node.childNodes) text += " " + extractDeepText(child);
    return text.replace(/\s+/g, " ").trim();
  }

  function extractRedditContent() {
    const parts = [];
    const subHeader = document.querySelector("shreddit-subreddit-header");
    if (subHeader) {
      const subTitle = subHeader.getAttribute("title") || subHeader.querySelector("h1")?.innerText || "";
      const subDesc = subHeader.querySelector("[slot=\"description\"]")?.innerText || "";
      if (subTitle || subDesc) parts.push(`Subreddit: ${subTitle} ${subDesc}`.trim());
    }

    const post = document.querySelector("shreddit-post, [data-testid=\"post-container\"]");
    if (post) {
      const postTitle = post.getAttribute("post-title") || post.querySelector("h1")?.innerText || "";
      if (postTitle) parts.push(`Post Title: ${postTitle.trim()}`);
      const postBodyEl = post.querySelector("[slot=\"text-body\"], .text-neutral-content, p");
      const postBody = postBodyEl ? extractDeepText(postBodyEl) : "";
      if (postBody) parts.push(`Post Body: ${postBody.slice(0, 1500)}`);
    }

    const comments = querySelectorAllDeep("shreddit-comment, [data-testid=\"comment\"]");
    const commentTexts = [];
    for (const c of comments) {
      if (commentTexts.length >= 10) break;
      const bodyEl = c.querySelector ? c.querySelector("[slot=\"comment\"], .md, p") : null;
      const cText = bodyEl ? extractDeepText(bodyEl) : extractDeepText(c);
      if (cText && cText.length > 15) commentTexts.push(cText.slice(0, 300));
    }
    if (commentTexts.length > 0) parts.push("Comments:\n" + commentTexts.join("\n---\n"));
    return parts.join("\n\n");
  }

  function extractGoogleSearchContent() {
    const parts = [];
    const qParam = new URLSearchParams(window.location.search).get("q") || "";
    const searchInput = document.querySelector("input[name=\"q\"], textarea[name=\"q\"]");
    const query = qParam || (searchInput ? searchInput.value : "");
    if (query) parts.push(`Search query: "${query}"`);

    const kp = document.querySelector("[data-attrid=\"description\"], #rhs .kno-rdesc");
    if (kp) {
      const kpText = (kp.innerText || "").replace(/\s+/g, " ").trim();
      if (kpText) parts.push(`Knowledge panel reference: ${kpText.slice(0, 400)}`);
    }

    const containers = document.querySelectorAll("#search .g, #rso .MjjYud, #rso > div");
    const extractedResults = [];
    for (const container of containers) {
      const h3 = container.querySelector("h3");
      if (!h3) continue;
      const titleText = (h3.innerText || "").replace(/\s+/g, " ").trim();
      if (!titleText || titleText.length < 3) continue;
      const snip = container.querySelector("[data-sncf], .VwiC3b, .IsZvec, .yXK7lf");
      const snipText = snip ? (snip.innerText || "").replace(/\s+/g, " ").trim() : "";
      extractedResults.push(snipText ? `- ${titleText} — ${snipText.slice(0, 200)}` : `- ${titleText}`);
      if (extractedResults.length >= 5) break;
    }
    if (extractedResults.length > 0) parts.push("Top search results:\n" + extractedResults.join("\n"));
    return parts.join("\n\n");
  }

  function isRedditReady() {
    const isThread = /\/comments\//i.test(window.location.pathname);
    if (isThread) {
      const post = document.querySelector("shreddit-post, [data-testid=\"post-container\"]");
      const postBody = post ? post.querySelector("[slot=\"text-body\"], .text-neutral-content, p") : null;
      const comments = querySelectorAllDeep("shreddit-comment, [data-testid=\"comment\"]");
      if (postBody || comments.length > 0) return true;
      const words = (document.body ? document.body.innerText : "").trim().split(/\s+/).filter(Boolean);
      return words.length >= 35;
    }
    const posts = document.querySelectorAll("shreddit-post, [data-testid=\"post-container\"], .Post, article");
    if (posts.length > 0) return true;
    const bodyWords = (document.body ? document.body.innerText : "").trim().split(/\s+/).filter(Boolean);
    return bodyWords.length >= 25;
  }

  window.__WAE_ADAPTER_RUNNER__ = {
    querySelectorAllDeep,
    extractDeepText,
    extractRedditContent,
    extractGoogleSearchContent,
    isRedditReady
  };
})();
