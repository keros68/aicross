import { describe, expect, it } from 'vitest';
import { consoleInflightCount, consoleTabPlan } from '../console-channel';
import type { TerminalSession } from '../store/app-state';
import type { ConsoleRun } from '../tauri';
import graphSource from '../components/center/graph/GraphTab.tsx?raw';
import boardSource from '../components/right/TaskBoard.tsx?raw';
import centerSource from '../components/center/CenterPanel.tsx?raw';
import installerSource from '../../../installer.nsi?raw';
import wixPathSource from '../../../windows/fragments/path.wxs?raw';
import tauriConfigSource from '../../../tauri.conf.json?raw';

const session = (value: Partial<TerminalSession> & Pick<TerminalSession, 'id' | 'tool'>): TerminalSession => ({
  folderPath: 'D:/p',
  ...value,
});

describe('TASK-16 控制台通道界面', () => {
  it('总指挥在前台时后台开图，不抢焦点', () => {
    const tabs = [session({ id: 'commander', tool: 'codex', commanderTitle: 'Codex · p' })];
    expect(consoleTabPlan(tabs, 'commander', 'graph-1')).toEqual({ kind: 'background' });
  });

  it('非总指挥前台时自动打开或切换通道图', () => {
    const empty = [session({ id: 'terminal', tool: 'terminal' })];
    expect(consoleTabPlan(empty, 'terminal', 'graph-1')).toEqual({ kind: 'open' });
    const withGraph = [...empty, session({ id: 'graph-tab', tool: 'graph', toolData: 'graph-1' })];
    expect(consoleTabPlan(withGraph, 'terminal', 'graph-1')).toEqual({ kind: 'focus', sessionId: 'graph-tab' });
  });

  it('标题计数只统计运行中的路', () => {
    const runs = [{
      run_id: 'r', graph_id: 'g', project: 'p', status: 'running', started_at: '',
      nodes: [
        { node_id: 'a', role: '验证', agent: 'pi', model: 'a', status: 'running', text_excerpt: '', artifacts_dir: '' },
        { node_id: 'b', role: '验证', agent: 'pi', model: 'b', status: 'done', text_excerpt: '', artifacts_dir: '' },
      ],
    }] as ConsoleRun[];
    expect(consoleInflightCount(runs)).toBe(1);
  });

  it('图节点有来源标记，任务板复用 dispatch-progress 心跳', () => {
    expect(graphSource).toContain("node.source === 'console'");
    expect(graphSource).toContain('来自总指挥');
    expect(boardSource).toContain('subscribeProgress(run.run_id');
    expect(boardSource).toContain('来自总指挥的在飞任务');
    expect(centerSource).toContain("graph?.source === 'console'");
  });

  it('Windows 安装与卸载成对维护 aicross 的 PATH', () => {
    expect(installerSource).toContain('WriteRegExpandStr HKCU "Environment" "Path" "$R8;$INSTDIR"');
    expect(installerSource).toContain('${StrRep} $R8 "$R8" ";$INSTDIR;" ";"');
    expect(installerSource.match(/WM_SETTINGCHANGE/g)?.length).toBe(2);
    expect(wixPathSource).toContain('Id="AicrossConsolePath"');
    expect(wixPathSource).toContain('Permanent="no"');
    expect(tauriConfigSource).toContain('"componentRefs": ["AicrossConsolePath"]');
  });
});
