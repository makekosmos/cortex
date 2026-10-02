// A commit gets only the checks that finish in seconds. Checks that compile a
// Rust workspace or run its tests take from tens of seconds to minutes, so the
// pre-commit hook defers them: the pre-push hook plans the same branch diff
// (merge-base with origin/main) over the pushed tree and runs them there. Both
// hooks share the disk-tree cache, so a commit right after check:affected is a
// cache hit and never counts deferred checks as passed.
export const PUSH_ONLY_CHECKS = ["clippy", "test:rust", "runtime-staging", "manager-gpui"];

/** The part of a pre-commit plan that runs at commit time. */
export function commitPlan(plan) {
  if (plan.mode !== "pre-commit") return plan;
  if (plan.full)
    return {
      ...plan,
      full: false,
      checks: ["fast"],
      reasons: [...plan.reasons, "the commit runs the fast gate; pre-push runs the full check"],
    };
  const deferred = plan.checks.filter((check) => PUSH_ONLY_CHECKS.includes(check));
  if (deferred.length === 0) return plan;
  return {
    ...plan,
    checks: plan.checks.filter((check) => !PUSH_ONLY_CHECKS.includes(check)),
    reasons: [...plan.reasons, `deferred to pre-push: ${deferred.join(", ")}`],
  };
}
