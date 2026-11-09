export const VOICE_BRIDGE_PRELUDE = `
from voice.ui_bridge import UIPipelineBridge

bridge = UIPipelineBridge()
emitted = []
bridge.emit = lambda ev: emitted.append(ev)

spoken_takes = []
def mock_speak(txt, is_cancelled=None):
    spoken_takes.append(txt)
`;
