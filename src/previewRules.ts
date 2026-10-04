import raw from './preview-rules.json';
import type { Rule } from './types';

const rules: Rule[] = raw.map((rule) => ({
  ...rule,
  cleanup_policy: rule.cleanup_policy === 'recommended' ? 'recommended' : 'review',
  data_kind: rule.data_kind === 'diagnostics' ? 'diagnostics' : 'cache',
}));
export default rules;
