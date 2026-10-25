// App.tsx — 窗口外壳（TASK-49、TASK-51；fix-51-r1 改为 magpie 式顶栏）：顶栏左边是
// 应用名与「主 agent」标签 + 下拉（所有页都显示，只有 skill、模式页随它变化），正中是分段导航（开始 / 资源 / skill /
// 模式 / 派发记录），右边是刷新（重新读取当前页）与窗口按钮；顶栏空白处可拖动窗口。
// 下面是内容区，查看页也在内容区里，导航保留。`aicross open <run_id>` 的冷启动请求
// 随 argv 走（takePendingOpen 取一次），热启动走单实例插件转发的 'open-run-request'
// 事件（带递增序号，同一 run 重开也要刷新）；查看页「返回」或点导航都会关掉查看页，
// 「返回」回到「派发记录」。

import { useCallback, useEffect, useRef, useState } from 'react';
import { commands, isTauri, type HostEntry } from './tauri';
import { HostMenu } from './components/common/HostMenu';
import { Icon } from './components/common/Icon';
import { ResizeEdges } from './components/common/ResizeEdges';
import { WindowControls } from './components/common/TitleBar';
import { Home } from './viewer/Home';
import { RunViewer } from './viewer/RunViewer';
import { ModesPage } from './config/ModesPage';
import { ResourcesPage } from './config/ResourcesPage';
import { SkillPage } from './config/SkillPage';
import { StartPage } from './config/StartPage';
import { PAGE_TABS, keepsMounted, pageTabStorage, readPageTab, writePageTab, type PageTab } from './config/page-tabs';
import { onWindowDragDoubleClick, onWindowDragMouseDown } from './lib/window-drag';
import { readHost, writeHost } from './lib/host-select';
import './config/ConfigPages.css';

export function App() {
  // 查看页请求：runId + 递增序号（fix-37-r1 R1-9，同一个 run 再次 open 也
  // 要刷新）。null = 显示导航选中的页。
  const [viewerRequest, setViewerRequest] = useState<{ runId: string; seq: number } | null>(null);
  const [tab, setTab] = useState<PageTab>(() => readPageTab(pageTabStorage()));
  // 顶栏「刷新」：每点一次加一，各页把它放进读取的依赖里重新读取。
  const [refreshKey, setRefreshKey] = useState(0);

  // 主 agent：hosts_list 的结果（null 为还没读到）与当前选中的 id。
  const [hosts, setHosts] = useState<HostEntry[] | null>(null);
  const [hostId, setHostId] = useState('');

  // 模式页「去测试」：跳到资源页并展开该 agent 卡片（seq 每次递增，同一 agent 再点也生效）。
  const [focusAgent, setFocusAgent] = useState<{ agent: string; seq: number } | null>(null);

  const selectTab = (next: PageTab) => {
    setTab(next);
    writePageTab(pageTabStorage(), next);
  };

  const selectHost = (id: string) => {
    setHostId(id);
    writeHost(pageTabStorage(), id);
  };

  /** 读主 agent 列表；当前选中的不在列表里时换成记住的或第一个。探测完、改厂商、刷新后重读。
   *  返回是否读成功（刷新按钮据此显示结果）。 */
  const loadHosts = useCallback(async (): Promise<boolean> => {
    if (!isTauri) return true;
    try {
      const list = await commands.hostsList();
      setHosts(list);
      setHostId((current) => (list.some((host) => host.id === current) ? current : readHost(pageTabStorage(), list)));
      return true;
    } catch {
      setHosts((current) => current ?? []);
      return false;
    }
  }, []);

  // 顶栏刷新：点了以后图标旋转，直到主 agent 列表与当前页都重读完（最短 400ms），
  // 然后短暂显示「已刷新」或「刷新失败」；进行中再点不重复发起。当前页读完时经
  // onRefreshDone(key, ok) 回报；页面没有回报（如正忙、不在窗口里）时 10 秒后按完成算。
  const [refreshing, setRefreshing] = useState(false);
  const [refreshNote, setRefreshNote] = useState<'' | 'ok' | 'fail'>('');
  const pageWait = useRef<{ key: number; resolve: (ok: boolean) => void } | null>(null);
  const noteTimer = useRef<number | undefined>(undefined);
  const onRefreshDone = useCallback((key: number, ok: boolean) => {
    const wait = pageWait.current;
    if (wait && wait.key === key) {
      pageWait.current = null;
      wait.resolve(ok);
    }
  }, []);

  useEffect(() => { loadHosts(); }, [loadHosts]);

  const currentHost = hosts?.find((host) => host.id === hostId) ?? null;

  useEffect(() => {
    if (!isTauri) return;
    commands.takePendingOpen()
      .then((runId) => { if (runId) setViewerRequest({ runId, seq: 0 }); })
      .catch(() => {});
    // R1-10：卸载可能早于异步注册完成——disposed 标志在注销完成时补拉销。
    let disposed = false;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        const fn = await listen<{ runId: string; seq: number }>('open-run-request', (event) => {
          setViewerRequest({ runId: event.payload.runId, seq: event.payload.seq });
        });
        if (disposed) fn();
        else unlisten = fn;
      } catch { /* event bridge unavailable (e.g. browser dev) — ignore */ }
    })();
    return () => { disposed = true; unlisten?.(); };
  }, []);

  // 查看页打开时导航高亮「派发记录」：查看页从那里进、关掉也回那里。
  const navSelected: PageTab = viewerRequest === null ? tab : 'runs';

  const refresh = async () => {
    if (refreshing) return;
    setRefreshing(true);
    setRefreshNote('');
    window.clearTimeout(noteTimer.current);
    const key = refreshKey + 1;
    const page = new Promise<boolean>((resolve) => {
      pageWait.current = { key, resolve };
      window.setTimeout(() => {
        if (pageWait.current?.key === key) {
          pageWait.current = null;
          resolve(true);
        }
      }, 10000);
    });
    setRefreshKey(key);
    const [hostsOk, pageOk] = await Promise.all([
      loadHosts(),
      page,
      new Promise((resolve) => window.setTimeout(resolve, 400)),
    ]);
    setRefreshing(false);
    setRefreshNote(hostsOk && pageOk ? 'ok' : 'fail');
    noteTimer.current = window.setTimeout(() => setRefreshNote(''), 1500);
  };

  return (
    <>
      <header className="topbar" onMouseDown={onWindowDragMouseDown} onDoubleClick={onWindowDragDoubleClick}>
        <div className="topbar-left">
          <span className="topbar-brand">
            <img className="topbar-logo" src="/favicon.png" alt="aicross" width={22} height={22} />
            <span className="topbar-name">aicross</span>
          </span>
          <HostMenu hosts={hosts} current={currentHost} onSelect={selectHost} />
        </div>
        <nav className="segmented topbar-nav" aria-label="页面">
          {PAGE_TABS.map((item) => (
            <button
              key={item.id}
              type="button"
              className={`segmented-item${item.id === navSelected ? ' selected' : ''}`}
              aria-current={item.id === navSelected ? 'page' : undefined}
              onClick={() => {
                setViewerRequest(null);
                selectTab(item.id);
              }}
            >
              {item.label}
            </button>
          ))}
        </nav>
        <div className="topbar-right">
          {refreshNote !== '' && (
            <span className={`topbar-refresh-note${refreshNote === 'fail' ? ' alert' : ''}`} role="status">
              {refreshNote === 'ok' ? '已刷新' : '刷新失败'}
            </span>
          )}
          <button
            type="button"
            className={`icon-btn topbar-refresh${refreshing ? ' spinning' : ''}`}
            title="刷新"
            aria-label="刷新"
            aria-busy={refreshing}
            onClick={() => void refresh()}
          >
            <Icon name="refresh" />
          </button>
          <WindowControls />
        </div>
      </header>
      <main className="app-content">
        {/* 常驻页（keepsMounted）挂载后只随导航与查看页隐藏，打开查看页也不卸载；
            其余页只在选中且没开查看页时挂载。 */}
        {PAGE_TABS.map(({ id }) => {
          const visible = viewerRequest === null && id === tab;
          return (keepsMounted(id) || visible) && (
            <div key={id} hidden={!visible}>
              {id === 'start' && (
                <StartPage
                  active={visible}
                  host={currentHost}
                  hosts={hosts}
                  onSelectHost={selectHost}
                  refreshKey={refreshKey}
                  onRefreshDone={onRefreshDone}
                  onDetected={loadHosts}
                  onGoModes={() => selectTab('modes')}
                  onGoResources={() => selectTab('resources')}
                />
              )}
              {id === 'resources' && (
                <ResourcesPage
                  active={visible}
                  host={hostId}
                  refreshKey={refreshKey}
                  onRefreshDone={onRefreshDone}
                  onDetected={loadHosts}
                  onGoModes={() => selectTab('modes')}
                  focusAgent={focusAgent}
                />
              )}
              {id === 'skill' && (
                <SkillPage
                  key={hostId}
                  active={visible}
                  host={currentHost}
                  hostsLoaded={hosts !== null}
                  refreshKey={refreshKey}
                  onRefreshDone={onRefreshDone}
                  onGoDetect={() => selectTab('resources')}
                  onVendorChanged={loadHosts}
                />
              )}
              {id === 'modes' && (
                <ModesPage
                  key={hostId}
                  active={visible}
                  host={currentHost}
                  hostsLoaded={hosts !== null}
                  refreshKey={refreshKey}
                  onRefreshDone={onRefreshDone}
                  onGoDetect={() => selectTab('resources')}
                  onGoSkill={() => selectTab('skill')}
                  onGoTest={(agent) => {
                    setFocusAgent((current) => ({ agent, seq: (current?.seq ?? 0) + 1 }));
                    selectTab('resources');
                  }}
                  onVendorChanged={loadHosts}
                />
              )}
              {id === 'runs' && (
                <Home
                  active={visible}
                  refreshKey={refreshKey}
                  onRefreshDone={onRefreshDone}
                  onOpenRun={(runId) => setViewerRequest({ runId, seq: 0 })}
                />
              )}
            </div>
          );
        })}
        {viewerRequest !== null && (
          <RunViewer
            key={viewerRequest.runId}
            runId={viewerRequest.runId}
            refreshSeq={viewerRequest.seq}
            refreshKey={refreshKey}
            onRefreshDone={onRefreshDone}
            onClose={() => {
              setViewerRequest(null);
              selectTab('runs');
            }}
          />
        )}
      </main>
      <ResizeEdges />
    </>
  );
}
