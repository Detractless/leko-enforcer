/**
 * Leko Minimalist Circle Interface - Logic & Bridge
 * Provides the state controller, draft card rendering, and IPC/API bridge.
 */

function parseIpcLine(line) {
  if (typeof line !== 'string') return null;
  const trimmed = line.trim();
  if (!trimmed) return null;
  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

function formatIpcMessage(msg) {
  return JSON.stringify(msg) + '\n';
}

class CircleController {
  constructor(stageEl, statusEl, cardEl) {
    this.stageEl = stageEl;
    this.statusEl = statusEl;
    this.cardEl = cardEl;
    this.currentState = 'idle';
    this.currentDraft = null;
    this.onConfirmCallback = null;
    this.onCancelCallback = null;
  }

  setState(state, customLabel = null) {
    const validStates = ['idle', 'listening', 'thinking', 'speaking', 'confirmed'];
    if (!validStates.includes(state)) {
      throw new Error(`Invalid circle state: ${state}`);
    }

    this.currentState = state;
    if (this.stageEl) {
      const isMuted = this.stageEl.classList && this.stageEl.classList.contains('is-muted');
      this.stageEl.className = `circle-stage state-${state}${isMuted ? ' is-muted' : ''}`;
    }

    const defaultLabels = {
      idle: 'Ready',
      listening: 'Listening...',
      thinking: 'Thinking...',
      speaking: 'Maeve Speaking...',
      confirmed: 'Locked & Saved'
    };

    if (this.statusEl) {
      this.statusEl.textContent = customLabel || defaultLabels[state];
    }

    if (typeof document !== 'undefined') {
      const btnStop = document.getElementById('btn-stop');
      if (btnStop) {
        if (state === 'speaking') {
          btnStop.textContent = 'Interrupt';
          btnStop.classList.add('btn-interrupt');
          btnStop.title = 'Interrupt speech (Space)';
        } else {
          btnStop.textContent = 'Stop';
          btnStop.classList.remove('btn-interrupt');
          btnStop.title = 'Interrupt speech or reset draft (Space)';
        }
      }
    }
  }

  showDraft(draft, onConfirm, onCancel) {
    this.currentDraft = draft;
    this.onConfirmCallback = onConfirm;
    this.onCancelCallback = onCancel;

    if (!this.cardEl) return;

    const titleEl = this.cardEl.querySelector('.card-title');
    const badgeEl = this.cardEl.querySelector('.card-badge');
    const detailsEl = this.cardEl.querySelector('.card-details');

    if (titleEl) titleEl.textContent = draft.name || 'Block Action';
    if (badgeEl) badgeEl.textContent = draft.kind || 'Rule';
    if (detailsEl) detailsEl.textContent = draft.description || '';

    this.cardEl.classList.add('visible');
  }

  hideDraft() {
    this.currentDraft = null;
    if (this.cardEl && this.cardEl.classList) {
      this.cardEl.classList.remove('visible');
    }
  }

  confirmDraft() {
    const draft = this.currentDraft;
    if (this.onConfirmCallback && draft) {
      this.onConfirmCallback(draft);
    }
    this.hideDraft();
    if (draft) this.setState('thinking', 'Saving...');
  }

  cancelDraft() {
    const draft = this.currentDraft;
    if (this.onCancelCallback && draft) {
      this.onCancelCallback(draft);
    }
    this.hideDraft();
    this.setState('idle');
  }
}

// In-process / Local WebSocket or PostMessage Bridge
class LekoBridge {
  constructor(controller, backendUrl = 'http://127.0.0.1:18080') {
    this.controller = controller;
    this.backendUrl = backendUrl;
    this.isMuted = false;

    // Attach global listener for native WebView IPC script evaluation
    if (typeof window !== 'undefined') {
      window.__leko_on_event = (event) => this.handleIncomingEvent(event);
    }
  }

  sendIpcAction(action, draft = null, extra = {}) {
    const payload = { action, draft, ...extra };
    if (typeof window !== 'undefined' && window.ipc && window.ipc.postMessage) {
      window.ipc.postMessage(JSON.stringify(payload));
    }
    return payload;
  }

  toggleMute() {
    this.isMuted = !this.isMuted;
    if (this.controller.stageEl && this.controller.stageEl.classList) {
      if (this.isMuted) {
        this.controller.stageEl.classList.add('is-muted');
      } else {
        this.controller.stageEl.classList.remove('is-muted');
      }
    }
    const btnMute = typeof document !== 'undefined' ? document.getElementById('btn-mute') : null;
    if (btnMute) {
      btnMute.classList.toggle('active', this.isMuted);
      btnMute.textContent = this.isMuted ? 'Unmute' : 'Mute';
    }
    return this.sendIpcAction('mute', null, { muted: this.isMuted });
  }

  stop() {
    this.controller.hideDraft();
    this.controller.setState('idle');
    return this.sendIpcAction('stop');
  }

  handleIncomingEvent(event) {
    if (!event || typeof event !== 'object') return;

    if (event.type === 'state') {
      this.controller.setState(event.state, event.label);
    } else if (event.type === 'draft') {
      this.controller.showDraft(
        event.draft,
        (draft) => this.sendIpcAction('confirm', draft),
        (draft) => this.sendIpcAction('cancel', draft)
      );
    } else if (event.type === 'reset') {
      this.controller.hideDraft();
      this.controller.setState('idle');
    }
  }

  handleResponse(response) {
    if (response.draft) {
      this.controller.showDraft(
        response.draft,
        (draft) => this.sendIpcAction('confirm', draft),
        (draft) => this.sendIpcAction('cancel', draft)
      );
    }
    if (response.spokenReply) {
      this.controller.setState('speaking');
    } else {
      this.controller.setState('idle');
    }
  }

  async sendVoiceTurn(text) {
    this.controller.setState('thinking');
    this.sendIpcAction('query', null, { text });

    if (typeof window === 'undefined' || !window.ipc) {
      const response = await this.mockProcess(text);
      this.handleResponse(response);
    }
  }

  async mockProcess(text) {
    await new Promise((res) => setTimeout(res, 20));
    const lower = text.toLowerCase();
    if (lower.includes('session')) {
      return {
        draft: {
          name: 'Focus Session',
          kind: 'Session',
          description: 'Immediate unskippable focus session for 30 minutes.'
        },
        spokenReply: 'Starting a focus session of 30m.'
      };
    }
    return {
      draft: {
        name: 'Social Media Block',
        kind: 'Block',
        description: 'Closes reddit.com and twitter.com for 2 hours.'
      },
      spokenReply: 'Drafted a block for reddit.com, twitter.com.'
    };
  }
}

if (typeof window !== 'undefined') {
  window.CircleController = CircleController;
  window.LekoBridge = LekoBridge;
}

if (typeof module !== 'undefined' && module.exports) {
  module.exports = { CircleController, LekoBridge, parseIpcLine, formatIpcMessage };
}
