// SkillPage.tsx — 「skill」页（TASK-51；fix-51-r1 改为 magpie 式卡片；TASK-53 加斜杠命令
// 卡片）：工具行左边是主 agent 的实际厂商与改厂商下拉，右边是安装按钮；当前主 agent 会读取的 ai-cross skill
// 位置与状态；安装到全局 / 项目、更新、卸载我们装的那份。数据取 skill_status
// （选过项目目录后带上它），资源是否探测过取 config_snapshot(host) 的 resources。
// 安装、更新、卸载同一时间只跑一个。

import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import {
  commands,
  isTauri,
  type HostEntry,
  type ResourcesOutput,
  type SkillLocation,
  type SkillScope,
  type SkillStatus,
} from '../tauri';
import { needsFirstDetect } from './config-view';
import {
  canUninstall,
  duplicatePaths,
  findHostStatus,
  globalInstallAction,
  hostLocations,
  locationStatus,
  outdatedText,
  scopeText,
  skillInstalled,
  skillOverview,
} from './skill-view';
import { SlashCard, type ModeChoice } from './SlashCard';
import { ConfirmDialog } from '../components/common/ConfirmDialog';
import { HostBar } from '../components/common/HostBar';
import { Icon } from '../components/common/Icon';
import { PageHeader } from '../components/common/PageHeader';
import { StatCards } from '../components/common/StatCards';
import './ConfigPages.css';

type Busy = { kind: 'install' | 'update'; scope: SkillScope } | { kind: 'uninstall'; path: string } | null;

/** active：导航切到「skill」时为 true；每次切回重新读状态（安装进行中不读）。
 *  host 为空表示还没有可作主 agent 的 agent。 */
export function SkillPage({
  active,
  host,
  hostsLoaded,
  refreshKey,
  onRefreshDone,
  onGoDetect,
  onVendorChanged,
}: {
  active: boolean;
  host: HostEntry | null;
  /** 主 agent 列表已读到；读到之前不显示「尚未探测资源」。 */
  hostsLoaded: boolean;
  /** 顶栏「刷新」的计数，变化时重新读取。 */
  refreshKey: number;
  /** 刷新触发的读取结束后回报（key 为本次 refreshKey），顶栏据此停止转圈。 */
  onRefreshDone: (key: number, ok: boolean) => void;
  onGoDetect: () => void;
  onVendorChanged: () => void;
}) {
  const hostId = host?.id ?? '';
  const [status, setStatus] = useState<SkillStatus | null>(null);
  const [resources, setResources] = useState<ResourcesOutput | null>(null);
  // 当前主 agent 的模式（斜杠命令的模式下拉用）。
  const [modes, setModes] = useState<ModeChoice[]>([]);
  const [project, setProject] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusyState] = useState<Busy>(null);
  const busyRef = useRef<Busy>(null);
  const setBusy = (next: Busy) => {
    busyRef.current = next;
    setBusyState(next);
  };
  const [confirm, setConfirm] = useState<SkillLocation | null>(null);
  const closeConfirm = useCallback(() => setConfirm(null), []);

  const load = useCallback(async (projectDir: string) => {
    const next = await commands.skillStatus(projectDir === '' ? undefined : projectDir);
    setStatus(next);
  }, []);

  useEffect(() => {
    if (!active) return;
    // 不在窗口里、没有主 agent，或安装进行中（结束时自己会重读）：刷新直接算完成。
    if (!isTauri || hostId === '' || busyRef.current !== null) {
      if (refreshKey > 0) onRefreshDone(refreshKey, true);
      return;
    }
    let cancelled = false;
    const statusLoad = commands.skillStatus(project === '' ? undefined : project)
      .then((next) => {
        if (!cancelled) { setStatus(next); setError(''); }
        return true;
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
        return false;
      });
    const snapshotLoad = commands.configSnapshot(hostId)
      .then((snapshot) => {
        if (cancelled) return true;
        setResources(snapshot.resources);
        setModes(snapshot.config.modes.map((mode) => ({ id: mode.id, name: mode.name })));
        return true;
      })
      .catch(() => false /* 资源读不到只是不显示未探测提示 */);
    void Promise.all([statusLoad, snapshotLoad]).then(([a, b]) => {
      if (refreshKey > 0) onRefreshDone(refreshKey, a && b);
    });
    return () => { cancelled = true; };
  }, [active, hostId, project, refreshKey, onRefreshDone]);

  const header = (actions?: ReactNode) => (
    <PageHeader left={host && <HostBar host={host} onVendorChanged={onVendorChanged} />} actions={actions} />
  );

  if (!isTauri) {
    return (
      <div className="page">
        {header()}
        <div className="page-body"><p className="cfg-empty">仅在 aicross 窗口中可用</p></div>
      </div>
    );
  }

  if (host === null) {
    return (
      <div className="page">
        {header()}
        <div className="page-body">
          {!hostsLoaded ? <p className="cfg-empty">读取中…</p> : (
            <div className="cfg-banner">
              <Icon name="alert" />
              <span className="cfg-banner-text">尚未探测资源</span>
              <button type="button" className="btn primary" onClick={onGoDetect}>去探测</button>
            </div>
          )}
        </div>
      </div>
    );
  }

  const bundled = status?.bundledVersion ?? '';
  const hostStatus = findHostStatus(status, hostId);
  const action = globalInstallAction(hostStatus, bundled);
  const duplicates = duplicatePaths(hostStatus);

  const run = async (next: NonNullable<Busy>, task: () => Promise<unknown>, projectDir: string) => {
    setBusy(next);
    setError('');
    try {
      await task();
    } catch (err) {
      setError(String(err));
    }
    try {
      await load(projectDir);
    } catch (err) {
      setError(String(err));
    }
    setBusy(null);
  };

  const installGlobal = () => {
    const kind = action.kind === 'update' ? 'update' : 'install';
    void run({ kind, scope: 'global' }, () => commands.skillInstall(hostId, 'global'), project);
  };

  const installProject = async () => {
    setError('');
    let dir: string | null = null;
    try {
      dir = await commands.pickFolder();
    } catch (err) {
      setError(String(err));
      return;
    }
    if (!dir) return;
    setProject(dir);
    void run({ kind: 'install', scope: 'project' }, () => commands.skillInstall(hostId, 'project', dir), dir);
  };

  const uninstall = (location: SkillLocation) => {
    setConfirm(null);
    const projectDir = location.scope === 'project' ? project : undefined;
    void run(
      { kind: 'uninstall', path: location.path },
      () => commands.skillUninstall(hostId, location.scope, projectDir),
      project,
    );
  };

  const globalBusy = busy?.kind !== 'uninstall' && busy?.scope === 'global';
  const projectBusy = busy?.kind === 'install' && busy.scope === 'project';

  return (
    <div className="page">
      {header(
        <>
          <button type="button" className="btn" disabled={busy !== null} onClick={() => void installProject()}>
            <Icon name="folder" />
            {projectBusy ? '安装中…' : '安装到项目'}
          </button>
          {action.disabled && status !== null ? (
            <span className={`pill skill-global-state ${action.kind === 'installed' ? 'ok' : ''}`}>{action.label}</span>
          ) : (
            <button
              type="button"
              className="btn primary"
              disabled={busy !== null || action.disabled || status === null}
              onClick={installGlobal}
            >
              <Icon name="download" />
              {globalBusy ? (busy?.kind === 'update' ? '更新中…' : '安装中…') : action.label}
            </button>
          )}
        </>,
      )}
      <div className="page-body">
        {needsFirstDetect(resources) && (
          <div className="cfg-banner">
            <Icon name="alert" />
            <span className="cfg-banner-text">尚未探测资源</span>
            <button type="button" className="btn primary" onClick={onGoDetect}>去探测</button>
          </div>
        )}
        {duplicates.length > 0 && (
          <div className="cfg-banner alert">
            <Icon name="alert" />
            <div className="cfg-banner-text">
              <div>检测到重复安装</div>
              {duplicates.map((path) => <div key={path} className="cfg-mono cfg-banner-path">{path}</div>)}
            </div>
          </div>
        )}
        {error !== '' && <p className="cfg-error cfg-head-error">{error}</p>}
        {status === null && error === '' && <p className="cfg-empty">读取中…</p>}
        {status !== null && <StatCards items={skillOverview(status, hostStatus)} />}
        {status !== null && (
          <section className="card">
            {hostLocations(hostStatus).map((location) => {
              const state = locationStatus(location, bundled);
              const removing = busy?.kind === 'uninstall' && busy.path === location.path;
              return (
                <div key={`${location.scope}:${location.path}`} className="card-row skill-row">
                  <span className="pill skill-scope">{scopeText(location.scope)}</span>
                  <div className="skill-row-main">
                    <span className="cfg-mono skill-path" title={location.path}>{location.path}</span>
                    {location.kind === 'external' && location.linkTarget && (
                      <span className="card-sub cfg-mono" title={location.linkTarget}>指向 {location.linkTarget}</span>
                    )}
                  </div>
                  {outdatedText(location, bundled) !== '' && (
                    <span className="pill warn">{outdatedText(location, bundled)}</span>
                  )}
                  <span className={`pill skill-state ${state.tone}`}>{state.text}</span>
                  <span className="skill-row-action">
                    {canUninstall(location) && (
                      <button
                        type="button"
                        className="icon-btn"
                        title="卸载"
                        aria-label="卸载"
                        disabled={busy !== null}
                        onClick={() => setConfirm(location)}
                      >
                        {removing ? '卸载中…' : <Icon name="trash" />}
                      </button>
                    )}
                  </span>
                </div>
              );
            })}
          </section>
        )}
        {status !== null && (
          <SlashCard
            // 换主 agent 时整张卡片重建，不残留上一个 agent 的列表与表单。
            key={host.id}
            host={host}
            modes={modes}
            skillInstalled={skillInstalled(hostStatus)}
            active={active}
            refreshKey={refreshKey}
          />
        )}
      </div>
      {confirm && (
        <ConfirmDialog
          title="卸载 ai-cross skill"
          confirmLabel="卸载"
          onConfirm={() => uninstall(confirm)}
          onCancel={closeConfirm}
        >
          <span className="cfg-mono">{confirm.path}</span>
        </ConfirmDialog>
      )}
    </div>
  );
}
