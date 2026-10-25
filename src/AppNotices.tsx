import { CircleAlert, LoaderCircle, X } from 'lucide-react';
import { analysisStates } from './AnalysisPanel';
import { text as t, ui } from './i18n';
import { uninstallActive, uninstallLabels } from './softwareTypes';
import type { AppController } from './useAppController';

export function AppNotices({ model }: { model: AppController }) {
  const {
    tab,
    setTab,
    analysis,
    pending,
    stopping,
    uncertain,
    closing,
    stopTask,
    error,
    setError,
    notice,
    setNotice,
    initializing,
    initError,
    setInitEpoch,
    checking,
    retryStatus,
  } = model;
  return (
    <>
      {model.uninstall && uninstallActive(model.uninstall.phase) && tab !== 'software' && (
        <div className="message info" role="status">
          <span>
            {model.uninstall.software.name}：{uninstallLabels[model.uninstall.phase]}
          </span>
          <button className="button secondary small" onClick={() => setTab('software')}>
            查看卸载任务
          </button>
        </div>
      )}
      {analysis?.state === 'scanning' && tab !== 'analysis' && (
        <div className="message info" role="status">
          <span>
            {analysisStates[analysis.state]} · 已检查 {analysis.checked.toLocaleString()} 项
          </span>
          <button className="button secondary small" onClick={() => setTab('analysis')}>
            查看空间分析
          </button>
          <button
            className="button secondary small"
            disabled={pending || stopping || uncertain || closing}
            onClick={stopTask}
          >
            {stopping ? '停止中…' : '停止分析'}
          </button>
        </div>
      )}

      {error && tab !== 'settings' && (
        <div className="message error" role="alert">
          <CircleAlert size={19} />
          <span>{error}</span>
          <button className="icon-button" aria-label={ui.dismissError} onClick={() => setError('')}>
            <X size={17} />
          </button>
        </div>
      )}
      {notice && (
        <div className="message info" role="status">
          <CircleAlert size={19} />
          <span>{notice}</span>
          <button
            className="icon-button"
            aria-label={ui.dismissNotice}
            onClick={() => setNotice('')}
          >
            <X size={17} />
          </button>
        </div>
      )}
      {initializing && (
        <div className="message info" role="status">
          正在读取设置并核对后台状态…
        </div>
      )}
      {initError && (
        <div className="message error" role="alert">
          <span>启动数据加载失败：{initError}</span>
          <button className="button secondary small" onClick={() => setInitEpoch((v) => v + 1)}>
            重新加载
          </button>
        </div>
      )}
      {checking !== 'ready' && !initializing && (
        <div className="message info" role="status">
          <span>
            {checking === 'checking'
              ? '正在核对请求是否已受理，请勿重复操作…'
              : '暂时无法确认任务状态，请重新检查。'}
          </span>
          <button className="button secondary small" onClick={retryStatus}>
            重新核对请求
          </button>
        </div>
      )}
      {uncertain && !initializing && !initError && (
        <div className="message info" role="status">
          <span>暂时无法确认任务状态</span>
          <button className="button secondary small" onClick={retryStatus}>
            重新检查状态
          </button>
          <button className="button secondary small" onClick={() => setInitEpoch((v) => v + 1)}>
            重新加载
          </button>
        </div>
      )}
      {closing && (
        <div role="status" className="message info">
          <LoaderCircle className="spin" size={18} />
          {t.closing}
        </div>
      )}
    </>
  );
}
