import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ApplicationSupportNotice, useApplicationSupport } from '../ApplicationSupport';
import type { ApplicationSupport } from '../types';
const mock = vi.hoisted(() => ({ applicationSupport: vi.fn() }));
vi.mock('../api', () => ({ desktop: true, api: mock }));
afterEach(cleanup);
beforeEach(() => vi.clearAllMocks());
const support: ApplicationSupport = {
  app_id: 'weixin',
  title: '微信',
  version: '4.1.13.65',
  supported_version: '4.1.13.65',
  identity_verified: true,
  scan_supported: true,
  cleanup_validated: false,
  rules: [
    { rule_id: 'weixin.logs', cleanup_validated: false },
    { rule_id: 'weixin.dumps', cleanup_validated: false },
  ],
  status: 'application_running',
  message: '应用仍在运行',
  running_components: ['Weixin.exe'],
  locations: ['C:\\用户\\测试\\AppData\\Roaming\\Tencent\\xwechat\\log'],
};
it('distinguishes a running application from pending real acceptance and exposes actual locations', async () => {
  const refresh = vi.fn();
  render(
    <ApplicationSupportNotice
      support={support}
      error=""
      loading={false}
      busy={false}
      onRefresh={refresh}
    />,
  );
  expect(screen.getByText(/4.1.13.65.*仍在运行/)).toBeVisible();
  expect(screen.getByText(/关闭应用不会解除验收限制/)).toBeVisible();
  expect(screen.getByText(/仍在运行：Weixin.exe/)).toBeVisible();
  await userEvent.setup().click(screen.getByText('识别依据与扫描位置'));
  expect(screen.getByText(support.locations[0])).toBeVisible();
  await userEvent.setup().click(screen.getByRole('button', { name: '重新核对应用信息' }));
  expect(refresh).toHaveBeenCalledOnce();
});
it('describes cross-version layout recognition without implying cleanup or requiring an exit', async () => {
  render(
    <ApplicationSupportNotice
      support={{
        ...support,
        app_id: 'feishu',
        title: '飞书',
        version: '99.0.0.1',
        running_components: [],
        status: 'validation_pending',
        rules: [
          {
            rule_id: 'feishu.http',
            cleanup_validated: false,
            scan_basis: 'recognized_layout',
            layout_id: 'feishu-web-profile-v1',
          },
        ],
      }}
      error=""
      loading={false}
      busy={false}
      onRefresh={vi.fn()}
    />,
  );
  expect(screen.getByText(/99.0.0.1.*已识别目录结构/)).toBeVisible();
  expect(screen.queryByText(/本机版本不在已支持范围/)).not.toBeInTheDocument();
  await userEvent.setup().click(screen.getByText('识别依据与扫描位置'));
  expect(screen.getByLabelText('各类别识别依据')).toHaveTextContent(
    '飞书网页缓存：目录结构已识别，版本仅作观察',
  );
  expect(
    screen.getByText(/版本号仅作记录；已开放规则按本机应用身份、目录结构和文件格式核验清理能力/),
  ).toBeVisible();
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
});
function Probe({ epoch }: { epoch: number }) {
  const state = useApplicationSupport(true, epoch);
  return (
    <span data-count={state.applications.length}>
      {state.loading ? 'loading' : state.error || state.applications[0]?.version}
    </span>
  );
}
it('discards stale support responses after refresh and permits a read retry', async () => {
  let late!: (v: ApplicationSupport[]) => void;
  mock.applicationSupport.mockReturnValueOnce(
    new Promise((r) => {
      late = r;
    }),
  );
  const { rerender } = render(<Probe epoch={0} />);
  mock.applicationSupport.mockResolvedValueOnce([{ ...support, version: 'new' }]);
  rerender(<Probe epoch={1} />);
  expect(screen.getByText('loading')).toHaveAttribute('data-count', '0');
  await screen.findByText('new');
  late([support]);
  await waitFor(() => expect(screen.getByText('new')).toBeVisible());
  mock.applicationSupport.mockRejectedValueOnce(new Error('read failed'));
  rerender(<Probe epoch={2} />);
  await screen.findByText('read failed');
  mock.applicationSupport.mockResolvedValueOnce([support]);
  rerender(<Probe epoch={3} />);
  await screen.findByText('4.1.13.65');
});
