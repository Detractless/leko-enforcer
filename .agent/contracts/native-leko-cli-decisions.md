# native-leko-cli decisions

Resolved: the user approved all four recommended answers. Nothing is locked until the user types `/contract open native-leko-cli`.

1. **Command vocabulary:** Approved. The ten canonical rule commands are `create-rule`, `list-rules`, `show-rule`, `edit-rule`, `delete-rule`, `queue-edit`, `cancel-edit`, `request-pass`, `check-status` and `evaluate-rule`.

2. **Infrastructure subcommands:** Approved. Keep `settings`, `agent` and `enforce` with their launch-agent and daemon interface preserved, adapting only their internals to `leko-core`. Drop the legacy rule commands such as `add-block`, `session`, `switch-off` and `status`. AC-001 and the Decisions section of `.agent/contracts/native-leko-cli.md` now state this explicitly.

3. **`--json` shape:** Approved. One object with the ten canonical rule fields at the top level and nested `target`, `schedule`, `penalty`, `pass` and `seal` objects.

4. **Voice routing between M2 and M3:** Approved. Accept that `voice/routing/` will call old command names and break until Milestone 3, and add no legacy CLI shims in M2.

Milestone map: M2 `native-leko-cli` (this contract), M3 `native-leko-voice-routing`, M4 `native-leko-addon-runner`.
