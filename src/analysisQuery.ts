import type { AnalysisDiscoveryQuery } from './types';

/** Shared query defaults belong to the query model, independent of filter controls. */
export const defaultDiscovery: AnalysisDiscoveryQuery = {
  kind: 'file',
  category: 'all',
  min_bytes: '0',
  search: '',
  include_nested: false,
  incomplete_only: false,
  offset: 0,
};
