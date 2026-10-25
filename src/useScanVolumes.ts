import { useEffect, useRef, useState } from 'react';
import type { SpaceJourney } from './useSpaceJourney';

/** Owns scan scope; analysis navigation never changes the selected disks. */
export function useScanVolumes(
  journey: Pick<
    SpaceJourney,
    'locations' | 'locationsLoaded' | 'locationsLoading' | 'locationError'
  >,
) {
  const [scanVolumeSelection, setScanVolumeSelection] = useState<string[] | null>([]);
  const scanVolumesInitialized = useRef(false);
  const chooseScanVolumes = (tokens: string[] | null) => {
    scanVolumesInitialized.current = true;
    setScanVolumeSelection(tokens);
  };
  useEffect(() => {
    if (scanVolumesInitialized.current || !journey.locations.length) return;
    const volumes = journey.locations.filter((location) => location.kind === 'volume');
    if (!volumes.length) return;
    scanVolumesInitialized.current = true;
    const c = volumes.find(
      (location) => /^c:\\$/i.test(location.path) && location.supported !== false,
    );
    setScanVolumeSelection(c ? [c.token] : []);
  }, [journey.locations]);
  const scanVolumeTokens = scanVolumeSelection?.filter((token) =>
    journey.locations.some(
      (location) =>
        location.token === token && location.kind === 'volume' && location.supported !== false,
    ),
  );
  const canScanVolumes =
    journey.locationsLoaded &&
    !journey.locationsLoading &&
    !journey.locationError &&
    journey.locations.some((l) => l.kind === 'volume' && l.supported !== false) &&
    (scanVolumeSelection === null || !!scanVolumeTokens?.length);
  return { scanVolumeSelection, chooseScanVolumes, scanVolumeTokens, canScanVolumes };
}
