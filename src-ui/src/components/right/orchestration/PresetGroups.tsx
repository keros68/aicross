import { useEffect, useRef } from 'react';
import type { Preset } from '../../../graph/types.ts';
import { presetDisplayName, presetGroupOf } from '../../../graph/label.ts';
import './PresetGroups.css';

const BASIC_ORDER = ['builtin-shared', 'builtin-economy', 'builtin-blind'];
const MORE_ORDER = ['builtin-arena', 'builtin-jury', 'builtin-swarm'];

const FUTURE_PRESETS = [
  { id: 'redblue', name: '红蓝模式（规划中）' },
  { id: 'chat', name: '群聊模式（规划中）' },
];

function ordered(presets: Preset[], ids: string[]): Preset[] {
  return [...presets].sort((a, b) => {
    const ai = ids.indexOf(a.id);
    const bi = ids.indexOf(b.id);
    if (ai < 0 && bi < 0) return 0;
    if (ai < 0) return 1;
    if (bi < 0) return -1;
    return ai - bi;
  });
}

interface PresetGroupsProps {
  presets: Preset[];
  onSelect: (preset: Preset) => void;
  selectedId?: string;
  includeFuture?: boolean;
}

export function PresetGroups({ presets, onSelect, selectedId, includeFuture = false }: PresetGroupsProps) {
  const userPresetCount = presets.filter((preset) => !preset.builtin).length;
  const basic = ordered(presets.filter((preset) => presetGroupOf(preset) === '基础模式'), BASIC_ORDER);
  const custom = presets.filter((preset) => presetGroupOf(preset) === '自定义模式');
  const more = ordered(presets.filter((preset) => presetGroupOf(preset) === '更多模式'), MORE_ORDER);

  const items = (group: Preset[]) => group.map((preset) => (
    <button
      key={preset.id}
      type="button"
      className="preset-group-item"
      role="option"
      aria-selected={preset.id === selectedId}
      onClick={() => onSelect(preset)}
    >
      {presetDisplayName(preset, userPresetCount)}
    </button>
  ));

  if (presets.length === 0) {
    return <div className="preset-groups"><div className="preset-group-item" aria-disabled="true">无可用预设</div></div>;
  }
  return (
    <div className="preset-groups" role="listbox">
      <div className="preset-group-label">基础模式</div>
      {items(basic)}
      {custom.length > 0 && (
        <>
          <div className="preset-group-label">自定义模式</div>
          {items(custom)}
        </>
      )}
      <details className="preset-more">
        <summary className="preset-group-label">更多模式</summary>
        {items(more)}
        {includeFuture && FUTURE_PRESETS.map((preset) => (
          <button key={preset.id} type="button" className="preset-group-item" disabled>
            {preset.name}
          </button>
        ))}
      </details>
    </div>
  );
}

// 右栏下拉的关闭判定（R1 复审 1）：Esc 或点到 .preset-select 之外就收起，
// 与图上盖预设菜单的 stampMenuShouldClose 同一口径。纯函数，可测。
export function presetSelectShouldClose(event: KeyboardEvent | MouseEvent): boolean {
  if (event.type === 'keydown') return (event as KeyboardEvent).key === 'Escape';
  const target = event.target as { closest?: (selector: string) => unknown } | null;
  return !target?.closest?.('.preset-select');
}

interface PresetSelectProps {
  presets: Preset[];
  selectedId: string;
  onSelect: (preset: Preset) => void;
}

export function PresetSelect({ presets, selectedId, onSelect }: PresetSelectProps) {
  const rootRef = useRef<HTMLDetailsElement>(null);
  const selected = presets.find((preset) => preset.id === selectedId);
  const userPresetCount = presets.filter((preset) => !preset.builtin).length;
  // 收起时把「更多模式」也折回去（R1 复审 2）：下次打开仍是默认折叠。
  const close = () => {
    const root = rootRef.current;
    if (!root) return;
    root.removeAttribute('open');
    root.querySelector('details.preset-more')?.removeAttribute('open');
  };
  useEffect(() => {
    const onEvent = (event: KeyboardEvent | MouseEvent) => {
      if (rootRef.current?.open && presetSelectShouldClose(event)) close();
    };
    document.addEventListener('mousedown', onEvent);
    document.addEventListener('keydown', onEvent);
    return () => {
      document.removeEventListener('mousedown', onEvent);
      document.removeEventListener('keydown', onEvent);
    };
  }, []);

  return (
    <details className="preset-select" ref={rootRef}>
      <summary className="orch-preset-select" aria-haspopup="listbox">
        {selected ? presetDisplayName(selected, userPresetCount) : '选择预设'}
      </summary>
      <div className="preset-select-menu">
        <PresetGroups
          presets={presets}
          selectedId={selectedId}
          includeFuture
          onSelect={(preset) => {
            onSelect(preset);
            close();
          }}
        />
      </div>
    </details>
  );
}
