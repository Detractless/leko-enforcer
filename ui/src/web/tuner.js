(function () {
  const $ = (id) => document.getElementById(id);
  const phraseEl = $('phrase');
  const presetsEl = $('presets');
  const waveEl = $('wave');
  const metaEl = $('timing-meta');
  const statusEl = $('status-line');
  const speedEl = $('speed');
  const keepOpenEl = $('keep-open');
  const cardEl = $('card');

  const sliders = {
    lead_ms: { el: $('lead_ms'), val: $('lead-val'), fmt: (v) => `${v} ms` },
    end_pad_ms: { el: $('end_pad_ms'), val: $('end-pad-val'), fmt: (v) => `${v} ms` },
    energy_threshold: { el: $('energy_threshold'), val: $('thresh-val'), fmt: (v) => Number(v).toFixed(3) },
    gap_bridge_ms: { el: $('gap_bridge_ms'), val: $('gap-val'), fmt: (v) => `${v} ms` },
  };

  let revealMode = 'word';
  let defaults = { reveal_mode: 'word', lead_ms: -30, energy_threshold: 0.04, gap_bridge_ms: 120, end_pad_ms: 0 };
  let audioCtx = null;
  let activeSrc = null;
  let meterTimer = null;
  let rafId = 0;
  let lastAnalysis = null;
  let lastAudioKey = '';
  let lastBuffer = null;

  window.ipc = { postMessage: function () {} };

  function currentParams() {
    return {
      text: phraseEl.value.trim() || 'No active countdowns.',
      reveal_mode: revealMode,
      lead_ms: Number(sliders.lead_ms.el.value),
      end_pad_ms: Number(sliders.end_pad_ms.el.value),
      energy_threshold: Number(sliders.energy_threshold.el.value),
      gap_bridge_ms: Number(sliders.gap_bridge_ms.el.value),
    };
  }

  function syncLabels() {
    for (const item of Object.values(sliders)) {
      item.val.textContent = item.fmt(item.el.value);
    }
    $('mode-word').classList.toggle('active', revealMode === 'word');
    $('mode-letter').classList.toggle('active', revealMode === 'letter');
  }

  function applyConfig(cfg) {
    revealMode = cfg.reveal_mode === 'letter' ? 'letter' : 'word';
    for (const [key, item] of Object.entries(sliders)) {
      if (cfg[key] !== undefined) { item.el.value = cfg[key]; }
    }
    syncLabels();
  }

  function stopPlayback(sendIdle) {
    if (activeSrc) {
      try { activeSrc.onended = null; activeSrc.stop(); } catch (_) {}
      activeSrc = null;
    }
    clearInterval(meterTimer); meterTimer = null;
    cancelAnimationFrame(rafId); rafId = 0;
    window.__leko_on_event({ type: 'audio_level', level: 0 });
    if (sendIdle) { window.__leko_on_event({ type: 'state', state: 'idle' }); }
  }

  function drawWave(data, playheadMs) {
    const ctx = waveEl.getContext('2d');
    const w = waveEl.width, h = waveEl.height;
    ctx.clearRect(0, 0, w, h);
    if (!data || !data.rms_20ms || !data.rms_20ms.length) { return; }
    const rms = data.rms_20ms;
    const totalMs = Math.max(data.duration_ms || rms.length * 20, 1);
    const maxRms = Math.max(...rms, 0.05);
    const padX = 12, plotW = w - padX * 2, baseY = h - 28, plotH = h - 54;
    const toX = (ms) => padX + Math.min(1, Math.max(0, ms / totalMs)) * plotW;

    ctx.fillStyle = 'rgba(59, 130, 246, 0.14)';
    for (const [a, b] of (data.segments_ms || [])) {
      const x1 = toX(a), x2 = toX(b);
      ctx.fillRect(x1, 18, Math.max(2, x2 - x1), baseY - 18);
    }

    const barW = Math.max(2, plotW / rms.length - 1);
    ctx.fillStyle = '#60a5fa';
    for (let i = 0; i < rms.length; i += 1) {
      const bh = (rms[i] / maxRms) * plotH;
      ctx.fillRect(toX(i * 20), baseY - bh, barW, Math.max(2, bh));
    }

    const threshY = baseY - Math.min(1, (data.threshold || 0) / maxRms) * plotH;
    ctx.strokeStyle = 'rgba(245, 183, 71, 0.8)';
    ctx.setLineDash([5, 4]);
    ctx.beginPath(); ctx.moveTo(padX, threshY); ctx.lineTo(w - padX, threshY); ctx.stroke();
    ctx.setLineDash([]);

    ctx.font = '18px ui-monospace, Menlo, monospace';
    (data.timings_ms || []).forEach((ms, idx) => {
      const x = toX(ms);
      ctx.strokeStyle = '#34d399';
      ctx.beginPath(); ctx.moveTo(x, 16); ctx.lineTo(x, baseY); ctx.stroke();
      ctx.fillStyle = '#a7f3d0';
      const label = (data.words && data.words[idx]) ? data.words[idx].slice(0, 8) : String(idx + 1);
      ctx.fillText(label, Math.min(w - 58, x + 3), h - 8);
    });

    if (typeof playheadMs === 'number' && playheadMs >= 0) {
      const px = toX(playheadMs);
      ctx.strokeStyle = '#f43f5e';
      ctx.lineWidth = 2.5;
      ctx.beginPath(); ctx.moveTo(px, 8); ctx.lineTo(px, baseY); ctx.stroke();
      ctx.lineWidth = 1;
    }
  }

  async function decodeBase64Wav(b64) {
    if (!audioCtx) { audioCtx = new (window.AudioContext || window.webkitAudioContext)(); }
    if (audioCtx.state === 'suspended') { await audioCtx.resume(); }
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i += 1) { bytes[i] = bin.charCodeAt(i); }
    return audioCtx.decodeAudioData(bytes.buffer.slice(0));
  }

  async function fetchAnalysis(includeAudio) {
    const params = currentParams();
    const res = await fetch('/api/synth', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ...params, include_audio: includeAudio }),
    });
    const data = await res.json();
    lastAnalysis = data;
    if (data.audio_b64) {
      lastBuffer = await decodeBase64Wav(data.audio_b64);
      lastAudioKey = params.text;
    }
    const segText = (data.segments_ms || []).map(([a, b]) => `${a}-${b}ms`).join(', ') || 'none';
    metaEl.textContent = `engine=${data.engine}  dur=${data.duration_ms}ms  segments=[${segText}]\nword_timings_ms=${JSON.stringify(data.timings_ms)}`;
    drawWave(data, -1);
    return data;
  }

  async function playAndSync() {
    stopPlayback(false);
    statusEl.textContent = 'Synthesizing...';
    const params = currentParams();
    const needAudio = lastAudioKey !== params.text || !lastBuffer;
    const data = await fetchAnalysis(needAudio);
    statusEl.textContent = data.engine === 'fallback' ? 'Using synthetic tone fallback (ptts unavailable).' : 'Playing TTS audio...';
    if (!lastBuffer) { return; }

    const rate = Number(speedEl.value) || 1;
    const scaledTimings = (data.timings_ms || []).map((ms) => Math.round(ms / rate));
    const scaledDur = Math.round((data.duration_ms || 0) / rate);

    const src = audioCtx.createBufferSource();
    src.buffer = lastBuffer;
    src.playbackRate.value = rate;
    src.connect(audioCtx.destination);
    activeSrc = src;

    const startedAt = audioCtx.currentTime;
    src.start(startedAt);
    window.__leko_on_event({ type: 'reply_card', text: data.text, duration_ms: scaledDur, word_timings_ms: scaledTimings, reveal_mode: revealMode });
    window.__leko_on_event({ type: 'state', state: 'speaking', text: data.text, duration_ms: scaledDur, word_timings_ms: scaledTimings, reveal_mode: revealMode });
    if (keepOpenEl.checked) { cardEl.dispatchEvent(new Event('mouseenter')); }

    meterTimer = setInterval(() => {
      const ms = (audioCtx.currentTime - startedAt) * 1000 * rate;
      const idx = Math.floor(ms / 20);
      const rms = (data.rms_20ms && data.rms_20ms[idx]) || 0;
      const level = rms <= 0.003 ? 0 : Math.min(1, Math.pow((rms - 0.003) / 0.06, 0.48));
      window.__leko_on_event({ type: 'audio_level', level });
    }, 35);

    function frame() {
      if (!activeSrc) { return; }
      const ms = (audioCtx.currentTime - startedAt) * 1000 * rate;
      drawWave(data, ms);
      rafId = requestAnimationFrame(frame);
    }
    rafId = requestAnimationFrame(frame);

    src.onended = () => {
      if (activeSrc !== src) { return; }
      stopPlayback(!keepOpenEl.checked);
      drawWave(data, -1);
      statusEl.textContent = 'Finished.';
    };
  }

  let debounce = null;
  function onTuningChange() {
    syncLabels();
    clearTimeout(debounce);
    debounce = setTimeout(() => {
      if (lastAudioKey === currentParams().text) { fetchAnalysis(false); }
    }, 60);
  }

  for (const item of Object.values(sliders)) { item.el.addEventListener('input', onTuningChange); }
  $('mode-word').addEventListener('click', () => { revealMode = 'word'; syncLabels(); });
  $('mode-letter').addEventListener('click', () => { revealMode = 'letter'; syncLabels(); });
  $('play-btn').addEventListener('click', playAndSync);
  $('stop-btn').addEventListener('click', () => { stopPlayback(true); statusEl.textContent = 'Stopped.'; });
  $('theme-btn').addEventListener('click', () => {
    const next = document.body.dataset.theme === 'light' ? 'dark' : 'light';
    document.body.dataset.theme = next;
    $('theme-btn').textContent = `Theme: ${next === 'light' ? 'Light' : 'Dark'}`;
  });
  keepOpenEl.addEventListener('change', () => {
    cardEl.dispatchEvent(new Event(keepOpenEl.checked ? 'mouseenter' : 'mouseleave'));
  });
  $('save-btn').addEventListener('click', async () => {
    const res = await fetch('/api/config', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(currentParams()),
    });
    const out = await res.json();
    statusEl.textContent = out.ok ? `Saved to Leko (${out.config.reveal_mode}, lead=${out.config.lead_ms}ms).` : 'Save failed.';
  });
  $('reset-btn').addEventListener('click', () => {
    applyConfig(defaults);
    onTuningChange();
    statusEl.textContent = 'Reset to defaults (click Save to apply to Leko).';
  });

  fetch('/api/config').then((r) => r.json()).then((data) => {
    defaults = data.defaults || defaults;
    applyConfig(data.config || defaults);
    presetsEl.innerHTML = '';
    (data.presets || []).forEach((txt, idx) => {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'btn';
      btn.textContent = idx === 0 ? 'Short ("No active countdowns.")' : (txt.length > 34 ? txt.slice(0, 32) + '...' : txt);
      btn.title = txt;
      btn.addEventListener('click', () => { phraseEl.value = txt; playAndSync(); });
      presetsEl.appendChild(btn);
    });
  });
})();
