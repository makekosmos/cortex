# Remove Horologion

## Context

Horologion is no longer an active Kosmos extension. The user requested moving its code into an archived GitHub repository under `ksanrse` and then fully removing Horologion files and references from the Kosmos monorepo.

## Scope

- Create or update `ksanrse/horologion` with the archived Horologion source.
- Mark the GitHub repository as archived.
- Remove Horologion source files, ignored build leftovers, tests, command references, docs references, and comments from Kosmos where they are no longer valid.
- Regenerate docs artifacts if `docs-site/` changes require it.

## Out of Scope

- Replacing the broader Pomodoro/focus backend model.
- Bumping or releasing any Kosmos package.
- Rewriting unrelated historical changelog entries unless needed to remove current Horologion references.

## Acceptance Criteria

**AC1.** `ksanrse/horologion` exists on GitHub, contains the archived Horologion source, and is marked archived.

**AC2.** The Kosmos repository no longer contains Horologion source directories or ignored leftover directories under `incubator/horologion` or `extensions/horologion`.

**AC3.** Current Kosmos code, tests, active docs, and generated docs contain no `horologion` or `Horologion` references, except where unavoidable in Git history outside the working tree.

**AC4.** Relevant verification commands pass, including repository search for Horologion references and docs checks after documentation edits.
