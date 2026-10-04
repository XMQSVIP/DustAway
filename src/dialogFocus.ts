import { useRef, type RefObject } from 'react';

/** Controlled dialogs have no Radix Trigger; retain their actual opening control. */
export function useDialogFocus(trigger?: RefObject<HTMLElement | null>) {
  const origin = useRef<HTMLElement | null>(null);
  return {
    onOpenAutoFocus() {
      origin.current =
        trigger?.current ??
        (document.activeElement instanceof HTMLElement ? document.activeElement : null);
    },
    onCloseAutoFocus(event: Event) {
      event.preventDefault();
      queueMicrotask(() => {
        if (
          document.querySelector(
            '[role="dialog"][data-state="open"], [role="alertdialog"][data-state="open"]',
          )
        )
          return;
        const target = origin.current;
        if (target?.isConnected && !target.closest('[hidden]') && !target.matches(':disabled'))
          target.focus({ preventScroll: true });
        else document.getElementById('main')?.focus();
      });
    },
  };
}
