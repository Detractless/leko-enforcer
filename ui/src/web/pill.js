(function () {
  const pill = document.getElementById('pill');
  const bars = document.getElementById('bars');
  const label = document.getElementById('label');

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

  for (let i = 0; i < 7; i += 1) {
    const bar = document.createElement('span');
    bar.className = 'bar';
    bars.appendChild(bar);
  }
  const barEls = Array.from(bars.children);
  barEls.forEach((bar, index) => {
    bar.style.setProperty('--bar-index', index);
  });

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
  pill.addEventListener('pointerdown', function (event) {
    pill.setPointerCapture(event.pointerId);
    pressedAt = performance.now();
    setState('held');
    post('ptt_start');
  });

  pill.addEventListener('pointerup', function (event) {
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
  });

  pill.addEventListener('contextmenu', function (event) {
    event.preventDefault();
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
    if (next === 'hover_on' || next === 'hover_off') {
      hovered = next === 'hover_on';
      render();
      return;
    }
    if (next === 'blocked' || next === 'error') {
      setState(next);
      localTimer = setTimeout(function () {
        setState((window.__leko_config && window.__leko_config.quietTimer && timerText) ? 'timer' : 'idle');
      }, 2200);
      return;
    }
    if (next === 'timer_start') {
      timerText = event.timer || '';
      setState('timer');
      return;
    }
    if (next === 'timer_stop') {
      timerText = '';
      setState('idle');
      return;
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
