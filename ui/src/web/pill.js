(function () {
  const pill = document.getElementById('pill');
  const bars = document.getElementById('bars');
  const label = document.getElementById('label');
  const card = document.getElementById('card');
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
  let cardCollapseTimer = null;
  let typeTimer = null;

  for (let i = 0; i < 7; i += 1) {
    const bar = document.createElement('span');
    bar.className = 'bar';
    bars.appendChild(bar);
  }
  const barEls = Array.from(bars.children);
  barEls.forEach((bar, index) => {
    bar.style.setProperty('--bar-index', index);
  });

  function clearCardTimers() {
    if (cardCollapseTimer) { clearTimeout(cardCollapseTimer); cardCollapseTimer = null; }
    if (typeTimer) { clearInterval(typeTimer); typeTimer = null; }
  }

  function scheduleCollapse(ms) {
    if (cardCollapseTimer) { clearTimeout(cardCollapseTimer); }
    cardCollapseTimer = setTimeout(function () {
      if (!cardHovered) { dismissCard(); }
    }, ms);
  }

  function streamText(text) {
    if (typeTimer) { clearInterval(typeTimer); typeTimer = null; }
    const full = text || '';
    if (full.length <= 24) {
      cardBody.textContent = full;
      return;
    }
    let idx = 0;
    cardBody.textContent = '';
    typeTimer = setInterval(function () {
      idx = Math.min(full.length, idx + 4);
      cardBody.textContent = full.slice(0, idx);
      if (idx >= full.length) {
        clearInterval(typeTimer);
        typeTimer = null;
      }
    }, 16);
  }

  function renderTable(columns, rows) {
    cardHead.innerHTML = '';
    cardRows.innerHTML = '';
    if (!Array.isArray(rows) || rows.length === 0) {
      cardTable.hidden = true;
      return;
    }
    if (Array.isArray(columns) && columns.length > 0) {
      const tr = document.createElement('tr');
      columns.forEach(function (col) {
        const th = document.createElement('th');
        th.textContent = String(col);
        tr.appendChild(th);
      });
      cardHead.appendChild(tr);
    }
    rows.forEach(function (row) {
      if (!Array.isArray(row)) { return; }
      const tr = document.createElement('tr');
      row.forEach(function (cell) {
        const td = document.createElement('td');
        td.textContent = String(cell);
        tr.appendChild(td);
      });
      cardRows.appendChild(tr);
    });
    cardTable.hidden = false;
  }

  function openCard(event) {
    clearCardTimers();
    cardActive = true;
    cardSpeaking = state === 'speaking';
    document.body.classList.add('has-card');
    card.hidden = false;
    streamText(event.text || '');
    renderTable(event.columns || [], event.rows || []);
    if (!cardSpeaking && !cardHovered) {
      scheduleCollapse(6000);
    }
  }

  function dismissCard() {
    if (!cardActive) { return; }
    clearCardTimers();
    cardActive = false;
    cardHovered = false;
    cardSpeaking = false;
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

  function localFeedback(heldFor) {
    if (heldFor > 200) {
      setState('thinking');
      return;
    }
    setState((window.__leko_config && window.__leko_config.quietTimer && timerText) ? 'timer' : 'idle');
  }

  let pressedAt = 0;
  function releaseHold() {
    const heldFor = performance.now() - pressedAt;
    if (state !== 'held') {
      return;
    }
    if (heldFor <= 200) {
      post('ptt_cancel');
    } else {
      post('ptt_end');
    }
    localFeedback(heldFor);
  }

  pill.addEventListener('pointerdown', function (event) {
    try {
      pill.setPointerCapture(event.pointerId);
    } catch (_) {}
    pressedAt = performance.now();
    setState('held');
    post('ptt_start');
  });

  pill.addEventListener('pointerup', releaseHold);
  pill.addEventListener('pointercancel', releaseHold);
  pill.addEventListener('contextmenu', function (event) {
    event.preventDefault();
  });

  card.addEventListener('mouseenter', function () {
    cardHovered = true;
    if (cardCollapseTimer) { clearTimeout(cardCollapseTimer); cardCollapseTimer = null; }
    post('card_hover_on');
  });
  card.addEventListener('mouseleave', function () {
    cardHovered = false;
    post('card_hover_off');
    if (cardActive && !cardSpeaking) {
      scheduleCollapse(2000);
    }
  });
  document.addEventListener('keydown', function (event) {
    if (event.key === 'Escape' && cardActive) {
      dismissCard();
    }
  });
  document.addEventListener('pointerdown', function (event) {
    if (cardActive && !card.contains(event.target) && !pill.contains(event.target)) {
      dismissCard();
    }
  });
  window.addEventListener('blur', function () {
    if (cardActive && !cardHovered) {
      dismissCard();
    }
  });

  window.__leko_on_event = function (raw) {
    let event;
    try {
      event = typeof raw === 'string' ? JSON.parse(raw) : raw;
    } catch (error) {
      return;
    }
    if (event.type === 'audio_level' && typeof event.level === 'number') {
      targetLevel = Math.max(0, Math.min(1, event.level));
      lastLevelAt = performance.now();
      return;
    }
    const rawState = event.state || event.type;
    const next = rawState === 'grace' ? 'listening' : rawState;
    if (next === 'reply_card') {
      openCard(event);
      return;
    }
    if (next === 'card_dismiss') {
      dismissCard();
      return;
    }
    if (next === 'hover_on' || next === 'hover_off') {
      hovered = next === 'hover_on';
      render();
      return;
    }
    if (next === 'blocked' || next === 'error') {
      dismissCard();
      setState(next);
      localTimer = setTimeout(function () {
        setState((window.__leko_config && window.__leko_config.quietTimer && timerText) ? 'timer' : 'idle');
      }, 2200);
      return;
    }
    if (next === 'timer_start') {
      timerText = event.timer || '';
      if (window.__leko_config && window.__leko_config.quietTimer && (state === 'idle' || state === 'timer')) {
        setState('timer');
      }
      return;
    }
    if (next === 'timer_stop') {
      timerText = '';
      setState('idle');
      return;
    }
    if (state === 'held' && next === 'idle') {
      return;
    }
    if (next === 'speaking' && cardActive) {
      cardSpeaking = true;
      if (cardCollapseTimer) { clearTimeout(cardCollapseTimer); cardCollapseTimer = null; }
    }
    if (next === 'idle' && cardActive) {
      cardSpeaking = false;
      if (!cardHovered) { scheduleCollapse(3500); }
    }
    if (next === 'idle' && window.__leko_config && window.__leko_config.quietTimer && timerText) {
      setState('timer');
      return;
    }
    if (WIDTHS[next]) {
      setState(next, event.timer);
    }
  };

  render();
})();
