import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { ManualCart } from '../ManualCart';
import { ScanVolumes } from '../ScanVolumes';
import { HomeStorage } from '../HomeStorage';
import type { AppController } from '../useAppController';
import type { AnalysisEntry, AnalysisLocation } from '../types';
vi.mock('../api', () => ({ desktop: true }));
afterEach(cleanup);
const entry = (id: number): AnalysisEntry => ({
  id: String(id),
  name: `文件${id}`,
  path: `G:\\资料\\${id}`,
  parent: 'root',
  ancestors: ['root'],
  directory: false,
  bytes: '1',
  files: 1,
  modified_unix: 0,
  shared: false,
  incomplete: false,
  issue: null,
  rule_ids: [],
  excluded: false,
  manual: { allowed: true, reason: null },
});
it('paginates selected ranges at 200, searches names and paths, and does not alter selection while viewing', async () => {
  const selectManual = vi.fn(),
    inspectManual = vi.fn(),
    clearManual = vi.fn();
  const manualChoices = new Map(
    Array.from({ length: 401 }, (_, i) => [String(i), { entry: entry(i), selected: true }]),
  );
  const model = {
    manualChoices,
    selectManual,
    inspectManual,
    clearManual,
    busy: false,
    saving: false,
  } as unknown as AppController;
  const user = userEvent.setup();
  render(<ManualCart model={model} />);
  expect(screen.getAllByRole('listitem')).toHaveLength(200);
  await user.click(screen.getByRole('button', { name: '下一页' }));
  expect(screen.getAllByRole('listitem')).toHaveLength(200);
  await user.click(screen.getByRole('button', { name: '下一页' }));
  expect(screen.getAllByRole('listitem')).toHaveLength(1);
  await user.click(screen.getByRole('button', { name: '查看所在目录' }));
  expect(inspectManual).toHaveBeenCalledWith(entry(400));
  expect(selectManual).not.toHaveBeenCalled();
  await user.type(screen.getByRole('searchbox'), '资料\\12');
  expect(screen.getAllByRole('listitem')).toHaveLength(11);
  await user.click(screen.getByRole('button', { name: '移除文件12' }));
  expect(selectManual).toHaveBeenCalledWith(entry(12), false);
  await user.click(screen.getByRole('button', { name: '取消全部自选' }));
  expect(clearManual).toHaveBeenCalledTimes(1);
});
const disks: AnalysisLocation[] = ['C', 'D', 'E', 'F'].map((token) => ({
  token,
  path: `${token}:\\`,
  title: token,
  kind: 'volume',
  supported: true,
}));
it('distinguishes disk loading, error, no supported disks, and no selection', () => {
  const props = {
    locations: [] as AnalysisLocation[],
    selection: [] as string[],
    disabled: false,
    error: '',
    onChange: vi.fn(),
    onReload: vi.fn(),
  };
  const view = render(<ScanVolumes {...props} loading loaded={false} />);
  expect(screen.getByRole('status')).toHaveTextContent('正在读取磁盘列表');
  expect(screen.queryByText(/请至少选择/)).not.toBeInTheDocument();
  expect(screen.getByRole('checkbox', { name: '全部磁盘' })).toBeDisabled();
  view.rerender(<ScanVolumes {...props} error="读取错误" />);
  expect(screen.getByRole('status')).toHaveTextContent('磁盘列表读取失败');
  view.rerender(<ScanVolumes {...props} />);
  expect(screen.getByRole('status')).toHaveTextContent('没有可用的磁盘');
  view.rerender(<ScanVolumes {...props} locations={disks} />);
  expect(screen.getByRole('status')).toHaveTextContent('请至少选择');
});
it('shows each selected volume separately, folds after three, and labels stale failures', async () => {
  const user = userEvent.setup();
  const readings = new Map(
    disks.map((d) => [
      d.token,
      {
        context: {
          location_token: d.token,
          volume: d.path,
          volume_key: d.token,
          available_bytes: '1024',
          sampled_at: 1,
          error: null,
        },
        loading: false,
        error: d.token === 'D' ? 'failed' : '',
      },
    ]),
  );
  const view = render(<HomeStorage space={{ volumes: [disks[1]], readings, refresh: vi.fn() }} />);
  expect(screen.getByLabelText('D:\\可用空间')).toHaveTextContent('上次查询可用');
  expect(screen.queryByLabelText('C:\\可用空间')).not.toBeInTheDocument();
  view.rerender(<HomeStorage space={{ volumes: disks, readings, refresh: vi.fn() }} />);
  expect(screen.getByText('已选 4 个磁盘')).toBeVisible();
  expect(screen.getByLabelText('F:\\可用空间')).not.toBeVisible();
  await user.click(screen.getByText('其余 1 个磁盘'));
  expect(screen.getByLabelText('F:\\可用空间')).toBeVisible();
  expect(within(screen.getByLabelText('C:\\可用空间')).getByText('1.0 KB')).toBeVisible();
});
