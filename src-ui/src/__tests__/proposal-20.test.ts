import { describe, expect, it, vi } from 'vitest';
import type { Preset } from '../graph/types';
import {
  acceptConsoleProposal,
  buildLocalProposalEvent,
  classificationTarget,
  classifyProposal,
  dismissStaleConsoleProposals,
  getConsoleProposalNotices,
  getConsoleProposals,
  proposalClassificationVerdict,
  proposalDecision,
  proposalDisplayGraphId,
  proposalIsForeign,
  proposalPresets,
  proposalsForTab,
  runConsoleProposal,
} from '../console-channel';
import { LocalProposalButton, orderProposalPresets, proposalBarText, proposalShortcutFromTarget } from '../components/center/ProposalBar';
import { presetDisplayName } from '../graph/label';

const preset = (id: string, builtin = true, name = id): Preset => ({ id, builtin, name, stages: [{ nodes: [] }] });

describe('TASK-20 建议条纯函数', () => {
  it('按基础三模式、自定义、更多模式循环', () => {
    const values = [
      preset('builtin-swarm'), preset('custom-b', false), preset('builtin-blind'),
      preset('builtin-economy'), preset('builtin-jury'), preset('custom-a', false), preset('builtin-shared'), preset('builtin-arena'),
    ];
    expect(orderProposalPresets(values).map((value) => value.id)).toEqual([
      'builtin-shared', 'builtin-economy', 'builtin-blind', 'custom-b', 'custom-a', 'builtin-arena', 'builtin-jury', 'builtin-swarm',
    ]);
  });

  it('从事件目标同时计算输入与 PTY 状态，四类目标走各自的最终动作', () => {
    const root = { querySelector: () => null };
    const target = (tagName: string, pty = false) => ({
      tagName,
      isContentEditable: false,
      classList: { contains: (name: string) => pty && name === 'xterm-helper-textarea' },
      closest: (selector: string) => pty && selector === '.xterm-helper-textarea' ? {} : null,
    });
    const xterm = target('TEXTAREA', true);
    expect(proposalShortcutFromTarget({ key: 'Enter', ctrlKey: true }, { commander: true }, xterm, root)).toBe('accept');
    expect(proposalShortcutFromTarget({ key: 'Escape' }, { commander: true }, xterm, root)).toBeNull();
    expect(proposalShortcutFromTarget({ key: 'Tab' }, { commander: true }, xterm, root)).toBeNull();
    expect(proposalShortcutFromTarget({ key: 'Enter' }, { commander: false }, target('INPUT'), root)).toBeNull();
    expect(proposalShortcutFromTarget({ key: 'Enter' }, { commander: false }, target('TEXTAREA'), root)).toBeNull();
    expect(proposalShortcutFromTarget({ key: 'Enter' }, { commander: false }, target('DIV'), root)).toBe('accept');
    expect(proposalShortcutFromTarget({ key: 'Enter', isComposing: true }, { commander: false }, target('DIV'), root)).toBeNull();
  });

  it('页面存在 modal 时建议条不吃 Esc', () => {
    const root = { querySelector: (selector: string) => selector === '[aria-modal="true"]' ? {} : null };
    expect(proposalShortcutFromTarget({ key: 'Escape' }, { commander: false }, { tagName: 'DIV' }, root)).toBeNull();
  });

  it('reason 作为末尾浅色说明进入建议条文本', () => {
    expect(proposalBarText('盲审模式', 2, '需要独立核对', false)).toBe('建议派发 · 盲审模式 · 2 路 · 回车采纳 · Esc 忽略 · Tab 换模式 · 需要独立核对');
    expect(proposalBarText('经济模式', 3, '', true)).toBe('建议派发 · 经济模式 · 3 路 · Ctrl+Enter 采纳 · 或点右侧按钮');
    expect(proposalBarText('经济模式', 3, '', false, true)).toBe('建议派发 · 经济模式 · 3 路 · 点右侧按钮');
  });
});

describe('TASK-20 建议判定与本地入口', () => {
  it('小任务直接跳过；开关关闭不跑模型，开启后只跑一次且需双方同意', async () => {
    const classify = vi.fn(async () => false);
    await expect(proposalDecision('解释这句话', [], false, classify)).resolves.toEqual({ propose: false, skipped: '单步小任务' });
    expect(classify).not.toHaveBeenCalled();
    await expect(proposalDecision('实现这个功能并补测试', [], false, classify)).resolves.toEqual({ propose: true });
    expect(classify).not.toHaveBeenCalled();
    await expect(proposalDecision('实现这个功能并补测试', [], true, classify)).resolves.toEqual({ propose: false, skipped: '归类模型判定不必派发' });
    expect(classify).toHaveBeenCalledTimes(1);
  });

  it('派发按钮生成默认经济模式的本地建议事件', () => {
    const onPropose = vi.fn();
    const button = LocalProposalButton({ text: '  实现并验证修改  ', attachments: ['D:/a.png'], className: 'test', onPropose });
    (button.props as { onClick: () => void }).onClick();
    expect(button.props.children).toBe('建议派发');
    expect(onPropose).toHaveBeenCalledWith('实现并验证修改', ['D:/a.png']);
    expect(buildLocalProposalEvent({ project: 'D:/p', graphId: 'g', text: '实现并验证修改', attachments: [] }, () => 'local-1')).toEqual({
      proposal_id: 'local-1', project: 'D:/p', graph_id: 'g', text: '实现并验证修改', attachments: [], preset: 'builtin-economy', reason: '', local: true,
    });
  });

  it('/v1/propose 的前端回执只登记建议，不调用 dispatch_graph_stage', async () => {
    const dispatchGraphStage = vi.fn(async () => { throw new Error('不应派发'); });
    const updates: Array<{ status: string }> = [];
    const economy = { id: 'builtin-economy', name: '经济模式', builtin: true, stages: [{ nodes: [{ agent: 'pi', provider: 'deepseek', model: 'm', role: '验证', thinking: '中', visibility: '盲', write: '只读' }] }] } as Preset;
    const io = {
      listDispatchableModels: async () => ({ models: [{ id: 'm', agent: 'pi', provider: 'deepseek', vendor: 'deepseek', billing_source: '', tier: '低', thinking: '低' }], error: null }),
      listPresets: async () => ({ version: 1 as const, presets: [economy] }),
      loadRoles: async () => ({ version: 1 as const, planning: {}, execution: {}, acceptance: {} }),
      dispatchGraphStage,
      consoleRunUpdate: async (update: { status: string }) => { updates.push(update); },
    } as any;
    const context = { getState: () => ({ graphs: {}, terminals: [], activeTerminalId: null }), dispatch: vi.fn() } as any;
    await runConsoleProposal({ proposal_id: 'p1', project: 'D:/p', text: '实现这个功能并补测试', preset: 'builtin-economy' }, context, io);
    expect(dispatchGraphStage).not.toHaveBeenCalled();
    expect(updates).toEqual([{ run_id: 'p1', graph_id: '', status: 'proposed', nodes: [], leftover: [], in_flight_node_ids: [], reason: '' }]);
  });

  it('登记失败时不先显示幽灵建议', async () => {
    const economy = { id: 'builtin-economy', name: '经济模式', builtin: true, stages: [{ nodes: [] }] } as Preset;
    const io = {
      listPresets: async () => ({ version: 1 as const, presets: [economy] }),
      loadRoles: async () => ({ version: 1 as const, planning: {}, execution: {}, acceptance: {} }),
      consoleRunUpdate: async () => { throw new Error('找不到 run：late-proposal'); },
    } as any;
    const context = { getState: () => ({ graphs: {}, terminals: [], activeTerminalId: null }), dispatch: vi.fn() } as any;
    await expect(runConsoleProposal({ proposal_id: 'late-proposal', project: 'D:/p', text: '实现这个功能并补测试' }, context, io)).rejects.toThrow('找不到 run');
    expect(getConsoleProposals().some((item) => item.proposal_id === 'late-proposal')).toBe(false);
  });

  it('本地建议先登记进 run 表，同项目多条按 id 合并并显示余量', async () => {
    const economy = { id: 'builtin-economy', name: '经济模式', builtin: true, stages: [{ nodes: [] }] } as Preset;
    const registered: string[] = [];
    const io = {
      listPresets: async () => ({ version: 1 as const, presets: [economy] }),
      loadRoles: async () => ({ version: 1 as const, planning: {}, execution: {}, acceptance: {} }),
      consoleRegisterProposal: async (id: string) => { registered.push(id); },
    } as any;
    const context = { getState: () => ({ graphs: {}, terminals: [], activeTerminalId: null }), dispatch: vi.fn() } as any;
    await runConsoleProposal({ proposal_id: 'local-a', project: 'D:/same', text: '实现甲功能并补测试', local: true }, context, io);
    await runConsoleProposal({ proposal_id: 'local-b', project: 'D:/same', text: '实现乙功能并补测试', local: true }, context, io);
    expect(registered).toEqual(['local-a', 'local-b']);
    expect(proposalsForTab(getConsoleProposals(), 'D:/same')).toHaveLength(2);
    expect(proposalsForTab(getConsoleProposals(), 'D:/missing', '', true).map((item) => item.proposal_id)).toEqual(expect.arrayContaining(['local-a', 'local-b']));
  });

  it('目标项目没有打开的 tab 时把建议放到当前对话图', () => {
    const state = {
      graphs: { active: { id: 'active', project: 'D:/other' } },
      terminals: [{ id: 'tab', tool: 'graph', toolData: 'active', folderPath: 'D:/other' }],
      activeTerminalId: 'tab',
    } as any;
    expect(proposalDisplayGraphId(state, 'D:/target')).toBe('active');
  });

  it('targets 建议的模式名不会重复为“自定义模式 · 自定义模式”', async () => {
    const user = { id: 'custom-1', name: '核对', builtin: false, stages: [{ nodes: [] }] } as Preset;
    const io = {
      listPresets: async () => ({ version: 1 as const, presets: [user] }),
      loadRoles: async () => ({ version: 1 as const, planning: {}, execution: {}, acceptance: {} }),
      consoleRegisterProposal: async () => {},
    } as any;
    const context = { getState: () => ({ graphs: {}, terminals: [], activeTerminalId: null }), dispatch: vi.fn() } as any;
    await runConsoleProposal({
      proposal_id: 'targets-proposal', project: 'D:/targets', text: '实现目标并补测试', local: true,
      targets: [{ agent: 'codex', model: 'm', role: '验证', thinking: '中', visibility: '盲', write: '只读' }],
    }, context, io);
    const presets = proposalPresets('targets-proposal');
    expect(presetDisplayName(presets[0], presets.filter((item) => !item.builtin).length)).toBe('自定义模式');
  });

  it('采纳撞在飞上限时移除建议并显示明确原因', async () => {
    const economy = { id: 'builtin-economy', name: '经济模式', builtin: true, stages: [{ nodes: [] }] } as Preset;
    const setupIo = {
      listPresets: async () => ({ version: 1 as const, presets: [economy] }),
      loadRoles: async () => ({ version: 1 as const, planning: {}, execution: {}, acceptance: {} }),
      consoleRegisterProposal: async () => {},
    } as any;
    const context = { getState: () => ({ graphs: {}, terminals: [], activeTerminalId: null }), dispatch: vi.fn() } as any;
    await runConsoleProposal({ proposal_id: 'limit-proposal', project: 'D:/limit', text: '实现限额功能并补测试', local: true }, context, setupIo);
    const consoleRunUpdate = vi.fn(async () => {});
    const limitIo = {
      listConsoleRuns: async () => [0, 1, 2].map((index) => ({ run_id: `r${index}`, status: 'running' })),
      consoleRunUpdate,
    } as any;
    await acceptConsoleProposal('limit-proposal', limitIo, context);
    expect(consoleRunUpdate).not.toHaveBeenCalled();
    expect(getConsoleProposals().some((item) => item.proposal_id === 'limit-proposal')).toBe(false);
    expect(getConsoleProposalNotices().find((item) => item.project === 'D:/limit')?.text).toBe('在飞已满，建议已撤回');
  });

  it('启动时把无法重建正文的遗留建议翻成 dismissed', async () => {
    const updates: Array<{ run_id: string; status: string }> = [];
    await dismissStaleConsoleProposals({
      listConsoleRuns: async () => [
        { run_id: 'stale', graph_id: 'g', project: 'D:/p', status: 'proposed', nodes: [], started_at: '1' },
        { run_id: 'live', graph_id: 'g', project: 'D:/p', status: 'running', nodes: [], started_at: '2' },
      ],
      consoleRunUpdate: async (update: any) => { updates.push(update); },
    });
    expect(updates).toMatchObject([{ run_id: 'stale', status: 'dismissed' }]);
  });

  it('归类只认回复里的第一个判词，失败与否决使用不同说明', async () => {
    expect(proposalClassificationVerdict('SINGLE，本次不派。规则要求输出 DISPATCH 或 SINGLE。')).toBe(false);
    expect(proposalClassificationVerdict('DISPATCH\n适合多路核对')).toBe(true);
    await expect(proposalDecision('实现这个功能并补测试', [], true, async () => { throw new Error('模型不可用'); }))
      .resolves.toEqual({ propose: false, skipped: '分类失败，已按不派处理：模型不可用' });
    expect(() => classificationTarget(JSON.stringify(['codex', 'openai', 'missing']), [{ id: 'other', agent: 'codex', provider: 'openai' } as any]))
      .toThrow('所选归类模型当前不可用');
  });

  it('归类调用使用 20 秒上限并复用固定 run 目录', async () => {
    const requests: any[] = [];
    const model = { id: 'm', agent: 'codex', provider: 'openai', vendor: 'openai', billing_source: '', tier: '低', thinking: '低' };
    const io = {
      listDispatchableModels: async () => ({ models: [model], error: null }),
      dispatchGraphStage: async (request: any) => {
        requests.push(request);
        return { results: [{ status: 'ok', text: 'DISPATCH' }] };
      },
    } as any;
    await expect(classifyProposal({ proposal_id: 'p', project: 'D:/p', text: '实现功能并补测试' }, { timeoutSecs: 600, classificationModel: '' } as any, io)).resolves.toBe(true);
    expect(requests[0]).toMatchObject({ runId: 'proposal-classify', settings: { timeoutSecs: 20 }, nodes: [{ nodeId: 'proposal-classify' }] });
  });
});

describe('复审残留：外项目建议要带出目标目录（09-16 编排者补）', () => {
  const proposal = {
    proposal_id: 'p1', project: 'D:/work/x', graph_id: 'g-x', preset_id: 'builtin-economy',
    node_count: 2, reason: '', created_at: '', text: '', attachments: [], local: false,
  } as never;

  it('同一张图上的建议不算外项目', () => {
    expect(proposalIsForeign(proposal, 'D:/work/x', 'g-x')).toBe(false);
  });

  it('同一个项目目录（大小写不同）也不算外项目', () => {
    expect(proposalIsForeign(proposal, 'd:/work/X', '')).toBe(false);
  });

  it('画在别的项目 tab 上时算外项目', () => {
    expect(proposalIsForeign(proposal, 'D:/work/y', 'g-y')).toBe(true);
  });

  it('控制台兜底（没有项目上下文）也算外项目', () => {
    expect(proposalIsForeign(proposal, '', '')).toBe(true);
  });
});
