const stageEl = document.getElementById('stage');
const statusEl = document.getElementById('status');
const cardEl = document.getElementById('draft-card');

const controller = new window.CircleController(stageEl, statusEl, cardEl);
const bridge = new window.LekoBridge(controller);

document.getElementById('btn-confirm').addEventListener('click', () => controller.confirmDraft());
document.getElementById('btn-cancel').addEventListener('click', () => controller.cancelDraft());
document.getElementById('btn-mute').addEventListener('click', () => bridge.toggleMute());
document.getElementById('btn-stop').addEventListener('click', () => bridge.stop());

stageEl.addEventListener('click', () => {
  if (controller.currentState === 'idle') {
    controller.setState('listening', 'Listening...');
    bridge.sendIpcAction('listen_start');
  } else {
    bridge.stop();
  }
});

window.addEventListener('keydown', (event) => {
  if (event.target && ['INPUT', 'TEXTAREA'].includes(event.target.tagName)) return;
  if (event.code === 'Space') {
    event.preventDefault();
    if (controller.currentState === 'idle') {
      controller.setState('listening', 'Listening...');
      bridge.sendIpcAction('listen_start');
    } else {
      bridge.stop();
    }
  } else if (event.code === 'Escape') {
    event.preventDefault();
    bridge.stop();
  }
});
