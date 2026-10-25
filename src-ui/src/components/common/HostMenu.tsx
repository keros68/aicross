// HostMenu.tsx — 顶栏的主 agent 下拉（fix-51-r1；fix-51-r3 改为所有页都显示）：小标签
// 「主 agent」+ 灰色圆角块（agent 图标 + 名称）；点开是列表，每项图标 + 名称 + 右侧
// 灰色厂商（认不出时不写），当前项打勾。资源与派发记录页也可切换，只是内容不随它变。

import { useCallback, useRef, useState } from 'react';
import type { HostEntry } from '../../tauri';
import { vendorText } from '../../lib/host-select';
import { AgentIcon } from './AgentIcon';
import { Icon } from './Icon';
import { useDismiss } from './use-dismiss';

export function HostMenu({
  hosts,
  current,
  onSelect,
}: {
  hosts: HostEntry[] | null;
  current: HostEntry | null;
  onSelect: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const close = useCallback(() => setOpen(false), []);
  useDismiss(open, rootRef, close);

  return (
    <div className="host-menu" ref={rootRef}>
      <span className="host-menu-label">主 agent</span>
      {hosts !== null && hosts.length === 0 && <span className="host-menu-empty">尚未探测资源</span>}
      {current && (
        <button
          type="button"
          className="host-menu-trigger"
          aria-haspopup="listbox"
          aria-expanded={open}
          aria-label="主 agent"
          onClick={() => setOpen((value) => !value)}
        >
          <AgentIcon agent={current.id} size={18} />
          <span className="host-menu-name">{current.name || current.id}</span>
          <Icon name="chevron-down" size={14} />
        </button>
      )}
      {open && current && (
        <div className="host-menu-pop" role="listbox" aria-label="主 agent">
          {(hosts ?? []).map((host) => {
            const vendor = vendorText(host);
            const selected = host.id === current.id;
            return (
              <button
                key={host.id}
                type="button"
                role="option"
                aria-selected={selected}
                className={`host-menu-item${selected ? ' selected' : ''}`}
                onClick={() => {
                  setOpen(false);
                  onSelect(host.id);
                }}
              >
                <AgentIcon agent={host.id} size={18} />
                <span className="host-menu-name">{host.name || host.id}</span>
                {!vendor.alert && <span className="host-menu-vendor">{vendor.text}</span>}
                <span className="host-menu-check">{selected && <Icon name="check" size={14} />}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
