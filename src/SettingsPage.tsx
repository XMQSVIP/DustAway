import { useState, useEffect } from 'react';
import {
  ChevronRight,
  Download,
  ExternalLink,
  FolderOpen,
  Monitor,
  Moon,
  Plus,
  SlidersHorizontal,
  Sun,
  Wind,
  X,
} from 'lucide-react';
import { LayerScene } from './LayerScene';
import { ProductCapabilities } from './ProductCapabilities';
import { api, desktop } from './api';
import { dynamic, text as t, ui } from './i18n';
import type { AppController } from './useAppController';
import type { SpatialPreferences } from './spatialPreferences';

export function SettingsPage({
  model,
  preferences,
  updatePreferences,
  settingsLeaf,
  setSettingsLeaf,
}: {
  model: AppController;
  preferences: SpatialPreferences;
  updatePreferences: (patch: Partial<SpatialPreferences>) => void;
  settingsLeaf: string;
  setSettingsLeaf: (leaf: string) => void;
}) {
  const { boot, error, busy, saving, closing, save, addExclusion, guard, setNotice } = model;
  const [cacheLocation, setCacheLocation] = useState<string | null>(null);
  useEffect(() => {
    if (desktop)
      void api
        .analysisStorage()
        .then(setCacheLocation)
        .catch(() => {});
  }, []);
  if (!boot) return null;
  return (
    <section className="page-stage settings-book">
      <header className="page-heading">
        <div>
          <h1>{ui.settings}</h1>
          <p>{ui.settingsDescription}</p>
        </div>
        <SlidersHorizontal size={25} className="muted" />
      </header>
      <nav className="settings-spines" aria-label="设置折页">
        {[
          { id: 'appearance', title: '外观', note: '主题与悬浮动效' },
          { id: 'exclusions', title: '保留清单', note: '扫描和清理时跳过' },
          { id: 'tools', title: '帮助与关于', note: '指引、诊断与版本' },
        ].map((item) => (
          <button
            key={item.id}
            className="settings-spine"
            data-settings-leaf={item.id}
            aria-label={item.title}
            aria-current={settingsLeaf === item.id ? 'page' : undefined}
            onClick={() => setSettingsLeaf(item.id)}
          >
            <span className="setting-hinge" aria-hidden="true" />
            <span>
              <strong>{item.title}</strong>
              <small>{item.note}</small>
            </span>
            <ChevronRight size={16} />
          </button>
        ))}
      </nav>
      <div className="settings-leaves">
        {error && (
          <p className="message error" role="alert">
            {error}
          </p>
        )}
        <section className="settings-section setting-leaf" hidden={settingsLeaf !== 'appearance'}>
          <h2>{ui.appearance}</h2>
          <p>{ui.appearanceDescription}</p>
          <div className="theme-options">
            {(
              [
                { id: 'system', name: ui.systemTheme, icon: Monitor },
                { id: 'light', name: ui.lightTheme, icon: Sun },
                { id: 'dark', name: ui.darkTheme, icon: Moon },
              ] as const
            ).map((theme) => (
              <button
                key={theme.id}
                aria-pressed={boot.settings.theme === theme.id}
                disabled={saving || closing || !desktop}
                className={boot.settings.theme === theme.id ? 'selected' : ''}
                onClick={() => save({ theme: theme.id })}
              >
                <div className="theme-preview" data-preview={theme.id}>
                  <LayerScene compact />
                </div>
                <theme.icon size={20} />
                {theme.name}
              </button>
            ))}
          </div>
          <fieldset className="motion-options">
            <legend>空间动效</legend>
            <p>雕塑旁的小块独立悬浮；减少动效时保持静止。</p>
            <div className="theme-options">
              <button
                aria-pressed={preferences.motion === 'system'}
                onClick={() => updatePreferences({ motion: 'system' })}
              >
                跟随系统动效
              </button>
              <button
                aria-pressed={preferences.motion === 'reduced'}
                onClick={() => updatePreferences({ motion: 'reduced' })}
              >
                减少动效
              </button>
            </div>
          </fieldset>
        </section>
        <section className="settings-section setting-leaf" hidden={settingsLeaf !== 'exclusions'}>
          <div className="section-heading">
            <h2>{t.exclusions}</h2>
            <div className="inline-actions">
              <button
                className="button secondary small"
                disabled={busy || saving || !desktop}
                onClick={() => addExclusion('file')}
              >
                <Plus size={15} />
                {ui.file}
              </button>
              <button
                className="button secondary small"
                disabled={busy || saving || !desktop}
                onClick={() => addExclusion('folder')}
              >
                <Plus size={15} />
                {ui.folder}
              </button>
            </div>
          </div>
          <p>这些文件、文件夹和类别会在扫描及清理时跳过。文件夹包含全部子目录。</p>
          {boot.settings.exclusions.length ? (
            <ul className="exclusion-list">
              {boot.settings.exclusions.map((path) => (
                <li key={path}>
                  <FolderOpen size={17} />
                  <span>{path}</span>
                  <button
                    className="icon-button"
                    aria-label={dynamic.removeExclusion(path)}
                    disabled={busy || saving}
                    onClick={() =>
                      save({
                        exclusions: boot.settings.exclusions.filter((p) => p !== path),
                      })
                    }
                  >
                    <X size={17} />
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <div className="settings-empty">{t.noExclusions}</div>
          )}
          <details className="rule-exclusions">
            <summary>{ui.excludeCategories}</summary>
            {boot.rules.map((rule) => (
              <label key={rule.id}>
                <input
                  type="checkbox"
                  disabled={busy || saving || !desktop}
                  checked={boot.settings.excluded_rules.includes(rule.id)}
                  onChange={() =>
                    save({
                      excluded_rules: boot.settings.excluded_rules.includes(rule.id)
                        ? boot.settings.excluded_rules.filter((id) => id !== rule.id)
                        : [...boot.settings.excluded_rules, rule.id],
                    })
                  }
                />
                {rule.title}
              </label>
            ))}
          </details>
        </section>
        <section className="settings-section setting-leaf" hidden={settingsLeaf !== 'tools'}>
          <h2>帮助与关于</h2>
          <details>
            <summary>空间分析临时索引</summary>
            <p>
              本次分析的路径和元数据会临时存到本机，会话结束后清除；异常退出的残留在下次启动清理。索引最多占用
              4 GiB，并为所在磁盘保留至少 2 GiB 可用空间。
            </p>
            <p>更换位置会清除当前分析结果和自选清单，需要重新分析。</p>
            {cacheLocation && <p className="full-path">{cacheLocation}</p>}
            <button
              className="button secondary"
              disabled={busy || saving || closing || !desktop}
              onClick={() =>
                guard(async () => {
                  const path = await api.pickAnalysisStorage();
                  if (path) {
                    setCacheLocation(path);
                    model.setPlans([]);
                    await model.reconcile(['user']);
                    setNotice('临时索引位置已更新，请重新开始空间分析。');
                  }
                })
              }
            >
              选择索引位置
            </button>
          </details>

          <p>{ui.diagnosticsPrivacy}</p>
          <div className="inline-actions">
            <button
              className="button secondary"
              disabled={busy || saving || !desktop}
              onClick={() =>
                guard(async () => {
                  if (await api.export()) setNotice(ui.diagnosticsSaved);
                })
              }
            >
              <Download size={17} />
              {ui.exportDiagnostics}
            </button>
            <button
              className="button secondary"
              disabled={!desktop}
              onClick={() =>
                guard(async () => {
                  await api.storage();
                })
              }
            >
              <ExternalLink size={17} />
              {ui.storageSettings}
            </button>
          </div>
        </section>
        <div className="about" hidden={settingsLeaf !== 'tools'}>
          <Wind size={24} />
          <div>
            <strong>拂尘 {boot.version}</strong>
            <p>{ui.about}</p>
            <small>{ui.previewBuild}</small>
            <ProductCapabilities boot={boot} />
          </div>
        </div>
      </div>
    </section>
  );
}
