// StartPage.tsx — 「开始」页（TASK-63）：新人一页走完。① 主 agent：探测、选在哪个软件里
// 对话、skill 状态；② 副 agent：被派去干活、审查的各家与其模型；③ 一键配好（装 skill +
// 一键推荐，不动手选人选），显示审与干活各派给谁；④ 回到主 agent 里输入什么。一次探测
// 同时得出主 agent 与副 agent（TASK-68）。数据取 config_snapshot(host)、skill_status、slash_list；写入只有 skill_install
// 与 config_recommend，同一时间只跑一个。

import { useCallback, useEffect, useRef, useState } from 'react';
import { commands, isTauri, type ConfigSnapshot, type DetectProgress, type HostEntry, type SkillStatus } from '../tauri';
import { needsFirstDetect } from './config-view';
import { canUpgradeHere, findHostStatus, managerText, outdatedText, recommendNotes, staleExternal } from './skill-view';
import {
  invokeText,
  needsSkillInstall,
  reviewMode,
  seatLines,
  setupHint,
  setupState,
  usageExamples,
  workMode,
  workerAgents,
  workerText,
} from './start-view';
import { CopyButton } from './SlashCard';
import { Icon } from '../components/common/Icon';
import { ConfirmDialog } from '../components/common/ConfirmDialog';
import type { ModeEntry } from '../tauri';
import './ConfigPages.css';

type Busy = 'detect' | 'setup' | 'upgrade' | null;

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
  const [upgradeNote, setUpgradeNote] = useState('');
  /** 替换别人装的旧版前先确认；'setup' 表示确认后接着一键推荐。 */
  const [confirm, setConfirm] = useState<'upgrade' | 'setup' | null>(null);
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
  const workers = workerAgents(resources, hostId);
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
  const stale = staleExternal(skillHost);
  const upgradable = stale !== null && canUpgradeHere(stale);
  const command = invokeText(hostId, invoke);
  const review = reviewMode(modes, defaultMode);
  const work = workMode(modes, defaultMode);
  const hint = setupHint([review, work], notes);

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

  /** 升级 skill：别人装的旧版换成内置版（原文件备份），我们装的旧版就地更新。 */
  const upgradeSkill = async (): Promise<void> => {
    if (stale !== null) {
      const result = await commands.skillUpgrade(hostId, stale.path) as { notes?: string[] };
      setUpgradeNote((result.notes ?? []).join('；'));
    } else {
      await commands.skillInstall(hostId, 'global');
    }
  };

  const runUpgrade = async () => {
    if (busyRef.current !== null || hostId === '') return;
    setBusy('upgrade');
    setError('');
    let failure = '';
    try {
      await upgradeSkill();
    } catch (err) {
      failure = String(err);
    }
    await load();
    if (failure !== '') setError(failure);
    setBusy(null);
  };

  const runSetup = async () => {
    if (busyRef.current !== null || hostId === '') return;
    setBusy('setup');
    setError('');
    setNotes([]);
    let failure = '';
    try {
      if (upgradable) await upgradeSkill();
      else if (needsSkillInstall(skillHost, bundled)) await commands.skillInstall(hostId, 'global');
      const result = await commands.configRecommend(hostId, null, false);
      setNotes(recommendNotes(result.notes));
    } catch (err) {
      failure = String(err);
    }
    await load();
    if (failure !== '') setError(failure);
    setBusy(null);
  };

  const setup = () => {
    if (upgradable) setConfirm('setup');
    else void runSetup();
  };

  const upgrade = () => {
    if (upgradable) setConfirm('upgrade');
    else void runUpgrade();
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
            <span className={`start-num${detected && hostId !== '' ? ' done' : ''}`}>
              {detected && hostId !== '' ? <Icon name="check" /> : '1'}
            </span>
            <div className="start-head-text">
              <div className="card-title">主 agent</div>
              <div className="card-sub">你和 AI 对话的软件，ai-cross skill 装在这里</div>
            </div>
            <button
              type="button"
              className={`btn${detected ? '' : ' primary'}`}
              disabled={busy !== null}
              title="一次探测同时找出主 agent 与副 agent"
              onClick={() => void detect()}
            >
              <Icon name={detected ? 'refresh' : 'search'} />
              {detectLabel}
            </button>
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
          {detected && (hosts === null || hosts.length === 0) && (
            <div className="card-row"><span className="card-sub">没有找到可用的主 agent</span></div>
          )}
          {detected && hostId !== '' && skill !== null && (
            <div className="card-row start-skill-row">
              <span className="start-label">ai-cross skill</span>
              <SkillState
                skillHost={skillHost}
                external={external}
                bundled={bundled}
                busy={busy !== null}
                upgrading={busy === 'upgrade'}
                onUpgrade={upgrade}
              />
            </div>
          )}
          {upgradeNote !== '' && (
            <div className="card-row start-notes">
              <div className="card-sub start-note">{upgradeNote}</div>
            </div>
          )}
        </section>

        <section className="card start-step">
          <div className="card-row card-head">
            <span className={`start-num${workers.length > 0 ? ' done' : ''}`}>
              {workers.length > 0 ? <Icon name="check" /> : '2'}
            </span>
            <div className="start-head-text">
              <div className="card-title">副 agent</div>
              <div className="card-sub">
                {detected ? '被派去干活、审查的；同一家的模型可能经几个 agent 都能用' : '被派去干活、审查的；探测后列出'}
              </div>
            </div>
            {detected && <button type="button" className="btn" onClick={onGoResources}>详情</button>}
          </div>
          {workers.map((worker) => (
            <div key={worker.id} className="card-row start-worker-row">
              <span className="start-label">{worker.name}</span>
              <span className="card-sub">{workerText(worker)}</span>
              {worker.isHost && <span className="pill muted">当前主 agent</span>}
            </div>
          ))}
          {detected && workers.length === 0 && (
            <div className="card-row"><span className="card-sub">没有找到能派发的 agent</span></div>
          )}
        </section>

        <section className="card start-step">
          <div className="card-row card-head">
            <span className={`start-num${state === 'done' ? ' done' : ''}`}>{state === 'done' ? <Icon name="check" /> : '3'}</span>
            <div className="start-head-text">
              <div className="card-title">配好</div>
              {state === 'need-detect' && <div className="card-sub">探测后可配</div>}
              {state === 'need-host' && <div className="card-sub">先选主 agent</div>}
            </div>
            {state === 'done' && <button type="button" className="btn" onClick={onGoModes}>改人选</button>}
            {(state === 'todo' || state === 'done') && (
              <button
                type="button"
                className={`btn${state === 'done' ? '' : ' primary'}`}
                disabled={busy !== null || skill === null}
                onClick={setup}
              >
                <Icon name="sparkles" />
                {busy === 'setup' ? '配置中…' : state === 'done' ? '重新推荐' : '一键配好'}
              </button>
            )}
          </div>
          {detected && hostId !== '' && snapshot !== null && (
            <>
              <ModeLine label="审" mode={review} />
              <ModeLine label="干活" mode={work} />
            </>
          )}
          {detected && hostId !== '' && hint.missing > 0 && (
            <div className="card-row start-notes">
              <div className="start-note">
                还有 {hint.missing} 个位置没选到人。
                {hint.reasons.length > 0 ? `${hint.reasons.join('；')}。` : '点「一键配好」按推荐补上，或到模式页手选。'}
                {hint.unverified > 0 && `资源页还有 ${hint.unverified} 个模型没验证，验证通过后再点「重新推荐」。`}
              </div>
              <div className="start-note-actions">
                {hint.unverified > 0 && <button type="button" className="btn" onClick={onGoResources}>去验证</button>}
                <button type="button" className="btn" onClick={onGoModes}>手选人选</button>
              </div>
            </div>
          )}
        </section>

        {confirm !== null && stale !== null && (
          <ConfirmDialog
            title="升级 ai-cross skill"
            confirmLabel="升级"
            onCancel={() => setConfirm(null)}
            onConfirm={() => {
              const next = confirm;
              setConfirm(null);
              void (next === 'setup' ? runSetup() : runUpgrade());
            }}
          >
            {host?.name ?? hostId} 里的 ai-cross skill 是 {stale.version || '旧版'}，由其他软件安装，读不到 aicross
            的设置。升级会换成 {bundled}，原来的文件备份到同一配置目录下的 .aicross-skill-backup，之后随 aicross 更新。
          </ConfirmDialog>
        )}

        <section className="card start-step">
          <div className="card-row card-head">
            <span className="start-num">4</span>
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
  busy,
  upgrading,
  onUpgrade,
}: {
  skillHost: ReturnType<typeof findHostStatus>;
  external: NonNullable<ReturnType<typeof findHostStatus>>['locations'][number] | null;
  bundled: string;
  busy: boolean;
  upgrading: boolean;
  onUpgrade: () => void;
}) {
  const upgradeButton = (
    <button type="button" className="btn primary" disabled={busy} onClick={onUpgrade}>
      {upgrading ? '升级中…' : '升级'}
    </button>
  );
  if (external) {
    const old = outdatedText(external, bundled);
    return (
      <>
        <span className="pill">{managerText(external.manager)}{external.version ? ` · ${external.version}` : ''}</span>
        {old !== '' && <span className="pill warn">{old}</span>}
        {old !== '' && (canUpgradeHere(external) ? upgradeButton : <span className="card-sub">在 skills-manager 里更新</span>)}
      </>
    );
  }
  const ours = skillHost?.locations.find((location) => location.kind === 'ours' && location.readByThisHost !== false);
  if (!ours) return <span className="pill muted">未安装</span>;
  if (bundled !== '' && ours.version.trim() !== bundled) {
    return (
      <>
        <span className="pill warn">需要升级 · {ours.version} → {bundled}</span>
        {upgradeButton}
      </>
    );
  }
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
