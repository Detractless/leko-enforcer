# Voice window and session logs decisions

Resolved: the user opened the contract with the recommended defaults below.

1. **Window size:** Start at 420 by 440 logical pixels: keep the current 420-pixel width and shorten the 600-pixel height. **Recommended:** accept 440 pixels so the window is near-square and still has room for the circle and confirmation controls.
2. **Audio capture detail:** Save final STT text, exact text submitted to the model, turn outcomes, and existing audio measurements. **Recommended:** do not save raw microphone recordings or every partial chunk transcript; these contain more private data and use more storage than the final recognition trace.
3. **Model trace:** Save the complete local request context and settings, exact reasoning_content when returned, final reply, tool draft, tool result, errors, timing, and playback outcome. **Recommended:** yes; the context can include prior turns and saved facts, so keep it local with owner-only file permissions and never upload it.
4. **File layout and retention:** Save one JSONL file per session under `~/Library/Application Support/Leko/session-logs/YYYY/MM/DD/`, readable in a text editor. **Recommended:** keep files until manually deleted, with no automatic purge; this preserves history across restarts but uses disk space over time.

Example layout:

```text
~/Library/Application Support/Leko/session-logs/
  2026/
    11/
      09/
        session-8f49c7.jsonl
```
