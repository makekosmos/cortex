$ErrorActionPreference = "Stop"

$rawInput = [Console]::In.ReadToEnd()
$payload = $rawInput | ConvertFrom-Json
$eventName = [string]$payload.hook_event_name

if ([string]::IsNullOrWhiteSpace($eventName)) {
    throw "hook_event_name is required"
}

$parentContext = @"
Kosmos model-orchestration contract:
- Classify the task as LOW, MEDIUM, or HIGH before choosing the execution path.
- For MEDIUM/HIGH tasks, keep the Sol parent as control plane. Route repository search, documentation lookup, grep/file inspection, and supporting tool calls to the explorer agent pinned to gpt-5.3-codex-spark (high). If a tool result blocks the next decision, use one bounded explorer and wait; do not fan out the blocker.
- Route routine bounded implementation to worker pinned to gpt-5.6-luna (medium).
- Escalate the same bounded slice once to terra_worker pinned to gpt-5.6-terra (high) only after an incomplete Luna result, failed focused verification, or a concrete need for deeper cross-file reasoning. Preserve the Luna result and state the escalation reason.
- The gpt-5.6-sol parent owns classification, planning, conflict resolution, evidence integration, and the final answer. It uses normal execution tools only through the explicit exception in the Kosmos orchestrator contract.
- Do not overlap write scopes. Obey AGENTS.md and the NO_LOOP/LIGHT_LOOP/FULL_LOOP proof-loop policy. Every delegated result must be integrated and verified before completion is claimed.
"@

if ($eventName -eq "SubagentStart") {
    $agentType = [string]$payload.agent_type
    $context = switch ($agentType) {
        "explorer" { "Kosmos Spark lane: perform only the assigned read/search/documentation/tool-support task, do not edit, do not spawn agents, and return distilled evidence to the Sol parent." }
        "worker" { "Kosmos Luna lane: implement only the assigned bounded slice, do not spawn agents, and report a concrete Terra escalation reason if the result is incomplete or focused verification fails." }
        "terra_worker" { "Kosmos Terra escalation lane: require the Luna result and concrete escalation reason, reconfirm the blocker, make the smallest bounded repair, and do not spawn agents." }
        "reviewer" { "Kosmos Terra review lane: remain read-only, judge current evidence independently, and return PASS, FAIL, or UNKNOWN without editing." }
        default { "Kosmos delegated lane: obey the pinned project-agent role, stay within scope, do not spawn further agents, and return concise evidence to the Sol parent." }
    }
} else {
    $context = $parentContext.Trim()
}

$result = @{
    hookSpecificOutput = @{
        hookEventName = $eventName
        additionalContext = $context
    }
}

[Console]::Out.Write(($result | ConvertTo-Json -Depth 4 -Compress))
