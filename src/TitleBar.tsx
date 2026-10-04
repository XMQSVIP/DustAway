import { useEffect, useState, type ReactNode } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Copy, Minus, Square, X } from 'lucide-react';
import { desktop } from './api';
import { text, windowText } from './i18n';

export function TitleBar({
  closing,
  onError,
  children,
  actions,
}: {
  closing: boolean;
  onError: (message: string) => void;
  children?: ReactNode;
  actions?: ReactNode;
}) {
  const [maximized, setMaximized] = useState(false);
  const [pending, setPending] = useState(false);
  useEffect(() => {
    if (!desktop) return;
    const window = getCurrentWindow();
    let disposed = false;
    let serial = 0;
    const update = async () => {
      const request = ++serial;
      try {
        const value = await window.isMaximized();
        if (!disposed && request === serial) setMaximized(value);
      } catch (error) {
        if (!disposed) onError(windowText.failed + String(error));
      }
    };
    // Listen before the initial read, so snap/keyboard changes also update the restore button.
    const listener = window.onResized(() => void update());
    void listener
      .then(() => update())
      .catch((error) => {
        if (!disposed) onError(windowText.failed + String(error));
      });
    return () => {
      disposed = true;
      void listener.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [onError]);

  async function control(action: 'minimize' | 'toggleMaximize' | 'close') {
    if (!desktop || pending || closing) return;
    setPending(true);
    try {
      // close() requests a normal OS close; Rust retains cancellation and journal handling.
      await getCurrentWindow()[action]();
    } catch (error) {
      onError(windowText.failed + String(error));
    } finally {
      setPending(false);
    }
  }
  const maximizeLabel = maximized ? windowText.restore : windowText.maximize;
  return (
    <header className="window-titlebar" aria-label={windowText.titlebar}>
      <div className="window-brand" data-tauri-drag-region title={windowText.drag}>
        <span className="window-brand-icon" aria-hidden="true">
          <svg viewBox="0 0 40 40" fill="none">
            <path
              d="M20 2c3 0 4 8 4 12 3-3 10-7 12-5s-2 9-5 12c4 0 10 2 9 5s-9 3-13 2c2 4 3 10 0 11s-7-5-8-9c-2 4-7 9-10 7s0-9 3-12C7 26 0 24 1 21s8-5 12-5C9 12 6 5 9 4s8 5 9 9c-1-4-1-11 2-11Z"
              fill="currentColor"
            />
          </svg>
        </span>
        <span className="brand-wordmark">
          <strong>{text.app}</strong>
          <small>DUSTAWAY</small>
        </span>
      </div>
      <div className="window-drag-area" data-tauri-drag-region title={windowText.drag} />
      {children}
      <div className="window-appearance">{actions}</div>
      <div className="window-controls" role="group" aria-label={windowText.controls}>
        <button
          type="button"
          aria-label={windowText.minimize}
          title={windowText.minimize}
          disabled={!desktop || pending || closing}
          onClick={() => void control('minimize')}
        >
          <Minus size={15} aria-hidden="true" />
        </button>
        <button
          type="button"
          aria-label={maximizeLabel}
          title={maximizeLabel}
          disabled={!desktop || pending || closing}
          onClick={() => void control('toggleMaximize')}
        >
          {maximized ? (
            <Copy size={14} aria-hidden="true" />
          ) : (
            <Square size={13} aria-hidden="true" />
          )}
        </button>
        <button
          type="button"
          className="window-close"
          aria-label={closing ? windowText.closing : windowText.close}
          title={closing ? windowText.closing : windowText.close}
          disabled={!desktop || pending || closing}
          onClick={() => void control('close')}
        >
          <X size={17} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}
