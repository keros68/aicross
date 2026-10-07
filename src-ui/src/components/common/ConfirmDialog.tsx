// ConfirmDialog.tsx — 应用内确认框（TASK-51）：覆盖会改动或删除东西的操作
// （全部按推荐重填、卸载 skill）。不用浏览器 confirm：它会阻塞 WebView。
// 打开时焦点落在「取消」，Esc 等同取消。

import { useEffect, useRef, type ReactNode } from 'react';

export function ConfirmDialog({
  title,
  children,
  confirmLabel,
  onConfirm,
  onCancel,
}: {
  title: string;
  children?: ReactNode;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const cancelRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    cancelRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.isComposing) {
        event.preventDefault();
        event.stopPropagation();
        onCancel();
      }
    };
    document.addEventListener('keydown', onKey, true);
    return () => document.removeEventListener('keydown', onKey, true);
  }, [onCancel]);

  return (
    <div className="dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onCancel(); }}>
      <div className="dialog" role="alertdialog" aria-modal="true" aria-label={title}>
        <div className="dialog-title">{title}</div>
        {children && <div className="dialog-body">{children}</div>}
        <div className="dialog-actions">
          <button type="button" className="btn" ref={cancelRef} onClick={onCancel}>取消</button>
          <button type="button" className="btn primary" onClick={onConfirm}>{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}
