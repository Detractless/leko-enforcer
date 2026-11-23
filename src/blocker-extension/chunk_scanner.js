// chunk_scanner.js - Chunk-Level Candidate Extraction & Local Lexical Filter

(function () {
  // Pre-filter keywords for erotic/suggestive text to avoid unnecessary model latency
  const LEXICAL_PATTERNS = [
    /\b(?:porn|porno|xxx|nsfw|sex|erotic|boobs|penis|vagina|clitoris|cum|cumming|blowjob|anal|horny|orgasm|nude|nudity|naked|strip|tits|dildo)\b/i,
    /\b(?:fetish|bitch|fuck|slut|whore|pussy|dick|cock|masturbat|masturbation)\b/i
  ];

  function matchesLexicalFilter(text) {
    if (!text || text.length < 10) return false;
    for (const pat of LEXICAL_PATTERNS) {
      if (pat.test(text)) return true;
    }
    return false;
  }

  function extractCandidateBlocks() {
    // Target common discussion/article elements
    const selectors = [
      'p',
      'shreddit-comment',
      'article',
      '[data-testid="comment"]',
      '.comment',
      'li'
    ];

    const elements = document.querySelectorAll(selectors.join(', '));
    const candidateBlocks = [];
    let stableIndex = 0;
    let totalExtracted = 0;

    elements.forEach((el) => {
      // Skip empty or tiny blocks
      const text = (el.innerText || el.textContent || '').trim();
      if (text.length < 15 || text.length > 2500) return;
      
      totalExtracted++;
      stableIndex++;

      // Ensure stable DOM identifier
      let blockId = el.getAttribute('data-wae-block-id');
      if (!blockId) {
        blockId = `wae_blk_${stableIndex}_${Math.random().toString(36).substr(2, 6)}`;
        el.setAttribute('data-wae-block-id', blockId);
      }

      // Check lexical pre-filter
      if (matchesLexicalFilter(text)) {
        candidateBlocks.push({
          id: blockId,
          text: text
        });
      }
    });

    return {
      candidates: candidateBlocks,
      totalExtracted: totalExtracted
    };
  }

  function redactBlocks(flaggedIds, shadowMode = false) {
    if (!flaggedIds || flaggedIds.length === 0) return 0;
    let count = 0;
    flaggedIds.forEach((id) => {
      const el = document.querySelector(`[data-wae-block-id="${id}"]`);
      if (el) {
        count++;
        if (shadowMode) {
          // In shadow mode, apply visual marker without destroying content for operator inspection
          el.style.outline = '2px dashed #f44336';
          el.setAttribute('data-wae-flagged', 'true');
          el.title = '[WAE Shadow Redaction: Flagged by Chunk Classifier]';
        } else {
          // In live mode, remove the node entirely before curtain lift
          el.remove();
        }
      }
    });
    return count;
  }

  window.__WAE_CHUNK_SCANNER__ = {
    extract: extractCandidateBlocks,
    redact: redactBlocks,
    matchesFilter: matchesLexicalFilter
  };
})();
