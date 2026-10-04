import { useEffect, useState } from 'react';
import { api, desktop } from './api';
import { ipcError } from './coordinator';
import type { ApplicationSupport } from './types';
import { applicationCondition } from './applicationCapability';
import { reasonLabels } from './i18n';
import rules from './previewRules';

export function useApplicationSupport(enabled: boolean, epoch: number) {
  const [result, setResult] = useState<{
    epoch: number;
    applications: ApplicationSupport[];
    error: string;
  } | null>(null);
  useEffect(() => {
    if (!enabled || !desktop) return;
    let current = true;
    api
      .applicationSupport()
      .then((result) => {
        if (current) setResult({ epoch, applications: result, error: '' });
      })
      .catch((e) => {
        if (current) {
          setResult({ epoch, applications: [], error: ipcError(e).message });
        }
      });
    return () => {
      current = false;
    };
  }, [enabled, epoch]);
  const current = enabled && desktop && result?.epoch === epoch ? result : null;
  return {
    applications: current?.applications ?? [],
    error: current?.error ?? '',
    loading: enabled && desktop && !current,
  };
}

export function ApplicationSupportNotice({
  support,
  loading,
  error,
  busy,
  onRefresh,
}: {
  support?: ApplicationSupport;
  loading: boolean;
  error: string;
  busy: boolean;
  onRefresh: () => void;
}) {
  return (
    <div className="application-support" aria-live="polite">
      <p>
        {loading
          ? '正在核对本机应用版本与身份…'
          : error
            ? `应用信息读取失败：${error}`
            : support
              ? `程序版本 ${support.version ?? '未确认'} · ${applicationCondition(support)}`
              : desktop
                ? '尚未确认应用身份或版本。'
                : '网页预览不读取本机应用。'}
      </p>
      {support?.rules?.length && support.rules.every((r) => !r.cleanup_validated) ? (
        <p className="support-boundary">
          这些规则仅只读扫描，关闭应用不会解除验收限制。扫描大小不代表整个应用占用。
        </p>
      ) : support?.rules?.length ? (
        <p>各类别能力分别核对；已开放项目仍需扫描并通过执行检查。</p>
      ) : (
        <p>逐规则能力未确认，暂不参与清理；可更新应用信息。</p>
      )}
      {support && (
        <details>
          <summary>识别依据与扫描位置</summary>
          <p>
            已核实样本的文件版本：{support.supported_version}
            。版本号仅作记录；已开放规则按本机应用身份、目录结构和文件格式核验清理能力。
            安装盘与资料所在盘可能不同；以下位置不一定均存在。
          </p>
          {!!support.rules?.length && (
            <ul aria-label="各类别识别依据">
              {support.rules.map((item) => (
                <li key={item.rule_id}>
                  {rules.find((r) => r.id === item.rule_id)?.title ?? '其他类别'}：
                  {item.scan_basis === 'verified_version'
                    ? '与已核实样本版本一致，目录结构已识别'
                    : item.scan_basis === 'recognized_layout'
                      ? '目录结构已识别，版本仅作观察'
                      : '目录结构尚未确认，以扫描结果为准'}
                  {'；'}
                  {!item.cleanup_validated
                    ? '尚未开放清理'
                    : !item.cleanup_versions?.length || item.cleanup_block === undefined
                      ? '清理能力未确认'
                      : `清理验收样本：${item.cleanup_versions.join('、')}（不限制版本）；${item.cleanup_block ? reasonLabels[item.cleanup_block] : '当前条件允许，仍需扫描核验'}`}
                </li>
              ))}
            </ul>
          )}
          <ul>
            {support.locations.map((path) => (
              <li key={path}>{path}</li>
            ))}
          </ul>
        </details>
      )}
      <button className="result-link" disabled={busy || loading || !desktop} onClick={onRefresh}>
        重新核对应用信息
      </button>
    </div>
  );
}
