// ModelPicker.tsx — 模型选择下拉（fix-51-r1，参照 magpie 的 picker）：触发器是灰色
// 圆角块（agent 图标 + agent/model），或只有图标的按钮（备选用）；弹出层顶部搜索框，
// 下面按 agent / provider 分组的列表，每项带图标、模型名与状态，禁用项灰掉。
// 键盘上下选、回车确定、Esc 与点外部关闭。

import { useCallback, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import type { SeatOption } from './config-view';
import { filterOptions, firstEnabled, flattenGroups, groupOptions, moveActive, pickerPlacement, PICKER_LIST_MAX } from './picker';
import { optionStateText } from './newuser-view';
import { AgentIcon } from '../components/common/AgentIcon';
import { Icon, type IconName } from '../components/common/Icon';
import { useDismiss } from '../components/common/use-dismiss';

export interface PickerExtra {
  key: string;
  label: string;
}

export function ModelPicker({
  value,
  options,
  onSelect,
  extra,
  disabled,
  alert,
  empty,
  badge,
  placeholder,
  icon,
  label,
}: {
  /** 当前选中项的 key；空串为未选。 */
  value: string;
  options: SeatOption[];
  onSelect: (key: string) => void;
  /** 列表最上方的额外一项（如「清除人选」「不设备选」），选中时回调传它的 key。 */
  extra?: PickerExtra | null;
  disabled?: boolean;
  /** 触发器用异常色（人选失效）。 */
  alert?: boolean;
  /** 还没选：中性虚线框 + 灰字（TASK-58）。 */
  empty?: boolean;
  /** 触发器里模型名后的小标记（如「推荐」「未发现」）。 */
  badge?: ReactNode;
  placeholder: string;
  /** 给了就用图标按钮做触发器（备选）。 */
  icon?: IconName;
  /** 图标按钮的悬停说明与无障碍名。 */
  label?: string;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(-1);
  const [placement, setPlacement] = useState({ up: false, listMax: PICKER_LIST_MAX });
  const rootRef = useRef<HTMLDivElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  const close = useCallback(() => setOpen(false), []);
  useDismiss(open, rootRef, close);

  const groups = useMemo(() => groupOptions(filterOptions(options, query)), [options, query]);
  // 键盘移动的顺序：额外项在最前，其后按分组展示顺序。
  const items = useMemo(() => {
    const flat = flattenGroups(groups).map((option) => ({ key: option.key, disabled: option.disabled }));
    return extra && query.trim() === '' ? [{ key: extra.key, disabled: false }, ...flat] : flat;
  }, [groups, extra, query]);
  const current = options.find((option) => option.key === value);

  const toggle = () => {
    if (disabled) return;
    if (open) {
      setOpen(false);
      return;
    }
    setQuery('');
    const rect = rootRef.current?.getBoundingClientRect();
    if (rect) setPlacement(pickerPlacement(rect.top, rect.bottom, window.innerHeight));
    setOpen(true);
    const selected = items.findIndex((item) => item.key === value);
    setActive(selected >= 0 ? selected : firstEnabled(items));
  };

  const choose = (key: string) => {
    setOpen(false);
    if (key !== value) onSelect(key);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const next = moveActive(items, active, event.key === 'ArrowDown' ? 1 : -1);
      setActive(next);
      listRef.current?.querySelector(`[data-index="${next}"]`)?.scrollIntoView({ block: 'nearest' });
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const item = items[active];
      if (item && !item.disabled) choose(item.key);
    }
  };

  const indexOf = (key: string) => items.findIndex((item) => item.key === key);

  return (
    <div className="picker" ref={rootRef}>
      {icon ? (
        <button
          type="button"
          className={`icon-btn picker-icon-trigger${value !== '' ? ' on' : ''}`}
          title={label}
          aria-label={label}
          aria-haspopup="listbox"
          aria-expanded={open}
          disabled={disabled}
          onClick={toggle}
        >
          {current ? <AgentIcon agent={current.agent} size={16} /> : <Icon name={icon} />}
        </button>
      ) : (
        <button
          type="button"
          className={`picker-trigger${alert ? ' alert' : ''}${empty && !alert ? ' empty' : ''}`}
          aria-haspopup="listbox"
          aria-expanded={open}
          disabled={disabled}
          onClick={toggle}
        >
          {current && <AgentIcon agent={current.agent} size={16} />}
          <span className="picker-trigger-text">
            {current ? `${current.agent}/${current.model}` : placeholder}
          </span>
          {badge}
          <Icon name="chevron-down" size={14} />
        </button>
      )}
      {open && (
        <div className={`picker-pop${placement.up ? ' up' : ''}`}>
          <div className="picker-search">
            <Icon name="search" />
            <input
              autoFocus
              value={query}
              placeholder="搜索模型"
              onChange={(event) => {
                setQuery(event.target.value);
                setActive(-1);
              }}
              onKeyDown={onKeyDown}
            />
          </div>
          <div className="picker-list" ref={listRef} role="listbox" style={{ maxHeight: placement.listMax }}>
            {extra && query.trim() === '' && (
              <button
                type="button"
                role="option"
                aria-selected={false}
                data-index={indexOf(extra.key)}
                className={`picker-item${active === indexOf(extra.key) ? ' active' : ''}`}
                onMouseEnter={() => setActive(indexOf(extra.key))}
                onClick={() => choose(extra.key)}
              >
                <span className="picker-item-name">{extra.label}</span>
              </button>
            )}
            {groups.map((group) => (
              <div key={group.key} className="picker-group">
                <div className="picker-group-title">
                  {group.label}
                  {group.untested && <span className="pill muted">未实测</span>}
                </div>
                {group.items.map((option) => {
                  const index = indexOf(option.key);
                  return (
                    <button
                      key={option.key}
                      type="button"
                      role="option"
                      aria-selected={option.key === value}
                      aria-disabled={option.disabled}
                      data-index={index}
                      disabled={option.disabled}
                      className={`picker-item${index === active ? ' active' : ''}`}
                      onMouseEnter={() => { if (!option.disabled) setActive(index); }}
                      onClick={() => choose(option.key)}
                    >
                      <AgentIcon agent={option.agent} size={16} />
                      <span className="picker-item-name">{option.model}</span>
                      <span className="picker-item-state">{optionStateText(option.state)}</span>
                      {option.key === value && <Icon name="check" size={14} />}
                    </button>
                  );
                })}
              </div>
            ))}
            {groups.length === 0 && <div className="picker-empty">无匹配模型</div>}
          </div>
        </div>
      )}
    </div>
  );
}
