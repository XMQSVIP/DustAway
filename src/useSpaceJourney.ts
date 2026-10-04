import { useEffect, useRef, useState } from 'react';
import { api, desktop } from './api';
import { ipcError } from './coordinator';
import type { AnalysisContext, AnalysisLocation, AnalysisSnapshot } from './types';

// Lives with the application controller. No paths, samples or selections are persisted.
export function useSpaceJourney(
  enabled: boolean,
  snapshot: AnalysisSnapshot | null,
  reportKey = '',
  sampleContext = true,
) {
  const activated = useRef(false);
  if (enabled) activated.current = true;
  const live = activated.current;
  const [locationsLoading, setLocationsLoading] = useState(false);
  const [locationsLoaded, setLocationsLoaded] = useState(false);
  const [locations, setLocations] = useState<AnalysisLocation[]>([]);
  const [selected, setSelected] = useState('');
  const [locationError, setLocationError] = useState('');
  const [locationEpoch, setLocationEpoch] = useState(0);
  const [context, setContext] = useState<AnalysisContext | null>(null);
  const [baseline, setBaseline] = useState<AnalysisContext | null>(null);
  const [contextLoading, setContextLoading] = useState(false);
  const [contextError, setContextError] = useState('');
  const [epoch, setEpoch] = useState(0);
  const samples = useRef(new Map<string, AnalysisContext>());
  const baselines = useRef(new Map<string, AnalysisContext>());
  const returnFocus = useRef<HTMLElement | null>(null);
  const lastReport = useRef('');
  if (reportKey) lastReport.current = reportKey;
  const terminalReport = lastReport.current;
  const finished = snapshot && snapshot.state !== 'scanning' ? snapshot.id : '';
  useEffect(() => {
    if (!live || !desktop) return;
    let valid = true;
    setLocationError('');
    setLocationsLoading(true);
    api
      .analysisLocations()
      .then((list) => {
        if (!valid) return;
        setLocations(list);
        setSelected(
          (value) =>
            value ||
            snapshot?.location.token ||
            list.find((l) => l.is_system && l.supported !== false)?.token ||
            list.find((l) => l.title === '当前用户文件夹')?.token ||
            list.find((l) => l.supported !== false)?.token ||
            '',
        );
      })
      .catch((e) => {
        if (valid) setLocationError(`无法读取位置：${ipcError(e).message}`);
      })
      .finally(() => {
        if (valid) {
          setLocationsLoading(false);
          setLocationsLoaded(true);
        }
      });
    return () => {
      valid = false;
    };
  }, [live, locationEpoch]);
  useEffect(() => {
    if (!live || !selected || !desktop || !sampleContext) return;
    let valid = true;
    const previous = samples.current.get(selected) ?? null;
    setContext(previous);
    setBaseline(previous?.volume_key ? (baselines.current.get(previous.volume_key) ?? null) : null);
    setContextError('');
    setContextLoading(true);
    api
      .analysisContext(selected)
      .then((value) => {
        if (!valid) return;
        if (value.error || value.available_bytes === null) {
          setContextError(value.error || '未取得磁盘空间');
          if (!previous) setContext(value);
          return;
        }
        samples.current.set(selected, value);
        setContext(value);
        if (value.volume_key) {
          if (!baselines.current.has(value.volume_key))
            baselines.current.set(value.volume_key, value);
          setBaseline(baselines.current.get(value.volume_key)!);
        } else setBaseline(null);
      })
      .catch((e) => {
        if (valid) setContextError(ipcError(e).message);
      })
      .finally(() => {
        if (valid) setContextLoading(false);
      });
    return () => {
      valid = false;
    };
  }, [live, selected, epoch, finished, terminalReport, sampleContext]);
  return {
    locationsLoading,
    locationsLoaded,
    locations,
    setLocations,
    selected,
    setSelected,
    locationError,
    setLocationError,
    reloadLocations: () => setLocationEpoch((n) => n + 1),
    context: context?.location_token === selected ? context : null,
    baseline,
    contextLoading,
    contextError,
    refresh: () => setEpoch((n) => n + 1),
    returnFocus,
  };
}
export type SpaceJourney = ReturnType<typeof useSpaceJourney>;
