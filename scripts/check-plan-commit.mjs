// A commit gets only the checks that finish in seconds. Checks that compile a
// Rust workspace or run its tests take from tens of seconds to minutes, so the
// pre-commit hook defers them: the pre-push hook plans over everything being
// pushed, runs them there, and fails closed to the full gate for a new branch.
// The cache records only the checks that actually ran, so a fast commit never
// counts as a passed push gate.
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
