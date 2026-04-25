# Verification Artifact Capture Issue

## Problem

The first attempt to save command output with PowerShell `*>` returned non-zero for commands that had already passed in the live verification run. PowerShell wrapped normal native stderr output from `cargo`/`bun` as `NativeCommandError`, producing noisy artifact files despite successful commands.

## Fix

No code change was needed. Re-capture raw verification output with `cmd /c "... > file 2>&1"` and rerun the verification commands from the current worktree.
