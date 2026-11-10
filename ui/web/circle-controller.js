const stageEl = document.getElementById('stage');
const statusEl = document.getElementById('status');
const cardEl = document.getElementById('draft-card');

const controller = new window.CircleController(stageEl, statusEl, cardEl);
const bridge = new window.LekoBridge(controller);

document.getElementById('btn-confirm').addEventListener('click', () => controller.confirmDraft());
document.getElementById('btn-cancel').addEventListener('click', () => controller.cancelDraft());

let activePointer = null;
let spaceHeld = false;
let captureActive = false;

function startsFromAction(target) {
  return target instanceof Element
    && Boolean(target.closest('input, textarea, select, [contenteditable="true"], button:not(#stage)'));
}

function beginCapture() {
  if (captureActive) return;
  captureActive = true;
  controller.setState('held');
  bridge.sendIpcAction('ptt_start');
}

function endCapture(cancel = false) {
  if (!captureActive && !(cancel && controller.currentState === 'grace')) return;
  captureActive = false;
  if (cancel) {
    bridge.cancelUnsubmitted();
  } else {
    controller.setState('grace');
    bridge.sendIpcAction('ptt_end');
  }
}

stageEl.addEventListener('pointerdown', (event) => {
  if (event.button != null && event.button !== 0) return;
  if (activePointer !== null) return;
  event.preventDefault();
  activePointer = event.pointerId;
  if (stageEl.setPointerCapture) stageEl.setPointerCapture(event.pointerId);
  beginCapture();
});

stageEl.addEventListener('pointerup', (event) => {
  if (event.pointerId !== activePointer) return;
  activePointer = null;
  endCapture();
});

stageEl.addEventListener('pointercancel', (event) => {
  if (event.pointerId !== activePointer) return;
  activePointer = null;
  endCapture();
});

stageEl.addEventListener('lostpointercapture', () => {
  if (activePointer === null) return;
  activePointer = null;
  endCapture();
});

window.addEventListener('keydown', (event) => {
  if (event.code === 'Escape' && (captureActive || controller.currentState === 'grace')) {
    event.preventDefault();
    activePointer = null;
    spaceHeld = false;
    endCapture(true);
    return;
  }
  if (startsFromAction(event.target)) return;
  if (event.code === 'Space') {
    if (event.repeat || spaceHeld) return;
    event.preventDefault();
    spaceHeld = true;
    beginCapture();
  }
});

window.addEventListener('keyup', (event) => {
  if (event.code !== 'Space' || !spaceHeld) return;
  event.preventDefault();
  spaceHeld = false;
  endCapture();
});

function releaseOnFocusLoss() {
  activePointer = null;
  spaceHeld = false;
  endCapture();
}

window.addEventListener('blur', releaseOnFocusLoss);
document.addEventListener('visibilitychange', () => {
  if (document.hidden) releaseOnFocusLoss();
});
