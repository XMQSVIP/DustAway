import { useState } from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { DeferredPage } from '../DeferredPage';
import { ProductCapabilities } from '../ProductCapabilities';
import { preparedSummary } from '../SelectionSummary';
import type { Bootstrap, ScopedPlan } from '../types';
import capabilities from '../../docs/current-capabilities.json';

afterEach(cleanup);
it('deferred pages do no work before visiting and retain state after leaving', () => {
  let mounts = 0;
  function Page() {
    const [text, setText] = useState(() => {
      mounts++;
      return '';
    });
    return <input aria-label="保留输入" value={text} onChange={(e) => setText(e.target.value)} />;
  }
  const view = render(
    <DeferredPage active={false}>
      <Page />
    </DeferredPage>,
  );
  expect(mounts).toBe(0);
  view.rerender(
    <DeferredPage active>
      <Page />
    </DeferredPage>,
  );
  fireEvent.change(screen.getByLabelText('保留输入'), { target: { value: '已选文件' } });
  view.rerender(
    <DeferredPage active={false}>
      <Page />
    </DeferredPage>,
  );
  view.rerender(
    <DeferredPage active>
      <Page />
    </DeferredPage>,
  );
  expect(screen.getByLabelText('保留输入')).toHaveValue('已选文件');
  expect(mounts).toBe(1);
});
it('about follows the build capability and refuses mismatched catalogue claims', () => {
  const boot = {
    version: capabilities.version,
    capabilities: { system_cleanup: false },
  } as Bootstrap;
  const view = render(<ProductCapabilities boot={boot} />);
  expect(screen.getByText(/本构建不提供提权清理/)).toBeInTheDocument();
  expect(screen.queryByText(/QQNT 尚不适用/)).not.toBeInTheDocument();
  view.rerender(<ProductCapabilities boot={{ ...boot, capabilities: { system_cleanup: true } }} />);
  expect(screen.getByText(/本构建支持受限系统维护/)).toBeInTheDocument();
  view.rerender(<ProductCapabilities boot={{ ...boot, version: '0.0.0' }} />);
  expect(screen.getByText(/版本信息不一致/)).toBeInTheDocument();
});
it('frozen mixed plans count directories separately and preserve large byte integers', () => {
  const plans = [
    {
      scope: 'user',
      plan: {
        files: 3,
        bytes: '9007199254740993',
        manual: { files: 1, directories: 2, bytes: '10' },
      },
    },
  ] as ScopedPlan[];
  const value = preparedSummary(plans);
  expect(value).toMatchObject({
    files: 3,
    directories: 2,
    bytes: 9007199254740993n,
    ruleFiles: 2,
    ruleBytes: 9007199254740983n,
  });
  expect(preparedSummary([])).toMatchObject({ files: 0, directories: 0, bytes: 0n });
});
