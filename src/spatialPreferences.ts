import { useEffect, useState } from 'react';

export interface SpatialPreferences {
  motion: 'system' | 'reduced';
}
const key = 'dustaway.ui.v3';
export const defaultPreferences: SpatialPreferences = {
  motion: 'system',
};
export function readSpatialPreferences(): SpatialPreferences {
  try {
    const value = JSON.parse(
      localStorage.getItem(key) ?? localStorage.getItem('dustaway.ui.v2') ?? 'null',
    );
    return {
      motion: value?.motion === 'reduced' ? 'reduced' : 'system',
    };
  } catch {
    return { ...defaultPreferences };
  }
}
export function useSpatialPreferences() {
  const [preferences, setPreferences] = useState(readSpatialPreferences);
  useEffect(() => {
    document.documentElement.dataset.motion = preferences.motion;
    try {
      localStorage.setItem(key, JSON.stringify(preferences));
    } catch {
      /* Display remains usable without storage. */
    }
  }, [preferences]);
  function updatePreferences(patch: Partial<SpatialPreferences>) {
    setPreferences((previous) => ({ ...previous, ...patch }));
  }
  return { preferences, updatePreferences };
}
