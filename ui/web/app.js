/**
 * Leko Minimalist Circle Interface - Logic & Bridge
 * Provides the state controller, draft card rendering, and IPC/API bridge.
 */

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
      this.stageEl.className = `circle-stage state-${state}`;
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
    if (this.cardEl) {
      this.cardEl.classList.remove('visible');
    }
  }

  confirmDraft() {
    if (this.onConfirmCallback && this.currentDraft) {
      this.onConfirmCallback(this.currentDraft);
    }
    this.hideDraft();
    this.setState('confirmed');
    setTimeout(() => {
      this.setState('idle');
    }, 1800);
  }

  cancelDraft() {
    if (this.onCancelCallback && this.currentDraft) {
      this.onCancelCallback(this.currentDraft);
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
  }

  async sendVoiceTurn(text) {
    this.controller.setState('thinking');
    try {
      if (typeof window !== 'undefined' && window.__TAURI__ && window.__TAURI__.invoke) {
        // Tauri Rust Command
        const response = await window.__TAURI__.invoke('process_query', { query: text });
        this.handleResponse(response);
      } else {
        // Simulated or HTTP fallback
        const response = await this.mockProcess(text);
        this.handleResponse(response);
      }
    } catch (err) {
      this.controller.setState('idle', `Error: ${err.message || 'Failed'}`);
    }
  }

  handleResponse(response) {
    if (response.draft) {
      this.controller.showDraft(
        response.draft,
        (draft) => this.sendConfirmation(draft, true),
        (draft) => this.sendConfirmation(draft, false)
      );
    }
    if (response.spokenReply) {
      this.controller.setState('speaking');
    } else {
      this.controller.setState('idle');
    }
  }

  async sendConfirmation(draft, isConfirmed) {
    if (isConfirmed) {
      this.controller.setState('confirmed');
    } else {
      this.controller.setState('idle');
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

if (typeof module !== 'undefined' && module.exports) {
  module.exports = { CircleController, LekoBridge };
}
