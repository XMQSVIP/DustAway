import type { DeskStage } from './cleanupPresentation';
import sculpture from './assets/space-sculpture.png';

/** Decorative layers share one offline asset; only the three detached pieces move. */
export function LayerScene({
  stage = 'idle',
  compact = false,
}: {
  stage?: DeskStage;
  compact?: boolean;
}) {
  return (
    <div
      className={`layer-scene${compact ? ' miniature' : ''}`}
      data-stage={stage}
      aria-hidden="true"
    >
      {['sculpture-body', 'piece-blue', 'piece-sphere', 'piece-apricot'].map((part) => (
        <img
          key={part}
          className={`sculpture-layer ${part}`}
          src={sculpture}
          alt=""
          draggable={false}
        />
      ))}
    </div>
  );
}
