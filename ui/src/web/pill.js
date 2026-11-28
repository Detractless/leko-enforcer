(function () {
  const pill = document.getElementById('pill');
  const bars = document.getElementById('bars');
  const label = document.getElementById('label');
  const card = document.getElementById('card');
  const cardClose = document.getElementById('card-close');
  const cardUser = document.getElementById('card-user');
  const cardQuery = document.getElementById('card-query');
  const cardBody = document.getElementById('card-body');
  const cardTable = document.getElementById('card-table');
  const cardHead = document.getElementById('card-head');
  const cardRows = document.getElementById('card-rows');

  const WIDTHS = {
    idle: 54, timer: 68, blocked: 92, listening: 112, held: 112,
    thinking: 82, speaking: 112, stopped: 112, error: 92
  };
  const BAR_COUNTS = { listening: 7, held: 7, speaking: 7, stopped: 7 };
  const REACTIVE_STATES = { listening: true, held: true, speaking: true };
  const WEIGHTS_7 = [0.56, 0.81, 0.92, 1.0, 0.92, 0.81, 0.56];

  let state = 'idle', timerText = '', localTimer = null, hovered = false;
  let targetLevel = 0, smoothLevel = 0, lastLevelAt = 0;
  let cardActive = false, cardHovered = false, cardSpeaking = false, cardTextDone = true, cardSpeechDone = true;
  let cardCollapseTimer = null, cardSpeechGraceTimer = null;
  let streamTokens = [], streamRawText = '', streamMode = 'word', streamIdx = 0, streamTimer = null, streamStartTimer = null;

  for (let i = 0; i < 7; i += 1) {
    const bar = document.createElement('span');
    bar.className = 'bar';
    bar.style.setProperty('--bar-index', i);
    bars.appendChild(bar);
  }
  const barEls = Array.from(bars.children);

  function clearCardTimers() {
    clearTimeout(cardCollapseTimer); cardCollapseTimer = null;
    clearTimeout(cardSpeechGraceTimer); cardSpeechGraceTimer = null;
    clearTimeout(streamTimer); streamTimer = null;
    clearTimeout(streamStartTimer); streamStartTimer = null;
  }

  function canDismissOutside() { return cardActive && cardSpeechDone && cardTextDone; }
  function scheduleCollapse(ms) { clearTimeout(cardCollapseTimer); cardCollapseTimer = setTimeout(function () { if (!cardHovered) { dismissCard(); } }, ms); }
  function maybeScheduleCollapse(ms) { if (canDismissOutside() && !cardHovered) { scheduleCollapse(ms); } }

  function tokenWeight(token) {
    const t = token.trim();
    const letters = t.replace(/[^a-zA-Z]/g, '').length;
    const digits = t.replace(/[^0-9]/g, '').length;
    const pause = /[.!?]$/.test(t) ? 4.2 : /[,;:]$/.test(t) ? 2.0 : 0;
    return 1.6 + letters + digits * 2.2 + pause;
  }

  function tokenizeStream(text) {
    return ((text || '').match(/\S+\s*/g) || []).map(function (t) { return { text: t, weight: tokenWeight(t) }; });
  }

  function resolveWordStarts(rem, startedAt, durationMs, timingsMs) {
    if (Array.isArray(timingsMs) && timingsMs.length > 0 && streamIdx === 0) {
      return rem.map(function (_, idx) {
        if (timingsMs.length === rem.length) { return startedAt + timingsMs[idx]; }
        const src = (rem.length > 1 ? idx / (rem.length - 1) : 0) * (timingsMs.length - 1);
        const lo = Math.floor(src), hi = Math.min(timingsMs.length - 1, lo + 1);
        return startedAt + timingsMs[lo] + (timingsMs[hi] - timingsMs[lo]) * (src - lo);
      });
    }
    const totalWeight = rem.reduce(function (sum, tok) { return sum + tok.weight; }, 0) || 1;
    const targetMs = durationMs > 0 ? Math.max(250, durationMs) : totalWeight * 48;
    let acc = 0;
    return rem.map(function (tok) { const ms = (acc / totalWeight) * targetMs; acc += tok.weight; return startedAt + ms; });
  }

  function resolveDeadlines(rem, startedAt, durationMs, timingsMs) {
    const wordStarts = resolveWordStarts(rem, startedAt, durationMs, timingsMs);
    if (streamMode !== 'letter' || streamIdx !== 0) { return wordStarts; }
    const chars = [], deadlines = [];
    for (let i = 0; i < rem.length; i += 1) {
      const wStart = wordStarts[i];
      const wEnd = i + 1 < rem.length
        ? Math.max(wStart + 25, wordStarts[i + 1])
        : Math.max(wStart + Math.max(90, rem[i].text.length * 42), startedAt + (durationMs || 0));
      const parts = Array.from(rem[i].text);
      parts.forEach(function (ch, c) {
        chars.push({ text: ch, weight: 1 });
        deadlines.push(wStart + (c / Math.max(1, parts.length)) * (wEnd - wStart));
      });
    }
    streamTokens = chars;
    return deadlines;
  }

  function startTokenStream(durationMs, timingsMs) {
    clearTimeout(streamStartTimer); streamStartTimer = null;
    clearTimeout(streamTimer); streamTimer = null;
    if (streamIdx >= streamTokens.length) { cardTextDone = true; maybeScheduleCollapse(3500); return; }
    const rem = streamTokens.slice(streamIdx);
    const deadlines = resolveDeadlines(rem, performance.now(), durationMs, timingsMs);
    const baseIdx = streamIdx;
    function tick() {
      streamTimer = null;
      if (!cardActive) { return; }
      const now = performance.now();
      while (streamIdx < streamTokens.length && deadlines[streamIdx - baseIdx] <= now + 4) { streamIdx += 1; }
      cardBody.textContent = streamTokens.slice(0, streamIdx).map(function (t) { return t.text; }).join('');
      if (streamIdx >= streamTokens.length) { cardTextDone = true; maybeScheduleCollapse(3500); return; }
      streamTimer = setTimeout(tick, Math.max(8, Math.round(deadlines[streamIdx - baseIdx] - now)));
    }
    tick();
  }

  function streamText(text, durationMs, timingsMs, mode) {
    clearTimeout(streamTimer); streamTimer = null; clearTimeout(streamStartTimer); streamStartTimer = null;
    streamRawText = text || '';
    streamMode = mode || (window.__leko_config && window.__leko_config.revealMode) || 'word';
    streamTokens = tokenizeStream(streamRawText); streamIdx = 0; cardBody.textContent = '';
    if (!streamTokens.length) { cardTextDone = true; return; }
    cardTextDone = false;
    if (durationMs > 0 || (Array.isArray(timingsMs) && timingsMs.length > 0)) { startTokenStream(durationMs, timingsMs); return; }
    streamStartTimer = setTimeout(function () { streamStartTimer = null; if (cardActive && !cardTextDone && !streamTimer) { startTokenStream(0); } }, 20);
  }

  function finishStreaming() {
    clearTimeout(streamStartTimer); streamStartTimer = null;
    if (cardTextDone) { return; }
    if (streamIdx >= streamTokens.length) { cardTextDone = true; return; }
    startTokenStream(Math.min(260, (streamTokens.length - streamIdx) * (streamMode === 'letter' ? 12 : 45)));
  }

  function renderTable(columns, rows) {
    cardHead.innerHTML = ''; cardRows.innerHTML = '';
    if (!Array.isArray(rows) || rows.length === 0) { cardTable.hidden = true; return; }
    if (Array.isArray(columns) && columns.length > 0) {
      const tr = document.createElement('tr');
      columns.forEach(function (col) { const th = document.createElement('th'); th.textContent = String(col); tr.appendChild(th); });
      cardHead.appendChild(tr);
    }
    rows.forEach(function (row) {
      if (!Array.isArray(row)) { return; }
      const tr = document.createElement('tr');
      row.forEach(function (cell) { const td = document.createElement('td'); td.textContent = String(cell); tr.appendChild(td); });
      cardRows.appendChild(tr);
    });
    cardTable.hidden = false;
  }

  function openQueryCard(query) {
    clearCardTimers(); cardActive = true; cardSpeaking = false; cardSpeechDone = false;
    card.classList.add('is-thinking', 'has-query'); cardQuery.textContent = query; cardUser.hidden = false;
    cardBody.textContent = 'Thinking...'; cardTable.hidden = true;
    document.body.classList.add('has-card'); card.hidden = false;
  }

  function openCard(event) {
    clearCardTimers(); cardActive = true; cardSpeaking = state === 'speaking'; cardSpeechDone = false;
    card.classList.remove('is-thinking');
    const q = event.query || cardQuery.textContent;
    if (q) { card.classList.add('has-query'); cardQuery.textContent = q; cardUser.hidden = false; }
    else { card.classList.remove('has-query'); cardUser.hidden = true; }
    document.body.classList.add('has-card'); card.hidden = false;
    streamText(event.text || '', event.duration_ms || 0, event.word_timings_ms, event.reveal_mode);
    renderTable(event.columns || [], event.rows || []);
    if (!cardSpeaking) {
      cardSpeechGraceTimer = setTimeout(function () { cardSpeechGraceTimer = null; cardSpeechDone = true; maybeScheduleCollapse(6000); }, 400);
    }
  }

  function dismissCard() {
    if (!cardActive) { return; }
    clearCardTimers();
    cardActive = cardHovered = cardSpeaking = false; cardTextDone = cardSpeechDone = true;
    card.hidden = true; card.classList.remove('is-thinking', 'has-query'); cardQuery.textContent = ''; cardUser.hidden = true;
    cardClose.classList.remove('is-hovered'); document.body.classList.remove('has-card'); post('card_dismiss');
  }

  function render() {
    pill.className = 'leko-pill state-' + state + (hovered ? ' is-hovered' : '');
    const count = BAR_COUNTS[state] || 0;
    barEls.forEach((bar, index) => {
      bar.style.display = index < count ? 'block' : 'none';
      if (!REACTIVE_STATES[state]) { bar.style.height = '2.5px'; bar.style.opacity = '0.9'; }
    });
    if (state === 'timer') { label.textContent = timerText; }
    else if (state === 'blocked' || state === 'error') { label.textContent = state === 'blocked' ? 'Blocked' : 'Error'; }
    else { label.textContent = ''; }
  }

  function animateBars(now) {
    if (REACTIVE_STATES[state]) {
      if (now - lastLevelAt > 240) { targetLevel = 0; }
      const rate = targetLevel > smoothLevel ? 0.42 : 0.34;
      smoothLevel += (targetLevel - smoothLevel) * rate;
      const t = now / 1000;
      const amp = Math.max(0.05 + 0.03 * Math.sin(t * 2.2), smoothLevel);
      for (let i = 0; i < barEls.length; i += 1) {
        const jitter = smoothLevel > 0.03 ? (0.66 + 0.34 * (0.5 + 0.5 * Math.sin(t * 13 + i * 1.9))) : 1.0;
        const height = 2.5 + amp * 9.0 * WEIGHTS_7[i] * jitter;
        barEls[i].style.height = Math.max(2.5, Math.min(11.5, height)).toFixed(1) + 'px';
        barEls[i].style.opacity = (0.48 + 0.52 * Math.min(1, amp * 1.1)).toFixed(2);
      }
    } else { targetLevel = smoothLevel = 0; }
    requestAnimationFrame(animateBars);
  }
  requestAnimationFrame(animateBars);

  function setState(next, text) {
    if (localTimer) { clearTimeout(localTimer); localTimer = null; }
    state = next;
    if (text !== undefined && next === 'timer') { timerText = text; }
    render();
  }

  function post(action) {
    if (window.ipc && window.ipc.postMessage) { window.ipc.postMessage(JSON.stringify({ action: action })); }
  }

  function restState() {
    return (window.__leko_config && window.__leko_config.quietTimer && timerText) ? 'timer' : 'idle';
  }

  function localFeedback(heldFor) {
    if (heldFor <= 200) { setState(restState()); return; }
    setState('listening');
    localTimer = setTimeout(function () { localTimer = null; if (state === 'listening') { setState('thinking'); } }, 250);
  }

  let pressedAt = 0, localPressed = false, stopPress = false;
  function flashStopped() {
    setState('stopped');
    localTimer = setTimeout(function () { localTimer = null; if (!stopPress && state === 'stopped') { setState(restState()); } }, 1200);
  }
  function releaseHold(force) {
    if (stopPress) { stopPress = false; if (!localTimer && state === 'stopped') { setState(restState()); } return; }
    const heldFor = performance.now() - pressedAt;
    const wasPressed = localPressed;
    localPressed = false;
    if (state !== 'held' && !(force && state === 'listening')) { return; }
    if (wasPressed) {
      post(heldFor <= 200 ? 'ptt_cancel' : 'ptt_end');
      localFeedback(heldFor);
    } else {
      if (force) { post('ptt_end'); }
      setState(restState());
    }
  }

  pill.addEventListener('pointerdown', function (event) {
    try { pill.setPointerCapture(event.pointerId); } catch (_) {}
    if (cardActive) { dismissCard(); }
    if (state === 'thinking' || state === 'speaking') { stopPress = true; localPressed = false; flashStopped(); post('stop'); return; }
    stopPress = false; localPressed = true; pressedAt = performance.now(); setState('held'); post('ptt_start');
  });
  pill.addEventListener('pointerup', function () { releaseHold(false); });
  pill.addEventListener('pointercancel', function () { releaseHold(false); });
  window.addEventListener('pointerup', function () { if (stopPress || localPressed || state === 'held') { releaseHold(false); } });
  window.addEventListener('pointercancel', function () { if (stopPress || localPressed || state === 'held') { releaseHold(false); } });
  pill.addEventListener('contextmenu', function (event) { event.preventDefault(); });

  function hitRect(el, x, y) { const r = el && el.getBoundingClientRect && el.getBoundingClientRect(); return Boolean(r && r.width > 0 && x >= r.left && x <= r.right && y >= r.top && y <= r.bottom); }
  function setCardHover(on) { if (cardHovered === on) { return; } cardHovered = on; if (on) { clearTimeout(cardCollapseTimer); cardCollapseTimer = null; post('card_hover_on'); } else { post('card_hover_off'); maybeScheduleCollapse(2000); } }
  cardClose.addEventListener('click', function (event) { event.stopPropagation(); const stop = state === 'speaking' || state === 'thinking' || cardSpeaking; dismissCard(); if (stop) { stopPress = localPressed = false; flashStopped(); post('stop'); } });
  card.addEventListener('mouseenter', function () { setCardHover(true); });
  card.addEventListener('mouseleave', function () { setCardHover(false); });
  document.addEventListener('keydown', function (event) { if (event.key === 'Escape' && cardActive) { dismissCard(); } });
  document.addEventListener('pointerdown', function (event) { if (canDismissOutside() && !card.contains(event.target) && !pill.contains(event.target)) { dismissCard(); } });
  window.addEventListener('blur', function () { if (stopPress || localPressed || state === 'held') { releaseHold(true); } if (canDismissOutside() && !cardHovered) { dismissCard(); } });

  window.__leko_on_event = function (raw) {
    let event;
    try { event = typeof raw === 'string' ? JSON.parse(raw) : raw; } catch (error) { return; }
    if (event.type === 'audio_level' && typeof event.level === 'number') {
      targetLevel = Math.max(0, Math.min(1, event.level));
      lastLevelAt = performance.now();
      return;
    }
    const rawState = event.state || event.type;
    const next = rawState === 'grace' ? 'listening' : rawState;
    if (next === 'force_release') { releaseHold(true); return; }
    if (next === 'outside_click') { if (canDismissOutside()) { dismissCard(); } return; }
    if (next === 'reply_card') {
      localPressed = false;
      if (state === 'stopped' && (stopPress || localTimer)) { return; }
      if (state !== 'speaking') { setState(restState()); }
      openCard(event);
      return;
    }
    if (next === 'card_dismiss') { dismissCard(); return; }
    if (next === 'thinking') { localPressed = false; setState('thinking'); if (event.query) { openQueryCard(event.query); } return; }
    if (next === 'hover_on' || next === 'hover_off') { hovered = next === 'hover_on'; render(); return; }
    if (next === 'card_cursor') {
      const active = cardActive && !card.hidden;
      if (active && hitRect(cardClose, event.x, event.y)) { cardClose.classList.add('is-hovered'); } else { cardClose.classList.remove('is-hovered'); }
      if (active && card.getBoundingClientRect) { setCardHover(hitRect(card, event.x, event.y)); }
      return;
    }
    if (next === 'stop' || next === 'stopped') { localPressed = false; dismissCard(); flashStopped(); return; }
    if (next === 'blocked' || next === 'error') { localPressed = false; dismissCard(); setState(next); localTimer = setTimeout(function () { setState(restState()); }, 2200); return; }
    if (next === 'timer_start') {
      timerText = event.timer || '';
      if (window.__leko_config && window.__leko_config.quietTimer && (state === 'idle' || state === 'timer')) {
        setState('timer');
      }
      return;
    }
    if (next === 'timer_stop') { timerText = ''; setState('idle'); return; }
    if (next === 'held' && !localPressed) { return; }
    if (localPressed && state === 'held' && next === 'idle') { return; }
    if (state === 'stopped' && (stopPress || localTimer) && (next === 'idle' || next === 'speaking')) { return; }
    if (next === 'speaking') {
      localPressed = false;
      clearTimeout(cardSpeechGraceTimer); cardSpeechGraceTimer = null;
      if (cardActive) {
        cardSpeaking = true; cardSpeechDone = false;
        clearTimeout(cardCollapseTimer); cardCollapseTimer = null;
        if (event.reveal_mode) { streamMode = event.reveal_mode; }
        if (streamIdx === 0 && streamRawText) { streamTokens = tokenizeStream(streamRawText); }
        if (!cardTextDone && (!streamTimer || event.duration_ms > 0 || (event.word_timings_ms && event.word_timings_ms.length))) {
          startTokenStream(event.duration_ms || 0, event.word_timings_ms);
        }
      }
      setState('speaking');
      return;
    }
    if (next === 'idle') {
      localPressed = false;
      if (cardSpeechGraceTimer) { clearTimeout(cardSpeechGraceTimer); cardSpeechGraceTimer = null; }
      if (cardActive) {
        cardSpeaking = false;
        cardSpeechDone = true;
        finishStreaming();
        maybeScheduleCollapse(3500);
      }
      setState(restState());
      return;
    }
    if (WIDTHS[next]) { setState(next, event.timer); }
  };

  render();
})();
