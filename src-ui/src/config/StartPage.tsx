// StartPage.tsx — 「开始」页（TASK-63）：新人一页走完三步。① 探测本机的 AI；② 选主 agent、
// 一键配好（装 skill + 一键推荐，不动手选人选），显示审与干活各派给谁；③ 回到主 agent 里
// 输入什么。数据取 config_snapshot(host)、skill_status、slash_list；写入只有 skill_install
// 与 config_recommend，同一时间只跑一个。

import { useCallback, useEffect, useRef, useState } from 'react';
import { commands, isTauri, type ConfigSnapshot, type DetectProgress, type HostEntry, type SkillStatus } from '../tauri';
import { needsFirstDetect } from './config-view';
import { findHostStatus, managerText, outdatedText, recommendNotes } from './skill-view';
import {
  foundAgents,
  invokeText,
  needsSkillInstall,
  reviewMode,
  seatLines,
  setupState,
  usageExamples,
  workMode,
} from './start-view';
import { CopyButton } from './SlashCard';
import { Icon } from '../components/common/Icon';
import type { ModeEntry } from '../tauri';
import './ConfigPages.css';

type Busy = 'detect' | 'setup' | null;

export function StartPage({
  active,
  host,
  hosts,
  onSelectHost,
  refreshKey,
  onRefreshDone,
  onDetected,
  onGoModes,
  onGoResources,
}: {
  active: boolean;
  host: HostEntry | null;
  hosts: HostEntry[] | null;
  onSelectHost: (id: string) => void;
  refreshKey: number;
  onRefreshDone: (key: number, ok: boolean) => void;
  /** 探测完成后通知外壳重读主 agent 列表。 */
  onDetected: () => Promise<boolean> | void;
  onGoModes: () => void;
  onGoResources: () => void;
}) {
  const hostId = host?.id ?? '';
  const [snapshot, setSnapshot] = useState<ConfigSnapshot | null>(null);
  const [skill, setSkill] = useState<SkillStatus | null>(null);
  const [invoke, setInvoke] = useState('');
  const [error, setError] = useState('');
  const [notes, setNotes] = useState<string[]>([]);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [busy, setBusyState] = useState<Busy>(null);
  const busyRef = useRef<Busy>(null);
  const setBusy = (next: Busy) => {
    busyRef.current = next;
    setBusyState(next);
  };

  /** 读三份数据；任一失败只记错误，已有数据保留。返回是否全部读到。 */
  const load = useCallback(async (): Promise<boolean> => {
    const results = await Promise.allSettled([
      commands.configSnapshot(hostId === '' ? null : hostId).then(setSnapshot),
      commands.skillStatus().then(setSkill),
      hostId === '' ? Promise.resolve() : commands.slashList(hostId).then((list) => setInvoke(list.invoke)),
    ]);
    const failed = results.find((result): result is PromiseRejectedResult => result.status === 'rejected');
    setError(failed ? String(failed.reason) : '');
    return failed === undefined;
  }, [hostId]);

  useEffect(() => {
    if (!active) return;
    if (!isTauri || busyRef.current !== null) {
      if (refreshKey > 0) onRefreshDone(refreshKey, true);
      return;
    }
    let cancelled = false;
    void load().then((ok) => {
      if (!cancelled && refreshKey > 0) onRefreshDone(refreshKey, ok);
    });
    return () => { cancelled = true; };
  }, [active, load, refreshKey, onRefreshDone]);

  if (!isTauri) {
    return (
      <div className="page">
        <div className="page-body"><p className="cfg-empty">仅在 aicross 窗口中可用</p></div>
      </div>
    );
  }

  const resources = snapshot?.resources ?? null;
  const detected = resources !== null && !needsFirstDetect(resources);
  const found = foundAgents(resources);
  const bundled = skill?.bundledVersion ?? '';
  const skillHost = findHostStatus(skill, hostId);
  const modes = snapshot?.config.modes ?? [];
  const defaultMode = snapshot?.config.defaultMode ?? '';
  const state = setupState({
    detected,
    host: hostId,
    skillHost,
    skillLoaded: skill !== null,
    bundledVersion: bundled,
    modes,
    defaultMode,
  });
  const external = skillHost?.locations.find((location) => location.kind === 'external' && location.readByThisHost !== false) ?? null;
  const command = invokeText(hostId, invoke);

  const detect = async () => {
    if (busyRef.current !== null) return;
    setBusy('detect');
    setError('');
    setProgress(null);
    let unlisten: (() => void) | undefined;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten = await listen<DetectProgress>('resources-detect-progress', (event) => {
        setProgress({ done: event.payload.done, total: event.payload.total });
      });
    } catch { /* 事件不可用 */ }
    let failure = '';
    try {
      await commands.resourcesDetect();
    } catch (err) {
      failure = String(err);
    }
    unlisten?.();
    await onDetected();
    await load();
    if (failure !== '') setError(failure);
    setProgress(null);
    setBusy(null);
  };

  const setup = async () => {
    if (busyRef.current !== null || hostId === '') return;
    setBusy('setup');
    setError('');
    setNotes([]);
    let failure = '';
    try {
      if (needsSkillInstall(skillHost, bundled)) await commands.skillInstall(hostId, 'global');
      const result = await commands.configRecommend(hostId, null, false);
      setNotes(recommendNotes(result.notes));
    } catch (err) {
      failure = String(err);
    }
    await load();
    if (failure !== '') setError(failure);
    setBusy(null);
  };

  const detectLabel = busy === 'detect'
    ? progress && progress.total > 0 ? `探测中 ${progress.done}/${progress.total}` : '探测中…'
    : detected ? '重新探测' : '探测';

  return (
    <div className="page">
      <div className="page-body start-body">
        {error !== '' && <p className="cfg-error cfg-head-error">{error}</p>}

        <section className="card start-step">
          <div className="card-row card-head">
            <span className={`start-num${detected ? ' done' : ''}`}>{detected ? <Icon name="check" /> : '1'}</span>
            <div className="start-head-text">
              <div className="card-title">本机的 AI</div>
              {detected && (
                <div className="card-sub">
                  {found.names.length === 0 ? '没有找到' : `${found.names.join('、')} · 可用模型 ${found.models} 个`}
                </div>
              )}
            </div>
            {detected && <button type="button" className="icon-btn" onClick={onGoResources}>详情</button>}
            <button
              type="button"
              className={`btn${detected ? '' : ' primary'}`}
              disabled={busy !== null}
              onClick={() => void detect()}
            >
              <Icon name={detected ? 'refresh' : 'search'} />
              {detectLabel}
            </button>
          </div>
        </section>

        <section className="card start-step">
          <div className="card-row card-head">
            <span className={`start-num${state === 'done' ? ' done' : ''}`}>{state === 'done' ? <Icon name="check" /> : '2'}</span>
            <div className="start-head-text">
              <div className="card-title">配好</div>
              {state === 'need-detect' && <div className="card-sub">探测后可配</div>}
              {state === 'need-host' && <div className="card-sub">没有找到可用的主 agent</div>}
            </div>
            {state === 'done' && <button type="button" className="btn" onClick={onGoModes}>改人选</button>}
            {(state === 'todo' || state === 'done') && (
              <button
                type="button"
                className={`btn${state === 'done' ? '' : ' primary'}`}
                disabled={busy !== null || skill === null}
                onClick={() => void setup()}
              >
                <Icon name="sparkles" />
                {busy === 'setup' ? '配置中…' : state === 'done' ? '重新推荐' : '一键配好'}
              </button>
            )}
          </div>
          {detected && hosts !== null && hosts.length > 0 && (
            <div className="card-row start-host-row">
              <span className="start-label">在哪个软件里用</span>
              <div className="segmented start-hosts">
                {hosts.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    className={`segmented-item${item.id === hostId ? ' selected' : ''}`}
                    aria-pressed={item.id === hostId}
                    disabled={busy !== null}
                    onClick={() => onSelectHost(item.id)}
                  >
                    {item.name}
                  </button>
                ))}
              </div>
            </div>
          )}
          {detected && hostId !== '' && skill !== null && (
            <div className="card-row start-skill-row">
              <span className="start-label">ai-cross skill</span>
              <SkillState skillHost={skillHost} external={external} bundled={bundled} />
            </div>
          )}
          {detected && hostId !== '' && snapshot !== null && (
            <>
              <ModeLine label="审" mode={reviewMode(modes, defaultMode)} />
              <ModeLine label="干活" mode={workMode(modes, defaultMode)} />
            </>
          )}
          {notes.length > 0 && (
            <div className="card-row start-notes">
              {notes.map((note) => <div key={note} className="card-sub start-note">{note}</div>)}
            </div>
          )}
        </section>

        <section className="card start-step">
          <div className="card-row card-head">
            <span className="start-num">3</span>
            <div className="start-head-text">
              <div className="card-title">怎么用</div>
              <div className="card-sub">
                {host ? `回到 ${host.name}，输入：` : '回到你常用的 AI 软件，输入：'}
              </div>
            </div>
          </div>
          {usageExamples(command).map((example) => (
            <div key={example.label} className="card-row start-example">
              <span className="pill muted start-example-label">{example.label}</span>
              <code className="cfg-mono start-example-text">{example.text}</code>
              <CopyButton text={example.text} icon />
            </div>
          ))}
        </section>
      </div>
    </div>
  );
}

function SkillState({
  skillHost,
  external,
  bundled,
}: {
  skillHost: ReturnType<typeof findHostStatus>;
  external: NonNullable<ReturnType<typeof findHostStatus>>['locations'][number] | null;
  bundled: string;
}) {
  if (external) {
    const old = outdatedText(external, bundled);
    return (
      <>
        <span className="pill">{managerText(external.manager)}{external.version ? ` · ${external.version}` : ''}</span>
        {old !== '' && <span className="pill warn">{old}</span>}
      </>
    );
  }
  const ours = skillHost?.locations.find((location) => location.kind === 'ours' && location.readByThisHost !== false);
  if (!ours) return <span className="pill muted">未安装</span>;
  if (bundled !== '' && ours.version.trim() !== bundled) return <span className="pill warn">可更新 · {ours.version}</span>;
  return <span className="pill ok">已安装{ours.version ? ` · ${ours.version}` : ''}</span>;
}

function ModeLine({ label, mode }: { label: string; mode: ModeEntry | null }) {
  const lines = seatLines(mode);
  return (
    <div className="card-row start-mode-row">
      <span className="start-label">{label}</span>
      <div className="start-mode-main">
        <span className="start-mode-name">{mode?.name ?? '未找到模式'}</span>
        <div className="start-seats">
          {lines.map((line, index) => (
            <span
              key={`${line.role}-${index}`}
              className={`start-seat${line.alert ? ' alert' : ''}`}
              title={line.title}
            >
              <span className="start-seat-role">{line.role}</span>
              {line.model === '' ? '未选' : line.model}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}
