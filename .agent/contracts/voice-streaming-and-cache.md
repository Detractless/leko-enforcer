# Contract: voice-streaming-and-cache

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Port two performance patterns from the DWB Dictation Tauri app into the leko-enforcer Python voice stack: (1) lazy model loading with page-cache warming and idle unloading to cut cold-start latency, and (2) pipelined chunking so earlier audio windows are transcribed while recording continues, reducing wait time for long dictations.

## Domains
Domains: none

## Acceptance criteria
- AC-001: Model loads lazily on first transcription request, not at startup; startup completes without loading the STT model.
  - evidence: tests/test_lazy_load.py "model not loaded at import"
  - breaker: importing the module or starting the assistant triggers a model load before any transcription call.
- AC-002: After idle timeout (configurable, default 5 min), the model unloads and memory returns to baseline within 10% of pre-load RSS.
  - evidence: tests/test_idle_unload.py "unloads after timeout"
  - breaker: model stays resident indefinitely or RSS does not drop after unload.
- AC-003: Page-cache warming reads the model file into OS cache at startup without holding it in process memory; cold-start transcription drops below 2s for a 5s clip.
  - evidence: tests/test_warm_cache.py "warm cache reduces cold start"
  - breaker: warming holds the file in process memory or cold start remains above 2s.
- AC-004: A recording longer than the chunk window (default 30s) produces intermediate transcription results for completed windows while recording is still active.
  - evidence: tests/test_pipelined_chunking.py "intermediate results during recording"
  - breaker: no transcription output until recording stops.
- AC-005: Pipelined chunking finalizes all windows and produces a single coherent transcript with spaces between chunks, matching batch output for the same input.
  - evidence: tests/test_pipelined_chunking.py "final transcript matches batch"
  - breaker: missing windows, duplicate text, or mismatched final transcript.
- AC-006: Cancelling a recording mid-stream resets the pipeline and discards partial results without crashing.
  - evidence: tests/test_pipelined_chunking.py "cancel resets pipeline"
  - breaker: cancel leaves stale partial results or raises an exception.

## Derived checklist
- D-1: Lazy load verified by importing module and checking model handle is None -> criterion:AC-001
- D-2: Idle unload verified by mocking clock advance past timeout -> criterion:AC-002
- D-3: RSS measured before and after unload, within 10% tolerance -> criterion:AC-002
- D-4: Warm cache reads file via buffered I/O, process RSS does not grow by model size -> criterion:AC-003
- D-5: Cold-start timing measured with and without warm cache -> criterion:AC-003
- D-6: Intermediate results emitted via callback during simulated long recording -> criterion:AC-004
- D-7: Final transcript compared character-for-character with batch path -> criterion:AC-005
- D-8: Cancel mid-stream verified no crash and clean state -> criterion:AC-006
- D-9: Configurable chunk window and idle timeout exposed in settings -> criterion:AC-004

## Edge cases
- Empty audio input (zero bytes) -> out-of-scope: handled by existing VAD gate, not this feature.
- Model file missing or corrupt at warm-cache time -> criterion:AC-003
- Recording shorter than chunk window -> criterion:AC-005
- Cancel during final window transcription -> criterion:AC-006
- Idle timeout fires while a transcription is in progress -> out-of-scope: unload deferred until transcription completes.
- Concurrent transcription requests during unload -> out-of-scope: single-threaded turn execution already serializes turns.

## Out of scope
- Cleanup LLM (flowscribe GGUF + llama-server) port — separate feature.
- VAD hangover tuning changes.
- UI changes for streaming partial text display.
- Apple Silicon Neural Engine acceleration.

## Decisions
- Port lazy-load + warm-cache + idle-unload as Phase 1 before pipelined chunking | self-contained, measured gains, lowest risk | REVERSIBLE
- Use 30s default chunk window matching DWB Dictation default | proven value, configurable | REVERSIBLE
- Join chunks with single space, no overlap context | matches DWB approach, simpler; coherence risk accepted | REVERSIBLE
- Defer unload if transcription in progress | prevents corrupting active transcription | REVERSIBLE