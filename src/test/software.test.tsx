import { render, screen, waitFor, cleanup, act, within, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest';
import { SoftwarePage, filterSoftware } from '../SoftwarePage';
import { HistoryPage } from '../HistoryPage';
import {
  softwarePreview,
  type InstalledSoftware,
  type UninstallTask,
  type ResidualScan,
  type ResidualCleanupPlan,
} from '../softwareTypes';
import type { AppController } from '../useAppController';
const mock = vi.hoisted(() => ({
  softwareList: vi.fn(),
  prepareSoftwareUninstall: vi.fn(),
  executeSoftwareUninstall: vi.fn(),
  scanSoftwareResiduals: vi.fn(),
  softwareResiduals: vi.fn(),
  prepareSoftwareCleanup: vi.fn(),
  executeSoftwareCleanup: vi.fn(),
  verifySoftware: vi.fn(),
  apps: vi.fn(),
  uninstallHistory: vi.fn(),
}));
vi.mock('../api', () => ({ desktop: true, api: mock }));
const app: InstalledSoftware = {
  ...softwarePreview.items[0],
  id: 'real-id',
  name: '测试软件',
  can_uninstall: true,
  unavailable_reason: null,
};
function model(task?: UninstallTask): AppController {
  return {
    busy: false,
    closing: false,
    uninstall: task ?? null,
    reconcile: vi.fn().mockResolvedValue([]),
  } as unknown as AppController;
}
function task(phase: UninstallTask['phase']): UninstallTask {
  return {
    id: 'task',
    software: app,
    phase,
    started_at: 1,
    updated_at: 2,
    progress: null,
    message: '状态说明',
    can_scan_residuals: ['removed', 'residuals_ready', 'finished'].includes(phase),
    residual_scan_id: phase === 'residuals_ready' ? 'scan-1' : null,
    residual: {
      deleted_files: 0,
      deleted_directories: 0,
      deleted_bytes: '0',
      skipped: 0,
      failed: 0,
      unknown: 0,
    },
  };
}
beforeEach(() => {
  Object.values(mock).forEach((fn) => fn.mockReset());
  mock.softwareList.mockResolvedValue({ items: [app], warnings: [] });
});
afterEach(cleanup);
describe('software removal', () => {
  it('shows software icons in the list and details, falling back when an image fails', async () => {
    const icon = 'data:image/png;base64,iVBORw0KGgo=';
    mock.softwareList.mockResolvedValue({
      items: [{ ...app, icon_data_url: icon }],
      warnings: [],
    });
    render(<SoftwarePage model={model()} active />);
    const trigger = await screen.findByRole('button', { name: /测试软件/ });
    const image = trigger.querySelector('img')!;
    expect(image).toHaveAttribute('src', icon);
    expect(image).toHaveAttribute('alt', '');
    await userEvent.click(trigger);
    expect(screen.getByRole('dialog').querySelector('img')).toHaveAttribute('src', icon);
    fireEvent.error(image);
    expect(trigger.querySelector('img')).toBeNull();
    expect(trigger.querySelector('.software-icon svg')).not.toBeNull();
  });

  it('keeps default icons for older records and rejects remote icon URLs', async () => {
    mock.softwareList.mockResolvedValue({
      items: [
        app,
        { ...app, id: 'remote', name: '另一软件', icon_data_url: 'https://example.com/icon.png' },
      ],
      warnings: [],
    });
    render(<SoftwarePage model={model()} active />);
    for (const name of [/测试软件/, /另一软件/]) {
      const row = await screen.findByRole('button', { name });
      expect(row.querySelector('img')).toBeNull();
      expect(row.querySelector('.software-icon svg')).not.toBeNull();
    }
  });

  it('shows independent inspection and cleanup support without blocking a valid uninstaller', async () => {
    const partial: InstalledSoftware = {
      ...app,
      publisher: '  ',
      version: '\t',
      installed_on: null,
      estimated_bytes: null,
      residual_capabilities: [
        { kind: 'cache', inspection: 'fixed_locations', cleanup: 'not_validated' },
        { kind: 'settings', inspection: 'unsupported', cleanup: 'unsupported' },
        { kind: 'local_data', inspection: 'unsupported', cleanup: 'unsupported' },
      ],
      missing_information: [
        'publisher',
        'version',
        'estimated_bytes',
        'installed_on',
        'install_location',
      ],
    };
    mock.softwareList.mockResolvedValue({ items: [partial], warnings: [] });
    const user = userEvent.setup();
    render(<SoftwarePage model={model()} active />);
    const trigger = await screen.findByRole('button', { name: /测试软件/ });
    await user.click(trigger);
    expect(screen.getByRole('button', { name: '卸载此软件' })).toBeVisible();
    expect(screen.getByText('软件信息与清理范围').parentElement).not.toHaveAttribute('open');
    await user.click(screen.getByText('软件信息与清理范围'));
    const table = screen.getByRole('table', { name: '残留分类能力' });
    expect(within(table).getByRole('row', { name: /缓存与日志/ })).toHaveTextContent(
      '固定位置尚未开放',
    );
    expect(within(table).getByRole('row', { name: /^设置/ })).toHaveTextContent('暂不支持暂不支持');
    expect(within(table).getByRole('row', { name: /本地资料/ })).toHaveTextContent(
      '暂不支持暂不支持',
    );
    expect(
      screen.getByText(/未提供：发布者、版本、登记大小、安装日期、安装位置/),
    ).toBeInTheDocument();
    expect(screen.getByText(/可检查不代表可清理/)).toHaveTextContent(
      '登录状态、浏览记录、离线内容或文档备份',
    );
    expect(screen.getByRole('button', { name: '卸载此软件' })).toBeEnabled();
    expect(mock.prepareSoftwareUninstall).not.toHaveBeenCalled();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(trigger).toHaveFocus());
  });
  it('keeps store data impact and removal prohibition independent from residual descriptions', async () => {
    mock.softwareList.mockResolvedValue({
      items: [
        {
          ...app,
          source: 'store',
          can_uninstall: false,
          unavailable_reason: '系统禁止移除此应用',
          residual_capabilities: [
            { kind: 'local_data', inspection: 'fixed_locations', cleanup: 'requires_verification' },
          ],
        },
      ],
      warnings: [],
    });
    render(<SoftwarePage model={model()} active />);
    await userEvent.click(await screen.findByRole('button', { name: /测试软件/ }));
    expect(screen.getByText(/标准卸载可能同时删除应用数据/)).toBeInTheDocument();
    await userEvent.click(screen.getByText('软件信息与清理范围'));
    expect(screen.getByRole('row', { name: /本地资料/ })).toHaveTextContent('需逐项核验');
    expect(screen.getByText('系统禁止移除此应用')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '卸载此软件' })).toBeDisabled();
  });
  it('preserves older descriptions without inferring category support from the old aggregate flag', async () => {
    const legacy = {
      ...app,
      residual_capabilities: undefined,
      missing_information: undefined,
      residual_note: '旧版残留说明',
    };
    mock.softwareList.mockResolvedValue({ items: [legacy], warnings: [] });
    render(<SoftwarePage model={model()} active />);
    await userEvent.click(await screen.findByRole('button', { name: /测试软件/ }));
    await userEvent.click(screen.getByText('软件信息与清理范围'));
    expect(screen.getByText('旧版残留说明')).toBeInTheDocument();
    expect(screen.queryByRole('table', { name: '残留分类能力' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '卸载此软件' })).toBeEnabled();
  });
  it('loads only on entry and searches without changing system state', async () => {
    const view = render(<SoftwarePage model={model()} active={false} />);
    expect(mock.softwareList).not.toHaveBeenCalled();
    view.rerender(<SoftwarePage model={model()} active />);
    await screen.findByRole('button', { name: /测试软件/ });
    await userEvent.type(screen.getByLabelText('搜索名称或发布者'), 'missing');
    expect(screen.getByText('没有找到匹配的软件')).toBeInTheDocument();
    expect(mock.executeSoftwareUninstall).not.toHaveBeenCalled();
  });
  it('confirms exactly the server plan and prevents repeated submission', async () => {
    const user = userEvent.setup();
    mock.prepareSoftwareUninstall.mockResolvedValue({
      id: 'sealed-plan',
      software: app,
      impact: '可能移除本地资料',
    });
    mock.executeSoftwareUninstall.mockResolvedValue('task');
    render(<SoftwarePage model={model()} active />);
    await user.click(await screen.findByRole('button', { name: /测试软件/ }));
    await user.click(screen.getByRole('button', { name: '卸载此软件' }));
    expect(await screen.findByText('可能移除本地资料')).toBeInTheDocument();
    expect(mock.executeSoftwareUninstall).not.toHaveBeenCalled();
    await user.dblClick(screen.getByRole('button', { name: '确认卸载' }));
    await waitFor(() => expect(mock.executeSoftwareUninstall).toHaveBeenCalledTimes(1));
    expect(mock.executeSoftwareUninstall).toHaveBeenCalledWith('sealed-plan');
  });
  it('keeps personal data unselected and readonly residues disabled', async () => {
    mock.softwareResiduals.mockResolvedValue({
      task_id: 'task',
      scan_id: 'scan-1',
      total: 2,
      offset: 0,
      complete: true,
      supported: true,
      warnings: [],
      items: [
        {
          id: 'data',
          path: 'C:\\fixture\\settings',
          category: 'data',
          bytes: '12',
          directory: false,
          selectable: true,
          reason: null,
        },
        {
          id: 'shared',
          path: 'C:\\fixture\\shared',
          category: 'cache',
          bytes: '13',
          directory: false,
          selectable: false,
          reason: '共享资料保留',
        },
      ],
    });
    render(<SoftwarePage model={model(task('residuals_ready'))} active />);
    const data = await screen.findByRole('checkbox', { name: /settings/ });
    expect(data).not.toBeChecked();
    expect(screen.getByText('已保留 1 项').parentElement).not.toHaveAttribute('open');
    await userEvent.click(screen.getByText('已保留 1 项'));
    const shared = screen.getByRole('checkbox', { name: /shared/ });
    expect(shared).toBeDisabled();
    expect(screen.getByRole('button', { name: '清理已选残留' })).toBeDisabled();
    await userEvent.click(data);
    expect(screen.getByRole('button', { name: '清理已选残留' })).toBeEnabled();
  });
  it('never scans residues while uninstall outcome remains unknown', () => {
    render(<SoftwarePage model={model(task('unknown'))} active />);
    expect(mock.scanSoftwareResiduals).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: '检查卸载结果' })).toBeEnabled();
    expect(screen.queryByRole('button', { name: '检查残留' })).not.toBeInTheDocument();
  });
  it('keeps interrupted cleanup counts visible without treating unprocessed files as deleted', () => {
    const interrupted = task('unknown');
    interrupted.uninstall_status = 'removed';
    interrupted.residual = {
      deleted_files: 1,
      deleted_directories: 0,
      deleted_bytes: '12',
      skipped: 0,
      failed: 0,
      unknown: 1,
      unprocessed: 2,
    };
    render(<SoftwarePage model={model(interrupted)} active />);
    expect(screen.getByText('残留结果待核对')).toBeInTheDocument();
    expect(screen.getByText(/标准卸载：已核对移除/)).toHaveTextContent('残留实际删除：12 B');
    expect(screen.getByText(/实际删除 1 个文件、0 个空文件夹，共 12 B/)).toHaveTextContent(
      '保留 0 项，失败 0 项，待核对 1 项，未处理 2 项',
    );
    expect(screen.queryByRole('button', { name: '检查残留' })).not.toBeInTheDocument();
  });
  it('does not infer successful uninstall from ambiguous older interrupted records', () => {
    render(<SoftwarePage model={model(task('unknown'))} active />);
    expect(screen.getByText(/标准卸载：结果待核对/)).toBeInTheDocument();
    expect(screen.queryByText(/标准卸载：已核对移除/)).not.toBeInTheDocument();
    expect(mock.scanSoftwareResiduals).not.toHaveBeenCalled();
  });
  it('keeps confirmed uninstall visible in interrupted residual history', async () => {
    const interrupted = { ...task('unknown'), uninstall_status: 'removed' as const };
    mock.uninstallHistory.mockResolvedValue([interrupted]);
    render(
      <HistoryPage
        model={
          {
            ...model(interrupted),
            history: [],
            historyEpoch: 0,
            historyLoading: false,
            historyError: '',
          } as unknown as AppController
        }
      />,
    );
    expect(await screen.findByText('测试软件 · 残留结果待核对')).toBeInTheDocument();
    expect(screen.getByText('标准卸载：已核对移除。')).toBeInTheDocument();
  });
  it('shows zero unprocessed entries for older task summaries', () => {
    render(<SoftwarePage model={model(task('finished'))} active />);
    expect(screen.getByText(/实际删除 0 个文件/)).toHaveTextContent('未处理 0 项');
  });
  it('refreshes history for a stopped cleanup even within the same timestamp', async () => {
    const current = task('cleaning_residuals');
    const historyModel = {
      ...model(current),
      history: [],
      historyEpoch: 0,
      historyLoading: false,
      historyError: '',
    } as unknown as AppController;
    mock.uninstallHistory.mockResolvedValue([current]);
    const view = render(<HistoryPage model={historyModel} />);
    expect(await screen.findByText(/测试软件 · 正在清理残留/)).toBeInTheDocument();
    const stopped = {
      ...current,
      phase: 'finished' as const,
      residual: { ...current.residual, deleted_files: 1, deleted_bytes: '12', unprocessed: 2 },
    };
    mock.uninstallHistory.mockResolvedValue([stopped]);
    view.rerender(<HistoryPage model={{ ...historyModel, uninstall: stopped }} />);
    expect(await screen.findByText(/测试软件 · 残留处理结束/)).toBeInTheDocument();
    expect(screen.getByText(/残留文件 1 个/)).toHaveTextContent('待核对 0 项，未处理 2 项');
    expect(mock.uninstallHistory).toHaveBeenCalledTimes(2);
    view.rerender(<HistoryPage model={{ ...historyModel, uninstall: { ...stopped } }} />);
    expect(mock.uninstallHistory).toHaveBeenCalledTimes(2);
  });
  it('previews the frozen data selection and requires a separate cleanup confirmation', async () => {
    const user = userEvent.setup();
    const candidate = {
      id: 'data-1',
      path: 'C:\\fixture\\settings.json',
      category: 'data',
      bytes: '12',
      directory: false,
      selectable: true,
      reason: null,
    };
    mock.softwareResiduals.mockResolvedValue({
      task_id: 'task',
      scan_id: 'scan-1',
      total: 1,
      offset: 0,
      complete: true,
      supported: true,
      warnings: [],
      items: [candidate],
    });
    mock.prepareSoftwareCleanup.mockResolvedValue({
      id: 'frozen-cleanup',
      task_id: 'task',
      files: 1,
      directories: 0,
      bytes: '12',
      includes_data: true,
      preview: [candidate],
    });
    mock.executeSoftwareCleanup.mockResolvedValue(undefined);
    render(<SoftwarePage model={model(task('residuals_ready'))} active />);
    await user.click(await screen.findByRole('checkbox', { name: /settings.json/ }));
    await user.click(screen.getByRole('button', { name: '清理已选残留' }));
    expect(await screen.findByRole('alertdialog')).toHaveTextContent(candidate.path);
    expect(screen.getByRole('alertdialog')).toHaveTextContent('不会放入回收站');
    expect(mock.prepareSoftwareCleanup).toHaveBeenCalledWith('task', ['data-1']);
    expect(mock.executeSoftwareCleanup).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: '返回检查' }));
    await waitFor(() => expect(screen.getByRole('button', { name: '清理已选残留' })).toHaveFocus());
    expect(screen.getByRole('checkbox', { name: /settings.json/ })).toBeChecked();
    expect(mock.executeSoftwareCleanup).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: '清理已选残留' }));
    await screen.findByRole('alertdialog');
    await user.click(screen.getByRole('button', { name: '确认永久删除' }));
    await waitFor(() => expect(mock.executeSoftwareCleanup).toHaveBeenCalledWith('frozen-cleanup'));
  });
  it('shows a loading failure rather than an empty successful list', async () => {
    mock.softwareList.mockRejectedValue(new Error('读取失败'));
    render(<SoftwarePage model={model()} active />);
    expect(await screen.findByRole('alert')).toHaveTextContent('读取失败');
    expect(screen.queryByText('没有发现可展示的软件')).not.toBeInTheDocument();
  });
  it('does not offer or auto-start a residual scan after recovered authority is lost', async () => {
    const recovered = {
      ...task('removed'),
      can_scan_residuals: false,
      message: '已核对安装登记：软件已移除。上次卸载范围未保留，本次不能检查或清理残留',
    };
    render(<SoftwarePage model={model(recovered)} active />);
    expect(screen.queryByRole('region', { name: '本次卸载任务' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '查看处理结果' }));
    expect(screen.getByText(/上次卸载范围未保留/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '检查残留' })).not.toBeInTheDocument();
    expect(mock.scanSoftwareResiduals).not.toHaveBeenCalled();
  });
  function scan(scanId: string, path: string, total = 1): ResidualScan {
    return {
      task_id: 'task',
      scan_id: scanId,
      total,
      offset: 0,
      complete: true,
      supported: true,
      warnings: [],
      items: [
        {
          id: scanId,
          path,
          category: 'data',
          bytes: '12',
          directory: false,
          selectable: true,
          reason: null,
        },
      ],
    };
  }
  it('continues one automatic read-only scan when the user changes pages', async () => {
    mock.scanSoftwareResiduals.mockResolvedValue(undefined);
    const current = task('removed');
    const view = render(<SoftwarePage model={model(current)} active={false} />);
    await waitFor(() => expect(mock.scanSoftwareResiduals).toHaveBeenCalledTimes(1));
    view.rerender(<SoftwarePage model={model(current)} active />);
    await waitFor(() => expect(screen.getByRole('button', { name: '重新检查残留' })).toBeEnabled());
    expect(mock.scanSoftwareResiduals).toHaveBeenCalledTimes(1);
    expect(mock.executeSoftwareCleanup).not.toHaveBeenCalled();
    expect(mock.executeSoftwareUninstall).not.toHaveBeenCalled();
  });
  it('defers an automatic scan while another operation is busy', async () => {
    const current = task('removed');
    const view = render(<SoftwarePage model={{ ...model(current), busy: true }} active={false} />);
    expect(mock.scanSoftwareResiduals).not.toHaveBeenCalled();
    mock.scanSoftwareResiduals.mockResolvedValue(undefined);
    view.rerender(<SoftwarePage model={model(current)} active={false} />);
    await waitFor(() => expect(mock.scanSoftwareResiduals).toHaveBeenCalledTimes(1));
  });
  it('returns to the list without deleting and reopens results with selections cleared', async () => {
    const user = userEvent.setup();
    mock.softwareResiduals.mockResolvedValue(scan('scan-1', 'C:\\fixture\\data.json'));
    render(<SoftwarePage model={model(task('residuals_ready'))} active />);
    await user.click(await screen.findByRole('checkbox', { name: /data.json/ }));
    expect(screen.getByRole('region', { name: '本次卸载任务' })).toBeVisible();
    expect(screen.queryByRole('searchbox', { name: '搜索名称或发布者' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '暂不清理' }));
    await waitFor(() =>
      expect(screen.getByRole('searchbox', { name: '搜索名称或发布者' })).toHaveFocus(),
    );
    await user.click(screen.getByRole('button', { name: '查看处理结果' }));
    expect(await screen.findByRole('checkbox', { name: /data.json/ })).not.toBeChecked();
    expect(screen.getByRole('region', { name: '本次卸载任务' })).toHaveFocus();
    expect(mock.scanSoftwareResiduals).not.toHaveBeenCalled();
    expect(mock.prepareSoftwareCleanup).not.toHaveBeenCalled();
    expect(mock.executeSoftwareCleanup).not.toHaveBeenCalled();
  });
  it('selects only visible eligible cache entries and never personal or retained files', async () => {
    const user = userEvent.setup();
    const result = scan('scan-1', 'C:\\fixture\\data.json');
    const base = result.items[0];
    result.items.push(
      { ...base, id: 'cache-a', path: 'C:\\fixture\\cache-a', category: 'cache' },
      { ...base, id: 'cache-b', path: 'C:\\fixture\\cache-b', category: 'cache' },
      { ...base, id: 'shared', path: 'C:\\fixture\\shared', category: 'cache', selectable: false },
    );
    result.total = 4;
    mock.softwareResiduals.mockResolvedValue(result);
    render(<SoftwarePage model={model(task('residuals_ready'))} active />);
    await user.click(await screen.findByRole('button', { name: '选择本页缓存' }));
    expect(screen.getByRole('checkbox', { name: /cache-a/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /cache-b/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /data.json/ })).not.toBeChecked();
    await user.click(screen.getByText('已保留 1 项'));
    expect(screen.getByRole('checkbox', { name: /shared/ })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: /shared/ })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: '取消本页缓存选择' }));
    expect(screen.getByText('已选 0 项')).toBeInTheDocument();
    expect(mock.executeSoftwareCleanup).not.toHaveBeenCalled();
  });
  it('keeps completed restored work compact and shows details only on request', async () => {
    const restored = { ...task('finished'), can_scan_residuals: false };
    render(<SoftwarePage model={model(restored)} active />);
    expect(await screen.findByRole('searchbox', { name: '搜索名称或发布者' })).toBeVisible();
    expect(screen.queryByRole('region', { name: '本次卸载任务' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '查看处理结果' }));
    expect(screen.getByText('查看处理详情').parentElement).not.toHaveAttribute('open');
    expect(screen.getByText(/实际删除 0 个文件/)).not.toBeVisible();
    await userEvent.click(screen.getByText('查看处理详情'));
    expect(screen.getByText(/实际删除 0 个文件/)).toBeVisible();
    expect(mock.scanSoftwareResiduals).not.toHaveBeenCalled();
  });
  it('offers completion instead of an unusable cleanup action for retained-only results', async () => {
    const result = scan('scan-1', 'C:\\fixture\\shared');
    result.items[0].selectable = false;
    mock.softwareResiduals.mockResolvedValue(result);
    render(<SoftwarePage model={model(task('residuals_ready'))} active />);
    expect(await screen.findByText('找到的残留暂不支持清理，已全部保留。')).toBeVisible();
    expect(screen.queryByRole('button', { name: '清理已选残留' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '完成' }));
    expect(screen.getByRole('searchbox', { name: '搜索名称或发布者' })).toHaveFocus();
    expect(mock.executeSoftwareCleanup).not.toHaveBeenCalled();
  });
  it('refreshes a fast rescan with unchanged phase and timestamp and resets paging and selection', async () => {
    const user = userEvent.setup();
    mock.softwareResiduals.mockResolvedValue(scan('scan-1', 'C:\\fixture\\old.json', 201));
    const view = render(<SoftwarePage model={model(task('residuals_ready'))} active />);
    await user.click(await screen.findByRole('checkbox', { name: /old.json/ }));
    expect(screen.getByText('已选 1 项')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '下一页' }));
    await waitFor(() => expect(mock.softwareResiduals).toHaveBeenCalledWith('task', 200, ''));
    mock.softwareResiduals.mockResolvedValue(scan('scan-2', 'C:\\fixture\\new.json'));
    view.rerender(
      <SoftwarePage
        model={model({ ...task('residuals_ready'), residual_scan_id: 'scan-2' })}
        active
      />,
    );
    const checkbox = await screen.findByRole('checkbox', { name: /new.json/ });
    expect(checkbox).not.toBeChecked();
    expect(screen.queryByRole('checkbox', { name: /old.json/ })).not.toBeInTheDocument();
    expect(screen.getByText('已选 0 项')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '清理已选残留' })).toBeDisabled();
    expect(mock.softwareResiduals).toHaveBeenLastCalledWith('task', 0, '');
  });
  it('ignores a previous scan response that arrives after the replacement manifest', async () => {
    let finishOld!: (scan: ResidualScan) => void;
    mock.softwareResiduals.mockReturnValueOnce(
      new Promise<ResidualScan>((resolve) => {
        finishOld = resolve;
      }),
    );
    const view = render(<SoftwarePage model={model(task('residuals_ready'))} active />);
    await waitFor(() => expect(mock.softwareResiduals).toHaveBeenCalledTimes(1));
    mock.softwareResiduals.mockResolvedValue(scan('scan-2', 'C:\\fixture\\new.json'));
    view.rerender(
      <SoftwarePage
        model={model({ ...task('residuals_ready'), residual_scan_id: 'scan-2' })}
        active
      />,
    );
    await screen.findByRole('checkbox', { name: /new.json/ });
    await act(async () => finishOld(scan('scan-1', 'C:\\fixture\\old.json')));
    expect(screen.queryByRole('checkbox', { name: /old.json/ })).not.toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: /new.json/ })).not.toBeChecked();
  });
  it('discards a late cleanup confirmation from a replaced manifest', async () => {
    const user = userEvent.setup();
    let finishPlan!: (plan: ResidualCleanupPlan) => void;
    const first = scan('scan-1', 'C:\\fixture\\old.json');
    mock.softwareResiduals.mockResolvedValue(first);
    mock.prepareSoftwareCleanup.mockReturnValue(
      new Promise<ResidualCleanupPlan>((resolve) => {
        finishPlan = resolve;
      }),
    );
    const view = render(<SoftwarePage model={model(task('residuals_ready'))} active />);
    await user.click(await screen.findByRole('checkbox', { name: /old.json/ }));
    await user.click(screen.getByRole('button', { name: '清理已选残留' }));
    mock.softwareResiduals.mockResolvedValue(scan('scan-2', 'C:\\fixture\\new.json'));
    view.rerender(
      <SoftwarePage
        model={model({ ...task('residuals_ready'), residual_scan_id: 'scan-2' })}
        active
      />,
    );
    await screen.findByRole('checkbox', { name: /new.json/ });
    await act(async () =>
      finishPlan({
        id: 'old-plan',
        task_id: 'task',
        files: 1,
        directories: 0,
        bytes: '12',
        includes_data: true,
        preview: first.items,
      }),
    );
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(mock.executeSoftwareCleanup).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: '清理已选残留' })).toBeDisabled();
  });
  it('sorts unknown estimates last and searches publisher', () => {
    const unknown = { ...app, id: 'unknown', estimated_bytes: null, publisher: 'Acme' };
    expect(filterSoftware([unknown, app], '', 'all', 'size')[1].id).toBe('unknown');
    expect(filterSoftware([unknown, app], 'acme', 'all', 'name')).toEqual([unknown]);
  });
});
