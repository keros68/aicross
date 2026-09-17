import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import type { Preset } from '../../graph/types';
import { presetDisplayName, presetGroupOf } from '../../graph/label';
import {
  acceptConsoleProposal,
  clearProposalNotice,
  dismissConsoleProposal,
  getConsoleProposalNotices,
  getConsoleProposals,
  proposalNoticeForTab,
  proposalForTab,
  proposalIsForeign,
  proposalPresets,
  proposalsForTab,
  subscribeConsoleProposals,
  updateProposalPreset,
} from '../../console-channel';
import { hasOpenModal, type DialogRoot } from '../../lib/focus-enforcer';
import './ProposalBar.css';

const BASIC_ORDER = ['builtin-shared', 'builtin-economy', 'builtin-blind'];
const MORE_ORDER = ['builtin-arena', 'builtin-jury', 'builtin-swarm'];

function orderByIds(values: Preset[], ids: string[]): Preset[] {
  return [...values].sort((a, b) => {
    const ai = ids.indexOf(a.id);
    const bi = ids.indexOf(b.id);
    if (ai < 0 && bi < 0) return 0;
    if (ai < 0) return 1;
    if (bi < 0) return -1;
    return ai - bi;
  });
}

export function orderProposalPresets(presets: Preset[]): Preset[] {
  const basic = orderByIds(presets.filter((preset) => presetGroupOf(preset) === '基础模式'), BASIC_ORDER);
  const custom = presets.filter((preset) => presetGroupOf(preset) === '自定义模式');
  const more = orderByIds(presets.filter((preset) => presetGroupOf(preset) === '更多模式'), MORE_ORDER);
  return [...basic, ...custom, ...more];
}

interface ShortcutEvent {
  key: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  isComposing?: boolean;
  targetMultiline?: boolean;
  targetInput?: boolean;
  modalOpen?: boolean;
}

export function proposalShortcutAction(
  event: ShortcutEvent,
  state: { commander: boolean; ptyFocused: boolean },
): 'accept' | 'dismiss' | 'cycle' | null {
  if (event.isComposing || event.modalOpen || event.altKey || event.metaKey || event.shiftKey) return null;
  if (state.commander && state.ptyFocused) return event.key === 'Enter' && event.ctrlKey ? 'accept' : null;
  if (event.targetInput || event.targetMultiline) return null;
  if (event.key === 'Escape') return 'dismiss';
  if (event.key === 'Tab' && !event.ctrlKey) return 'cycle';
  if (event.key !== 'Enter') return null;
  return event.ctrlKey ? null : 'accept';
}

function proposalKeyText(ctrlEnter: boolean, inputFocused: boolean): string {
  if (ctrlEnter) return 'Ctrl+Enter 采纳 · 或点右侧按钮';
  return inputFocused ? '点右侧按钮' : '回车采纳 · Esc 忽略 · Tab 换模式';
}

export function proposalBarText(modeName: string, nodeCount: number, reason: string, ctrlEnter: boolean, inputFocused = false): string {
  const keys = proposalKeyText(ctrlEnter, inputFocused);
  const base = `建议派发 · ${modeName} · ${nodeCount} 路 · ${keys}`;
  return reason ? `${base} · ${reason}` : base;
}

export function LocalProposalButton({
  text,
  attachments = [],
  className,
  onPropose,
}: {
  text: string;
  attachments?: string[];
  className: string;
  onPropose: (text: string, attachments: string[]) => void;
}) {
  const value = text.trim();
  return (
    <button type="button" className={className} disabled={!value} onClick={() => onPropose(value, attachments)}>
      建议派发
    </button>
  );
}

interface ShortcutTarget {
  tagName?: string;
  isContentEditable?: boolean;
  classList?: { contains(value: string): boolean };
  closest?(selector: string): unknown;
}

export function proposalTargetFlags(target: EventTarget | ShortcutTarget | null): { targetInput: boolean; ptyFocused: boolean } {
  const element = target as ShortcutTarget | null;
  const ptyFocused = Boolean(element?.classList?.contains('xterm-helper-textarea') || element?.closest?.('.xterm-helper-textarea'));
  const contentEditable = typeof element?.isContentEditable === 'boolean'
    ? element.isContentEditable
    : Boolean(element?.closest?.('[contenteditable="true"],[contenteditable=""],[contenteditable="plaintext-only"]'));
  const targetInput = element?.tagName === 'INPUT'
    || element?.tagName === 'TEXTAREA'
    || contentEditable;
  return { targetInput, ptyFocused };
}

export function proposalShortcutFromTarget(
  event: Omit<ShortcutEvent, 'targetInput' | 'targetMultiline' | 'modalOpen'>,
  state: { commander: boolean },
  target: EventTarget | ShortcutTarget | null,
  root: DialogRoot,
): 'accept' | 'dismiss' | 'cycle' | null {
  const flags = proposalTargetFlags(target);
  return proposalShortcutAction({ ...event, targetInput: flags.targetInput, modalOpen: hasOpenModal(root) }, { ...state, ptyFocused: flags.ptyFocused });
}

export function ProposalBar({ project, graphId = '', commander = false }: { project: string; graphId?: string; commander?: boolean }) {
  const proposals = useSyncExternalStore(subscribeConsoleProposals, getConsoleProposals, getConsoleProposals);
  const notices = useSyncExternalStore(subscribeConsoleProposals, getConsoleProposalNotices, getConsoleProposalNotices);
  const matching = proposalsForTab(proposals, project, graphId, commander);
  const proposal = proposalForTab(proposals, project, graphId, commander);
  const notice = proposalNoticeForTab(notices, project, graphId, commander);
  const barRef = useRef<HTMLDivElement>(null);
  const [focusFlags, setFocusFlags] = useState(() => proposalTargetFlags(document.activeElement));
  const [busy, setBusy] = useState(false);
  const presets = useMemo(() => proposal ? orderProposalPresets(proposalPresets(proposal.proposal_id)) : [], [proposal]);
  const selected = proposal ? presets.find((preset) => preset.id === proposal.preset_id) : undefined;
  const userPresetCount = presets.filter((preset) => !preset.builtin).length;
  const modeName = selected ? presetDisplayName(selected, userPresetCount) : '经济模式';
  const ctrlEnter = commander && focusFlags.ptyFocused;
  const inputFocused = focusFlags.targetInput && !ctrlEnter;

  useEffect(() => {
    const update = () => setFocusFlags(proposalTargetFlags(document.activeElement));
    const updateAfterBlur = () => queueMicrotask(update);
    document.addEventListener('focusin', update, true);
    document.addEventListener('focusout', updateAfterBlur, true);
    return () => {
      document.removeEventListener('focusin', update, true);
      document.removeEventListener('focusout', updateAfterBlur, true);
    };
  }, []);

  useEffect(() => {
    if (!proposal) return;
    const cycle = () => {
      const index = presets.findIndex((preset) => preset.id === proposal.preset_id);
      const next = presets[(index + 1 + presets.length) % presets.length];
      if (next) updateProposalPreset(proposal.proposal_id, next.id);
    };
    const onKey = (event: KeyboardEvent) => {
      if (barRef.current?.offsetParent === null) return;
      const action = proposalShortcutFromTarget({
        key: event.key, ctrlKey: event.ctrlKey, altKey: event.altKey, metaKey: event.metaKey, shiftKey: event.shiftKey,
        isComposing: event.isComposing,
      }, { commander }, event.target, document);
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      if (busy) return;
      if (action === 'cycle') {
        cycle();
        return;
      }
      setBusy(true);
      const operation = action === 'accept' ? acceptConsoleProposal(proposal.proposal_id) : dismissConsoleProposal(proposal.proposal_id);
      void operation.then(() => setBusy(false), () => setBusy(false));
    };
    document.addEventListener('keydown', onKey, true);
    return () => document.removeEventListener('keydown', onKey, true);
  }, [busy, commander, presets, proposal]);

  if (!proposal) return notice ? (
    <div ref={barRef} className="proposal-bar proposal-bar-notice" role="status">
      <span>{notice.text}</span>
      <button type="button" className="proposal-bar-action" onClick={() => clearProposalNotice(notice.id)}>关闭</button>
    </div>
  ) : null;
  const cycle = () => {
    const index = presets.findIndex((preset) => preset.id === proposal.preset_id);
    const next = presets[(index + 1 + presets.length) % presets.length];
    if (next) updateProposalPreset(proposal.proposal_id, next.id);
  };
  const runAction = (action: 'accept' | 'dismiss') => {
    if (busy) return;
    setBusy(true);
    const operation = action === 'accept' ? acceptConsoleProposal(proposal.proposal_id) : dismissConsoleProposal(proposal.proposal_id);
    void operation.then(() => setBusy(false), () => setBusy(false));
  };
  const keys = proposalKeyText(ctrlEnter, inputFocused);
  const remaining = matching.length > 1 ? ` · 还有 ${matching.length - 1} 条` : '';
  // 这条建议属于别的项目、只是兜底画在这里时，把目标目录带出来（09-16 复审 GLM 4）
  const foreign = proposalIsForeign(proposal, project, graphId) ? ` · 目标 ${proposal.project}` : '';
  return (
    <div ref={barRef} className="proposal-bar" role="status" aria-label={proposalBarText(modeName, proposal.node_count, proposal.reason, ctrlEnter, inputFocused)}>
      <span>建议派发 · </span>
      <button type="button" className="proposal-bar-mode" disabled={busy} onClick={cycle}>{modeName}</button>
      <span> · {proposal.node_count} 路 · {keys}{remaining}{foreign}</span>
      {proposal.reason && <span className="proposal-bar-reason"> · {proposal.reason}</span>}
      <span className="proposal-bar-actions">
        <button type="button" className="proposal-bar-action" disabled={busy} onClick={() => runAction('accept')}>采纳</button>
        <button type="button" className="proposal-bar-action" disabled={busy} onClick={() => runAction('dismiss')}>忽略</button>
      </span>
    </div>
  );
}
