import { afterEach, expect, it } from 'vitest';
import { relativeSize } from '../rankingSize';
import { readSpatialPreferences, defaultPreferences } from '../spatialPreferences';

afterEach(() => localStorage.clear());

it('uses integer arithmetic before converting visual ratios', () => {
  expect(relativeSize('90071992547409930000', 180143985094819860000n)).toBe(50);
  expect(relativeSize('0', 0n)).toBe(0);
  expect(relativeSize('unavailable', 200n)).toBe(0);
});

it('recovers invalid display preferences without retaining unrelated data', () => {
  localStorage.setItem('dustaway.ui.v2', '{broken');
  expect(readSpatialPreferences()).toEqual(defaultPreferences);
  localStorage.setItem(
    'dustaway.ui.v2',
    JSON.stringify({ view: 'unknown', spread: 125, motion: 'unknown', candidate: 'private' }),
  );
  expect(readSpatialPreferences()).toEqual({ motion: 'system' });
});

it('inherits only motion from the old preferences and leaves the old key intact', () => {
  const old = JSON.stringify({ view: 'list', spread: 80, motion: 'reduced', candidate: 'private' });
  localStorage.setItem('dustaway.ui.v2', old);
  expect(readSpatialPreferences()).toEqual({ motion: 'reduced' });
  expect(localStorage.getItem('dustaway.ui.v2')).toBe(old);
  localStorage.setItem('dustaway.ui.v3', JSON.stringify({ motion: 'system' }));
  expect(readSpatialPreferences()).toEqual({ motion: 'system' });
});
