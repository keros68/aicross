// ModelPicker.tsx — 模型选择下拉（fix-51-r1，参照 magpie 的 picker）：触发器是灰色
// 圆角块（agent 图标 + agent/model），或只有图标的按钮（备选用）；弹出层顶部搜索框，
// 下面按 agent / provider 分组的列表，每项带图标、模型名与状态，禁用项灰掉。
// 键盘上下选、回车确定、Esc、点外部、外部滚动与窗口缩放关闭。

import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import type { SeatOption } from './config-view';
import {
  closesOnViewportChange,
  filterOptions,
  pickerAccepts,
  pickerShown,
  groupOptions,
  moveActive,
  openActive,
  pickerItems,
  pickerPlacement,
  visibleBounds,
  PICKER_LIST_MAX,
  type VerticalBounds,
} from './picker';
import { optionStateText } from './newuser-view';
import { AgentIcon } from '../components/common/AgentIcon';
import { Icon, type IconName } from '../components/common/Icon';
import { useDismiss } from '../components/common/use-dismiss';

/** 会裁切弹出层的祖先（overflow 不是 visible，如滚动的页面正文）的上下边界。 */
function clipAncestors(element: HTMLElement): VerticalBounds[] {
  const clips: VerticalBounds[] = [];
  for (let node = element.parentElement; node; node = node.parentElement) {
    const style = window.getComputedStyle(node);
    if (style.overflowY !== 'visible' || style.overflowX !== 'visible') {
      const rect = node.getBoundingClientRect();
      clips.push({ top: rect.top, bottom: rect.bottom });
    }
  }
  return clips;
}

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
  // 被禁用（本行或其它位置保存中）时已展开的弹层也收起：渲染期间直接改状态，重新启用后不会自己弹出。
  if (open && !pickerShown(open, disabled === true)) setOpen(false);
  const shown = pickerShown(open, disabled === true);
  const close = useCallback(() => setOpen(false), []);
  useDismiss(shown, rootRef, close);

  // 展开期间外部滚动或窗口缩放会让按打开时算好的位置失效，直接关闭。
  // scroll 不冒泡，在捕获阶段监听以收到页面正文等祖先的滚动。
  useEffect(() => {
    if (!shown) return;
    const onChange = (event: Event) => {
      const target = event.target;
      const inside = target instanceof Node && rootRef.current?.contains(target) === true;
      if (closesOnViewportChange(event.type === 'resize' ? 'resize' : 'scroll', inside)) setOpen(false);
    };
    window.addEventListener('scroll', onChange, true);
    window.addEventListener('resize', onChange);
    return () => {
      window.removeEventListener('scroll', onChange, true);
      window.removeEventListener('resize', onChange);
    };
  }, [shown]);

  const groups = useMemo(() => groupOptions(filterOptions(options, query)), [options, query]);
  const items = useMemo(() => pickerItems(options, query, extra), [options, query, extra]);
  const current = options.find((option) => option.key === value);

  const toggle = () => {
    if (disabled) return;
    if (open) {
      setOpen(false);
      return;
    }
    setQuery('');
    const root = rootRef.current;
    if (root) {
      const rect = root.getBoundingClientRect();
      setPlacement(pickerPlacement(rect.top, rect.bottom, visibleBounds(clipAncestors(root), window.innerHeight)));
    }
    setOpen(true);
    setActive(openActive(options, extra, value));
  };

  const choose = (key: string, itemDisabled = false) => {
    if (!pickerAccepts(disabled === true, itemDisabled)) return;
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
      if (item) choose(item.key, item.disabled);
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
          aria-expanded={shown}
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
          aria-expanded={shown}
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
      {shown && (
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
                disabled={disabled}
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
                      aria-disabled={option.disabled || disabled === true}
                      data-index={index}
                      disabled={option.disabled || disabled === true}
                      className={`picker-item${index === active ? ' active' : ''}`}
                      onMouseEnter={() => { if (!option.disabled) setActive(index); }}
                      onClick={() => choose(option.key, option.disabled)}
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
