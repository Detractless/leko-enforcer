(function () {
  const pill = document.getElementById('pill');
  const bars = document.getElementById('bars');
  const label = document.getElementById('label');
  const card = document.getElementById('card');
  const cardClose = document.getElementById('card-close');
  const cardBody = document.getElementById('card-body');
  const cardTable = document.getElementById('card-table');
  const cardHead = document.getElementById('card-head');
  const cardRows = document.getElementById('card-rows');

  const WIDTHS = {
    idle: 54, timer: 68, blocked: 92, listening: 112, held: 112,
    thinking: 82, speaking: 112, error: 92
  };
  const BAR_COUNTS = { listening: 7, held: 7, speaking: 7 };
  const WEIGHTS_7 = [0.56, 0.81, 0.92, 1.0, 0.92, 0.81, 0.56];

  let state = 'idle';
  let timerText = '';
  let localTimer = null;
  let hovered = false;
  let targetLevel = 0;
  let smoothLevel = 0;
  let lastLevelAt = 0;
  let cardActive = false;
  let cardHovered = false;
  let cardSpeaking = false;
  let cardTextDone = true;
  let cardSpeechDone = true;
  let cardCollapseTimer = null;
  let cardSpeechGraceTimer = null;
  let typeTimer = null;
  let streamFull = '';
  let streamIdx = 0;

  for (let i = 0; i < 7; i += 1) {
    const bar = document.createElement('span');
    bar.className = 'bar';
    bar.style.setProperty('--bar-index', i);
    bars.appendChild(bar);
  }
  const barEls = Array.from(bars.children);

  function clearCardTimers() {
    if (cardCollapseTimer) { clearTimeout(cardCollapseTimer); cardCollapseTimer = null; }
    if (cardSpeechGraceTimer) { clearTimeout(cardSpeechGraceTimer); cardSpeechGraceTimer = null; }
    if (typeTimer) { clearInterval(typeTimer); typeTimer = null; }
  }

  function canDismissOutside() {
    return cardActive && cardSpeechDone && cardTextDone;
  }

  function scheduleCollapse(ms) {
    if (cardCollapseTimer) { clearTimeout(cardCollapseTimer); }
    cardCollapseTimer = setTimeout(function () { if (!cardHovered) { dismissCard(); } }, ms);
  }

  function maybeScheduleCollapse(ms) {
    if (canDismissOutside() && !cardHovered) { scheduleCollapse(ms); }
  }

  function stepStream(step) {
    streamIdx = Math.min(streamFull.length, streamIdx + step);
    cardBody.textContent = streamFull.slice(0, streamIdx);
    if (streamIdx >= streamFull.length) {
      clearInterval(typeTimer);
      typeTimer = null;
      cardTextDone = true;
      maybeScheduleCollapse(3500);
    }
  }

  function streamText(text) {
    if (typeTimer) { clearInterval(typeTimer); typeTimer = null; }
    streamFull = text || '';
    streamIdx = 0;
    if (!streamFull) { cardBody.textContent = ''; cardTextDone = true; return; }
    cardTextDone = false;
    cardBody.textContent = '';
    typeTimer = setInterval(function () { stepStream(1); }, 62);
  }

  function finishStreaming() {
    if (cardTextDone || !typeTimer) { return; }
    clearInterval(typeTimer);
    typeTimer = setInterval(function () { stepStream(2); }, 16);
  }

  function renderTable(columns, rows) {
    cardHead.innerHTML = '';
    cardRows.innerHTML = '';
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

  function openCard(event) {
    clearCardTimers();
    cardActive = true;
    cardSpeaking = state === 'speaking';
    cardSpeechDone = false;
    document.body.classList.add('has-card');
    card.hidden = false;
    streamText(event.text || '');
    renderTable(event.columns || [], event.rows || []);
    if (!cardSpeaking) {
      cardSpeechGraceTimer = setTimeout(function () {
        cardSpeechGraceTimer = null;
        cardSpeechDone = true;
        maybeScheduleCollapse(6000);
      }, 400);
    }
  }

  function dismissCard() {
    if (!cardActive) { return; }
    clearCardTimers();
    cardActive = false;
    cardHovered = false;
    cardSpeaking = false;
    cardTextDone = true;
    cardSpeechDone = true;
    card.hidden = true;
    document.body.classList.remove('has-card');
    post('card_dismiss');
  }

  function render() {
    pill.className = 'leko-pill state-' + state + (hovered ? ' is-hovered' : '');
    const count = BAR_COUNTS[state] || 0;
    barEls.forEach((bar, index) => {
      bar.style.display = index < count ? 'block' : 'none';
    });
    if (state === 'timer') {
      label.textContent = timerText;
    } else if (state === 'blocked' || state === 'error') {
      label.textContent = state === 'blocked' ? 'Blocked' : 'Error';
    } else {
      label.textContent = '';
    }
  }

  function animateBars(now) {
    if (BAR_COUNTS[state]) {
      if (now - lastLevelAt > 240) {
        targetLevel = 0;
      }
      const rate = targetLevel > smoothLevel ? 0.42 : 0.34;
      smoothLevel += (targetLevel - smoothLevel) * rate;
      const t = now / 1000;
      const breath = 0.05 + 0.03 * Math.sin(t * 2.2);
      const amp = Math.max(breath, smoothLevel);
      for (let i = 0; i < barEls.length; i += 1) {
        const jitter = smoothLevel > 0.03 ? (0.66 + 0.34 * (0.5 + 0.5 * Math.sin(t * 13 + i * 1.9))) : 1.0;
        const height = 2.5 + amp * 9.0 * WEIGHTS_7[i] * jitter;
        barEls[i].style.height = Math.max(2.5, Math.min(11.5, height)).toFixed(1) + 'px';
        barEls[i].style.opacity = (0.48 + 0.52 * Math.min(1, amp * 1.1)).toFixed(2);
      }
    } else {
      targetLevel = 0;
      smoothLevel = 0;
    }
    requestAnimationFrame(animateBars);
  }
  requestAnimationFrame(animateBars);

  function setState(next, text) {
    if (localTimer) {
      clearTimeout(localTimer);
      localTimer = null;
    }
    state = next;
    if (text !== undefined && next === 'timer') {
      timerText = text;
    }
    render();
  }

  function post(action) {
    if (window.ipc && window.ipc.postMessage) {
      window.ipc.postMessage(JSON.stringify({ action: action }));
    }
  }

  function restState() {
    return (window.__leko_config && window.__leko_config.quietTimer && timerText) ? 'timer' : 'idle';
  }

  function localFeedback(heldFor) {
    if (heldFor > 200) {
      setState('thinking');
      return;
    }
    setState(restState());
  }

  let pressedAt = 0;
  let localPressed = false;
  function releaseHold(force) {
    const heldFor = performance.now() - pressedAt;
    const wasPressed = localPressed;
    localPressed = false;
    if (state !== 'held' && !(force && state === 'listening')) {
      return;
    }
    if (wasPressed) {
      if (heldFor <= 200) {
        post('ptt_cancel');
      } else {
        post('ptt_end');
      }
      localFeedback(heldFor);
    } else {
      if (force) {
        post('ptt_end');
      }
      setState(restState());
    }
  }

  pill.addEventListener('pointerdown', function (event) {
    try {
      pill.setPointerCapture(event.pointerId);
    } catch (_) {}
    localPressed = true;
    pressedAt = performance.now();
    setState('held');
    post('ptt_start');
  });

  pill.addEventListener('pointerup', function () { releaseHold(false); });
  pill.addEventListener('pointercancel', function () { releaseHold(false); });
  window.addEventListener('pointerup', function () {
    if (localPressed || state === 'held') { releaseHold(false); }
  });
  window.addEventListener('pointercancel', function () {
    if (localPressed || state === 'held') { releaseHold(false); }
  });
  pill.addEventListener('contextmenu', function (event) {
    event.preventDefault();
  });

  cardClose.addEventListener('click', function (event) {
    event.stopPropagation();
    dismissCard();
  });
  card.addEventListener('mouseenter', function () {
    cardHovered = true;
    if (cardCollapseTimer) { clearTimeout(cardCollapseTimer); cardCollapseTimer = null; }
    post('card_hover_on');
  });
  card.addEventListener('mouseleave', function () {
    cardHovered = false;
    post('card_hover_off');
    maybeScheduleCollapse(2000);
  });
  document.addEventListener('keydown', function (event) {
    if (event.key === 'Escape' && cardActive) { dismissCard(); }
  });
  document.addEventListener('pointerdown', function (event) {
    if (canDismissOutside() && !card.contains(event.target) && !pill.contains(event.target)) { dismissCard(); }
  });
  window.addEventListener('blur', function () {
    if (localPressed || state === 'held') { releaseHold(true); }
    if (canDismissOutside() && !cardHovered) { dismissCard(); }
  });

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
      if (state !== 'speaking') { setState(restState()); }
      openCard(event);
      return;
    }
    if (next === 'card_dismiss') { dismissCard(); return; }
    if (next === 'hover_on' || next === 'hover_off') { hovered = next === 'hover_on'; render(); return; }
    if (next === 'blocked' || next === 'error') {
      localPressed = false;
      dismissCard();
      setState(next);
      localTimer = setTimeout(function () { setState(restState()); }, 2200);
      return;
    }
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
    if (next === 'speaking') {
      localPressed = false;
      if (cardSpeechGraceTimer) { clearTimeout(cardSpeechGraceTimer); cardSpeechGraceTimer = null; }
      if (cardActive) {
        cardSpeaking = true;
        cardSpeechDone = false;
        if (cardCollapseTimer) { clearTimeout(cardCollapseTimer); cardCollapseTimer = null; }
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
