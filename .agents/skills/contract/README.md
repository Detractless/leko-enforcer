# Contract runtime

## Requirements

Use Node.js 24.21.0, the version used for this bundle. The runtime scripts use Node's built-in modules; no npm install is required.

## Install

1. Keep .agents/skills/contract/runtime/ in the repository.
2. From the repository root, run the skill command with the bundled runner, for example:
   node .agents/skills/contract/runtime/contractctl.mjs status --root .
3. For Claude Code, merge the hooks object in hooks.example.json into the user's Claude Code settings. The sample contains only the plan gate, shell and plan checks, plan registry, and stop checks. Hook commands use repository-relative paths and must run with the repository root as the working directory.
4. For Codex, register runtime/codex/codex-guard.mjs as a PreToolUse hook for apply_patch and Bash.
5. For OpenCode, load runtime/opencode/opencode-guard-plugin.ts as a plugin.
6. For Antigravity, register runtime/antigravity/antigravity-guard.mjs for PreToolUse calls.

All adapters resolve their shared checks from this runtime directory. Keep the nested folders intact when copying or moving it.
