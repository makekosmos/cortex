# Spec and build

## Configuration
- **Artifacts Path**: {@artifacts_path} → `.zenflow/tasks/{task_id}`

---

## Agent Instructions

Ask the user questions when anything is unclear or needs their input. This includes:
- Ambiguous or incomplete requirements
- Technical decisions that affect architecture or user experience
- Trade-offs that require business context

Do not make assumptions on important decisions — get clarification first.

---

## Workflow Steps

### [x] Step: Technical Specification
<!-- chat-id: 7f2b6e6a-254d-4de0-ae2d-567141a88cb8 -->

Assess the task's difficulty, as underestimating it leads to poor outcomes.
- easy: Straightforward implementation, trivial bug fix or feature
- medium: Moderate complexity, some edge cases or caveats to consider
- hard: Complex logic, many caveats, architectural considerations, or high-risk changes

Create a technical specification for the task that is appropriate for the complexity level:
- Review the existing codebase architecture and identify reusable components.
- Define the implementation approach based on established patterns in the project.
- Identify all source code files that will be created or modified.
- Define any necessary data model, API, or interface changes.
- Describe verification steps using the project's test and lint commands.

Save the output to `{@artifacts_path}/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach
- Source code structure changes
- Data model / API / interface changes
- Verification approach

If the task is complex enough, create a detailed implementation plan based on `{@artifacts_path}/spec.md`:
- Break down the work into concrete tasks (incrementable, testable milestones)
- Each task should reference relevant contracts and include verification steps
- Replace the Implementation step below with the planned tasks

Rule of thumb for step size: each step should represent a coherent unit of work (e.g., implement a component, add an API endpoint, write tests for a module). Avoid steps that are too granular (single function).

Save to `{@artifacts_path}/plan.md`. If the feature is trivial and doesn't warrant this breakdown, keep the Implementation step below as is.

---

### [x] Step: Sidebar stability + UX refresh
<!-- chat-id: 38d18a19-211f-49ab-9643-4ddfce56c410 -->
Update `src/components/Sidebar.tsx` (and layout styles) to lock width, reduce jitter, and polish nav styling. Verify by navigating between pages and toggling collapse.

### [x] Step: Playtime aggregation + stats API
<!-- chat-id: 1c102fce-52b0-43e2-861d-c5d32abd81e4 -->
Add `playtime_daily` storage, tracker updates, and a range-based stats command in Rust; expose it in `src/lib/api.ts`. Verify with `cargo test` or manual DB checks.

### [x] Step: Statistics charts refresh
<!-- chat-id: ce7f5b7e-f827-43f6-aab7-2dc4dec95a70 -->
Replace the existing chart with range totals + per-game breakdown (range selector) in `src/pages/Statistics.tsx`. Verify chart readability and data binding.

### [x] Step: Verification + report
Run available tests (`pnpm test`, `cargo test`) and write `{@artifacts_path}/report.md` with implementation notes.
