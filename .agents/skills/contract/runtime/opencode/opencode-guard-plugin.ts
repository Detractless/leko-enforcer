import type { Plugin } from "@opencode-ai/plugin"
import { spawnSync } from "node:child_process"
import { fileURLToPath } from "node:url"

// Code layout and locks for OpenCode. Before every tool call this asks opencode-guard.mjs (same checks as
// Claude Code's policy-guard and the Codex guard). A refusal is thrown, which stops the tool and tells the model why.
// If the guard cannot run, the tool call goes ahead. Rollback: delete the loader file that points here,
// or create ~/.claude/state/code-layout-off (and locks-off).

const NODE = process.execPath
const GUARD = fileURLToPath(new URL("./opencode-guard.mjs", import.meta.url))

export const LayoutGuardPlugin: Plugin = async ({ directory }) => ({
  "tool.execute.before": async (input, output) => {
    const tool = String(input?.tool ?? "").toLowerCase()
    if (!["write", "edit", "apply_patch", "bash"].includes(tool)) return
    const run = spawnSync(NODE, [GUARD], {
      input: JSON.stringify({ tool, args: output?.args, directory }),
      encoding: "utf8",
      timeout: 10000,
    })
    if (run.status !== 0 || !run.stdout) return
    let verdict: { deny?: string | null } = {}
    try {
      verdict = JSON.parse(run.stdout)
    } catch {
      return
    }
    if (verdict.deny) throw new Error(verdict.deny)
  },
})
