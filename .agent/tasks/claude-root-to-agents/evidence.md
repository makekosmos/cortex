# Evidence: replace .claude root with symlink to .agents

## Summary
Replaced the entire `/workspace/.claude` directory with a symlink to `/workspace/.agents` so Claude-facing paths now resolve directly into `.agents`.

## Acceptance Criteria Results

### AC1
`/workspace/.claude` is a symlink after implementation.

Result: PASS

Evidence:
- `ls -ld .claude` => `lrwxrwxrwx 1 node node 7 ... .claude -> .agents`
- Raw artifact: `.agent/tasks/claude-root-to-agents/raw/post-claude-ls.txt`

### AC2
`/workspace/.claude` resolves to `/workspace/.agents`.

Result: PASS

Evidence:
- `readlink .claude` => `.agents`
- Verified resolved path equals `/workspace/.agents`
- Raw artifacts:
  - `.agent/tasks/claude-root-to-agents/raw/readlink-claude.txt`
  - `.agent/tasks/claude-root-to-agents/raw/verification.txt`

### AC3
Paths under `.claude` for shared assets continue to work via the symlink, specifically `.claude/skills` and `.claude/agents` exist and resolve through `.agents`.

Result: PASS

Evidence:
- Verification confirms `.claude/skills` exists
- Verification confirms `.claude/agents` exists
- Raw artifact: `.agent/tasks/claude-root-to-agents/raw/verification.txt`

### AC4
`.agents/skills` and `.agents/agents` remain present after the change.

Result: PASS

Evidence:
- Verification confirms `.agents/skills` exists
- Verification confirms `.agents/agents` exists
- Raw artifact: `.agent/tasks/claude-root-to-agents/raw/verification.txt`
