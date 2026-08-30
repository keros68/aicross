import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { Changes } from "./components/Changes";
import { Sessions } from "./components/Sessions";
import { SettingsDialog } from "./components/SettingsDialog";
import { TerminalView } from "./components/TerminalView";
import { Workspace } from "./components/Workspace";
import type { EnvEntry, InitialState, Settings, TerminalTab, Tool } from "./types";

type LeftTab = "workspace" | "resources" | "sessions";
type RightTab = "changes" | "plan" | "tasks";

const leftLabels: Record<LeftTab, string> = {
  workspace: "工作区",
  resources: "资源",
  sessions: "记录",
};
const rightLabels: Record<RightTab, string> = {
  changes: "修改记录",
  plan: "方案与编排",
  tasks: "任务列表",
};

export default function App() {
  const [initial, setInitial] = useState<InitialState | null>(null);
  const [tools, setTools] = useState<Tool[]>([]);
  const [settingsDraft, setSettingsDraft] = useState<Settings | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [leftTab, setLeftTab] = useState<LeftTab>("workspace");
  const [rightTab, setRightTab] = useState<RightTab>("changes");
  const [leftCollapsed, setLeftCollapsed] = useState(false);
  const [rightCollapsed, setRightCollapsed] = useState(false);
  const [terminalTabs, setTerminalTabs] = useState<TerminalTab[]>([]);
  const [centerTab, setCenterTab] = useState("launchpad");
  const [toolEnv, setToolEnv] = useState<Record<string, EnvEntry[]>>({});
  const [expandedEnv, setExpandedEnv] = useState<string | null>(null);
  const [loadingTools, setLoadingTools] = useState(true);
  const [error, setError] = useState("");
  const counters = useRef<Record<string, number>>({});

  const refreshTools = useCallback(() => {
    setLoadingTools(true);
    api.tools().then(setTools).catch((value) => setError(String(value))).finally(() => setLoadingTools(false));
  }, []);

  useEffect(() => {
    api.initialState()
      .then((value) => {
        setInitial(value);
        setSettingsDraft(value.settings);
      })
      .catch((value) => setError(String(value)));
    refreshTools();
  }, [refreshTools]);

  const settings = initial?.settings;
  const leftTabs = useMemo(
    () => (settings ? (Object.keys(leftLabels) as LeftTab[]).filter((key) => settings.tabs[key]) : []),
    [settings],
  );
  const rightTabs = useMemo(
    () => (settings ? (Object.keys(rightLabels) as RightTab[]).filter((key) => settings.tabs[key]) : []),
    [settings],
  );

  useEffect(() => {
    if (!leftTabs.includes(leftTab) && leftTabs[0]) setLeftTab(leftTabs[0]);
    if (!rightTabs.includes(rightTab) && rightTabs[0]) setRightTab(rightTabs[0]);
  }, [leftTab, leftTabs, rightTab, rightTabs]);

  const changeProject = useCallback(async (path: string) => {
    try {
      const value = await api.setProject(path);
      setInitial(value);
      setSettingsDraft(value.settings);
    } catch (value) {
      setError(String(value));
    }
  }, []);

  const chooseProject = async () => {
    const selected = await open({ directory: true, multiple: false, defaultPath: initial?.projectPath });
    if (typeof selected === "string") await changeProject(selected);
  };

  const saveSettings = async () => {
    if (!settingsDraft || !initial) return;
    try {
      const saved = await api.saveSettings(settingsDraft);
      setInitial({ ...initial, settings: saved });
      setSettingsOpen(false);
    } catch (value) {
      setError(String(value));
    }
  };

  const updateEnv = (toolId: string, rows: EnvEntry[]) => {
    setToolEnv((value) => ({ ...value, [toolId]: rows }));
  };

  const launch = (tool: Tool) => {
    const rows = toolEnv[tool.id] ?? [];
    const env: Record<string, string> = {};
    for (const row of rows) {
      const key = row.key.trim();
      if (!key) continue;
      if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
        setError(`环境变量名无效：${key}`);
        return;
      }
      if (Object.hasOwn(env, key)) {
        setError(`环境变量重复：${key}`);
        return;
      }
      env[key] = row.value;
    }
    const number = (counters.current[tool.id] ?? 0) + 1;
    counters.current[tool.id] = number;
    const id = crypto.randomUUID();
    const tab: TerminalTab = {
      id,
      toolId: tool.id,
      title: `${tool.name} ${number}`,
      running: true,
      env,
    };
    setTerminalTabs((value) => [...value, tab]);
    setCenterTab(id);
    setError("");
  };

  const handleExit = useCallback((id: string) => {
    setTerminalTabs((tabs) => tabs.map((tab) => (tab.id === id ? { ...tab, running: false } : tab)));
  }, []);
  const handleTerminalError = useCallback((message: string) => setError(message), []);

  const closeTerminal = async (tab: TerminalTab) => {
    if (tab.running) await api.closeTerminal(tab.id).catch((value) => setError(String(value)));
    setTerminalTabs((tabs) => tabs.filter((item) => item.id !== tab.id));
    if (centerTab === tab.id) setCenterTab("launchpad");
  };

  if (!initial || !settings || !settingsDraft) {
    return <main className="loading-screen">正在打开 aicross…</main>;
  }

  const shellClasses = [
    "shell",
    leftTabs.length === 0 ? "no-left" : "",
    rightTabs.length === 0 ? "no-right" : "",
    leftCollapsed ? "left-collapsed" : "",
    rightCollapsed ? "right-collapsed" : "",
  ].filter(Boolean).join(" ");

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">aicross</div>
        <div className="project-path mono" title={initial.projectPath}>{initial.projectPath}</div>
        <div className="topbar-spacer" />
        {initial.recentProjects.length > 0 && (
          <select className="recent-select" value="" onChange={(event) => void changeProject(event.target.value)} aria-label="最近项目">
            <option value="" disabled>最近项目</option>
            {initial.recentProjects.map((path) => <option value={path} key={path}>{path}</option>)}
          </select>
        )}
        <button className="button small" onClick={() => void chooseProject()}>打开目录</button>
        <button className="button small" onClick={() => setSettingsOpen(true)}>设置</button>
      </header>

      {error && <div className="global-error"><span>{error}</span><button onClick={() => setError("")}>关闭</button></div>}

      <main className={shellClasses}>
        {leftTabs.length > 0 && (
          <aside className="side-panel left-panel">
            {leftCollapsed ? (
              <button className="expand-button" onClick={() => setLeftCollapsed(false)} aria-label="展开左栏">›</button>
            ) : (
              <>
                <PanelTabs tabs={leftTabs} labels={leftLabels} active={leftTab} setActive={setLeftTab} onCollapse={() => setLeftCollapsed(true)} />
                {leftTab === "workspace" && <Workspace projectPath={initial.projectPath} />}
                {leftTab === "resources" && <Placeholder title="资源" text="第 1 期接入" />}
                {leftTab === "sessions" && <Sessions />}
              </>
            )}
          </aside>
        )}

        <section className="center-panel">
          <nav className="tabbar center-tabs" aria-label="终端 tab">
            <button className={`tab ${centerTab === "launchpad" ? "active" : ""}`} onClick={() => setCenterTab("launchpad")}>启动台</button>
            {terminalTabs.map((tab) => (
              <div className={`terminal-tab ${centerTab === tab.id ? "active" : ""}`} key={tab.id}>
                <button className="terminal-tab-label" onClick={() => setCenterTab(tab.id)}>
                  <span className={`status-dot ${tab.running ? "running" : "exited"}`} />
                  {tab.title}
                </button>
                <button className="tab-close" aria-label={`关闭 ${tab.title}`} onClick={() => void closeTerminal(tab)}>×</button>
              </div>
            ))}
          </nav>
          <div className="center-content">
            {centerTab === "launchpad" && (
              <Launchpad
                tools={tools}
                loading={loadingTools}
                expandedEnv={expandedEnv}
                env={toolEnv}
                onExpand={setExpandedEnv}
                onEnvChange={updateEnv}
                onLaunch={launch}
                onRefresh={refreshTools}
              />
            )}
            {terminalTabs.map((tab) => (
              <TerminalView key={tab.id} tab={tab} active={centerTab === tab.id} onExit={handleExit} onError={handleTerminalError} />
            ))}
          </div>
        </section>

        {rightTabs.length > 0 && (
          <aside className="side-panel right-panel">
            {rightCollapsed ? (
              <button className="expand-button" onClick={() => setRightCollapsed(false)} aria-label="展开右栏">‹</button>
            ) : (
              <>
                <PanelTabs tabs={rightTabs} labels={rightLabels} active={rightTab} setActive={setRightTab} onCollapse={() => setRightCollapsed(true)} />
                {rightTab === "changes" && <Changes projectPath={initial.projectPath} />}
                {rightTab === "plan" && <Placeholder title="方案与编排" text="第 2 期接入" />}
                {rightTab === "tasks" && <Placeholder title="任务列表" text="尚未启用" />}
              </>
            )}
          </aside>
        )}
      </main>

      {settingsOpen && (
        <SettingsDialog value={settingsDraft} onChange={setSettingsDraft} onSave={() => void saveSettings()} onClose={() => { setSettingsDraft(initial.settings); setSettingsOpen(false); }} />
      )}
    </div>
  );
}

function PanelTabs<T extends string>({
  tabs,
  labels,
  active,
  setActive,
  onCollapse,
}: {
  tabs: T[];
  labels: Record<T, string>;
  active: T;
  setActive: (tab: T) => void;
  onCollapse: () => void;
}) {
  return (
    <nav className="tabbar panel-tabs">
      {tabs.map((tab) => <button key={tab} className={`tab ${active === tab ? "active" : ""}`} onClick={() => setActive(tab)}>{labels[tab]}</button>)}
      <button className="collapse-button" onClick={onCollapse} aria-label="折叠栏">收起</button>
    </nav>
  );
}

function Placeholder({ title, text }: { title: string; text: string }) {
  return <div className="placeholder"><div className="placeholder-title">{title}</div><div>{text}</div></div>;
}

function Launchpad({
  tools,
  loading,
  expandedEnv,
  env,
  onExpand,
  onEnvChange,
  onLaunch,
  onRefresh,
}: {
  tools: Tool[];
  loading: boolean;
  expandedEnv: string | null;
  env: Record<string, EnvEntry[]>;
  onExpand: (id: string | null) => void;
  onEnvChange: (id: string, rows: EnvEntry[]) => void;
  onLaunch: (tool: Tool) => void;
  onRefresh: () => void;
}) {
  return (
    <div className="launchpad">
      <div className="launchpad-head">
        <div><h1>启动台</h1><p>在当前项目目录打开本机 CLI。</p></div>
        <button className="button small" disabled={loading} onClick={onRefresh}>{loading ? "检测中" : "重新检测"}</button>
      </div>
      <div className="tool-list">
        {tools.map((tool) => {
          const rows = env[tool.id] ?? [];
          return (
            <article className="tool-card" key={tool.id}>
              <div className="tool-main">
                <div className="tool-copy">
                  <div className="tool-title-row"><h2>{tool.name}</h2><span className={`detection ${tool.detected ? "detected" : "missing"}`}>{tool.detected ? "已检测" : "未检测"}</span></div>
                  <p>{tool.description}</p>
                  <div className="tool-version mono">{tool.detected ? tool.version : tool.binary}</div>
                </div>
                <div className="tool-actions">
                  <button className="button small" onClick={() => onExpand(expandedEnv === tool.id ? null : tool.id)}>环境变量</button>
                  <button className="button small primary" disabled={!tool.detected} onClick={() => onLaunch(tool)}>启动</button>
                </div>
              </div>
              {expandedEnv === tool.id && <EnvironmentEditor rows={rows} onChange={(value) => onEnvChange(tool.id, value)} />}
            </article>
          );
        })}
      </div>
    </div>
  );
}

function EnvironmentEditor({ rows, onChange }: { rows: EnvEntry[]; onChange: (rows: EnvEntry[]) => void }) {
  const update = (index: number, patch: Partial<EnvEntry>) => onChange(rows.map((row, rowIndex) => rowIndex === index ? { ...row, ...patch } : row));
  return (
    <div className="env-editor">
      <div className="hint">仅传给本次进程，值不写入日志。</div>
      {rows.map((row, index) => (
        <div className="env-row" key={index}>
          <input aria-label="环境变量名" placeholder="变量名" value={row.key} onChange={(event) => update(index, { key: event.target.value })} />
          <input aria-label="环境变量值" type="password" placeholder="值" value={row.value} onChange={(event) => update(index, { value: event.target.value })} />
          <button className="icon-button" aria-label="删除环境变量" onClick={() => onChange(rows.filter((_, rowIndex) => rowIndex !== index))}>×</button>
        </div>
      ))}
      <button className="button small" onClick={() => onChange([...rows, { key: "", value: "" }])}>添加变量</button>
    </div>
  );
}
