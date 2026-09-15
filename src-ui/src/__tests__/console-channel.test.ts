import { describe, expect, it } from 'vitest';
import { addWindowsPathEntry, consoleCancelTargets, consoleInflightCount, consoleTabPlan, mergeConsoleCommit, removeWindowsPathEntry, selectConsoleGraph } from '../console-channel';
import { newGraph } from '../graph/types';
import { startPipelineHeadless } from '../graph/dispatch';
import type { TerminalSession } from '../store/app-state';
import type { ConsoleRun } from '../tauri';
import overlaySource from '../components/center/graph/DispatchOverlay.tsx?raw';
import dispatchSource from '../graph/dispatch.ts?raw';

const session = (value: Partial<TerminalSession> & Pick<TerminalSession, 'id' | 'tool'>): TerminalSession => ({ folderPath: 'D:/p', ...value });

describe('TASK-16 R1 控制台薄中继', () => {
  it('总指挥前台只后台开图，普通前台自动打开或切换', () => {
    const commander = [session({ id: 'commander', tool: 'codex', commanderTitle: 'Codex · p' })];
    expect(consoleTabPlan(commander, 'commander', 'g')).toEqual({ kind: 'background' });
    const ordinary = [session({ id: 'terminal', tool: 'terminal' })];
    expect(consoleTabPlan(ordinary, 'terminal', 'g')).toEqual({ kind: 'open' });
    ordinary.push(session({ id: 'graph', tool: 'graph', toolData: 'g' }));
    expect(consoleTabPlan(ordinary, 'terminal', 'g')).toEqual({ kind: 'focus', sessionId: 'graph' });
  });

  it('在飞计数只用后端回报的真实节点列表', () => {
    const runs = [{ run_id: 'r', graph_id: 'g', project: 'p', status: 'running', started_at: '', in_flight_node_ids: ['a'], nodes: [
      { node_id: 'a', role: '验证', agent: 'pi', model: 'a', status: 'running', text_excerpt: '', artifacts_dir: '' },
      { node_id: 'b', role: '验证', agent: 'pi', model: 'b', status: 'running', text_excerpt: '', artifacts_dir: '' },
    ] }] as ConsoleRun[];
    expect(consoleInflightCount(runs)).toBe(1);
  });

  it('新图把 project 写在 Graph 上，旧图按 tab 目录匹配', () => {
    const graph = newGraph('D:/Project');
    expect(graph.project).toBe('D:/Project');
    expect((graph.nodes[graph.root] as unknown as { project?: string }).project).toBeUndefined();
    const legacy = { ...newGraph(), id: 'legacy' };
    const state = { graphs: { legacy }, terminals: [session({ id: 'tab', tool: 'graph', toolData: 'legacy', folderPath: 'd:\\project\\' })], activeTerminalId: 'tab' };
    expect(selectConsoleGraph(state as any, 'D:/PROJECT')).toBe(legacy);
  });

  it('PATH 项按大小写和尾反斜杠匹配，只增删自己的一项', () => {
    const original = '%USERPROFILE%\\bin;C:\\Tools;D:\\Apps\\aicross\\';
    expect(addWindowsPathEntry(original, 'd:\\apps\\AICROSS')).toBe(original);
    expect(addWindowsPathEntry('%USERPROFILE%\\bin', 'D:\\Apps\\aicross')).toBe('%USERPROFILE%\\bin;D:\\Apps\\aicross');
    expect(removeWindowsPathEntry(original, 'd:\\apps\\AICROSS')).toBe('%USERPROFILE%\\bin;C:\\Tools');
    expect(removeWindowsPathEntry(null, 'D:\\Apps\\aicross')).toBeNull();
    expect(removeWindowsPathEntry(original, 'D:\\missing')).toBe(original);
  });

  it('DispatchOverlay 与控制台通道共用 startPipelineHeadless', () => {
    expect(dispatchSource).toContain('export async function startPipelineHeadless');
    expect(overlaySource).toContain('await startPipelineHeadless({');
    expect(overlaySource).not.toContain('await runStageLoop(');
  });

  it('startPipelineHeadless 负责完整阶段和 finally 收尾', async () => {
    const base = newGraph();
    const nodeId = 'n';
    const graph = { ...base, nodes: { ...base.nodes, n: { id: nodeId, kind: 'turn' as const, parents: [base.root], user: { text: '', attachments: [] }, reply: { status: 'draft' as const, text: '' }, target: { agent: 'pi', model: 'm', role: '验证', thinking: '中' as const, visibility: '盲' as const, write: '只读' as const }, created_at: new Date().toISOString() } } };
    const calls: string[] = [];
    const result = await startPipelineHeadless({
      graph,
      settings: { maxRequests: 3 },
      begin: () => calls.push('begin'),
      end: () => calls.push('end'),
      deps: {
        runId: 'run',
        buildRequest: (_graph, ids) => ({ stage: 1, nodes: ids.map((id) => ({ nodeId: id })) }),
        preview: async () => null,
        dispatch: async () => ({ runId: 'run', results: [{ nodeId, status: 'ok', text: '完成', modelReported: 'm', sessionId: '', tokensIn: 1, tokensOut: 1, durationMs: 1, error: '' }] }),
        commit: (value) => value,
        onPreview: () => {},
        onStage: () => calls.push('stage'),
        onResults: () => calls.push('results'),
      },
    });
    expect(result.outcome.kind).toBe('done');
    expect(result.graph.nodes[nodeId].reply?.status).toBe('done');
    expect(calls).toEqual(['begin', 'stage', 'results', 'end']);
  });
});

describe('console commit merges onto the latest graph (R1 review 1)', () => {
  it('keeps a concurrent user edit on another node while applying the pipeline reply', () => {
    const base = newGraph('D:/proj');
    const rootId = base.root;
    const previous = { ...base, nodes: { ...base.nodes, a: { ...base.nodes[rootId], id: 'a', parents: [rootId], user: { text: '执行', attachments: [] }, reply: { status: 'pending', text: '' } }, b: { ...base.nodes[rootId], id: 'b', parents: [rootId], user: { text: '旁边', attachments: [] } } } } as typeof base;
    // 用户在运行期间改了 b 的 prompt（store 里的最新图）
    const latest = { ...previous, nodes: { ...previous.nodes, b: { ...previous.nodes.b, user: { text: '旁边（用户改过）', attachments: [] } } } } as typeof base;
    // 流水线只把 a 的回复落下来
    const next = { ...previous, nodes: { ...previous.nodes, a: { ...previous.nodes.a, reply: { status: 'done', text: '完成' } } } } as typeof base;
    const merged = mergeConsoleCommit(latest, previous, next);
    expect(merged.nodes.a.reply?.status).toBe('done');
    expect(merged.nodes.b.user.text).toBe('旁边（用户改过）');
  });
});

describe('console cancel targets (R1 review N1)', () => {
  it('unions the summary in-flight ids with pending nodes on the graph', () => {
    const base = newGraph('D:/proj');
    const graph = { ...base, nodes: { ...base.nodes, a: { ...base.nodes[base.root], id: 'a', parents: [base.root], reply: { status: 'pending', text: '' } }, b: { ...base.nodes[base.root], id: 'b', parents: [base.root], reply: { status: 'done', text: 'x' } } } } as typeof base;
    expect(consoleCancelTargets({ in_flight_node_ids: ['b'] }, graph).sort()).toEqual(['a', 'b']);
    expect(consoleCancelTargets({ in_flight_node_ids: [] }, undefined)).toEqual([]);
  });
});
