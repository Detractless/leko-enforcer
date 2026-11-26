(function () {
  const pill = document.getElementById('pill');
  const bars = document.getElementById('bars');
  const label = document.getElementById('label');

  const WIDTHS = {
    idle: 54, timer: 68, blocked: 92, listening: 112, held: 112,
    thinking: 112, speaking: 112, error: 92
  };
  const BAR_COUNTS = { listening: 5, held: 5, thinking: 3, speaking: 3 };

  let state = 'idle';
  let timerText = '';
  let localTimer = null;
  let hovered = false;

  for (let i = 0; i < 5; i += 1) {
    const bar = document.createElement('span');
    bar.className = 'bar';
    bar.style.animationDelay = (i * -0.12) + 's';
    bars.appendChild(bar);
  }
  Array.from(bars.children).forEach((bar, index) => {
    bar.style.setProperty('--bar-index', index);
  });

  function render() {
    pill.className = 'leko-pill state-' + state + (hovered ? ' is-hovered' : '');
    const count = BAR_COUNTS[state] || 0;
    Array.from(bars.children).forEach((bar, index) => {
      bar.style.display = index < count ? 'block' : 'none';
    });
    if (state === 'timer') {
      label.textContent = timerText;
    } else if (state === 'blocked' || state === 'error') {
      label.textContent = state === 'blocked' ? 'Blocked' : 'Error';
    } else if (state === 'speaking' || state === 'thinking') {
      label.textContent = 'MAEVE';
    } else {
      label.textContent = '';
    }
  }

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

  function localFeedback() {
    setState('speaking');
    localTimer = setTimeout(function () {
      setState('idle');
    }, 350);
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
    post('ptt_end');
    if (heldFor <= 200) {
      post('ptt_start');
      post('ptt_end');
    }
    localFeedback();
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
    const next = event.state || event.type;
    if (next === 'hover_on' || next === 'hover_off') {
      hovered = next === 'hover_on';
      render();
      return;
    }
    if (next === 'blocked') {
      setState('blocked');
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
