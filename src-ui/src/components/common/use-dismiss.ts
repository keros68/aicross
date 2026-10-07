// 弹出层的关闭：打开时点外部或按 Esc 关闭（fix-51-r1，模型选择、主 agent 下拉、
// 更多菜单共用）。

import { useEffect, type RefObject } from 'react';

export function useDismiss(open: boolean, ref: RefObject<HTMLElement | null>, onClose: () => void): void {
  useEffect(() => {
    if (!open) return;
    const onDown = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) onClose();
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.isComposing) {
        event.preventDefault();
        event.stopPropagation();
        onClose();
      }
    };
    document.addEventListener('mousedown', onDown, true);
    document.addEventListener('keydown', onKey, true);
    return () => {
      document.removeEventListener('mousedown', onDown, true);
      document.removeEventListener('keydown', onKey, true);
    };
  }, [open, ref, onClose]);
}
