import type { CleanupDecision } from './cleanupDecision';

export function CleanupGuidance({ decision }: { decision: CleanupDecision }) {
  return (
    <p className="decision-guidance" role="status">
      {decision.message}
    </p>
  );
}
