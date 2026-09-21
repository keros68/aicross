import { describe, expect, it } from 'vitest';
import {
  CONSOLE_CATCHUP_PREFIX,
  consoleCatchupNotice,
  consoleReplyResumeKey,
  projectConsoleCatchup,
} from '../console-projection';
import { loadConsoleSessions, persistConsoleSessions } from '../console-persistence';
import { consoleResumeKey, type ConsoleMessage, type ConsoleTargetAgent } from '../console-session';
import type { TerminalSession } from '../store/app-state';

const codex: ConsoleTargetAgent = { agent: 'codex', provider: '', model: 'gpt-5' };
const kimi: ConsoleTargetAgent = { agent: 'kimi', provider: '', model: 'kimi-code/k3' };
const mentions = [codex, kimi];

let seq = 0;
function message(kind: ConsoleMessage['kind'], text: string, detail: Partial<ConsoleMessage> = {}): ConsoleMessage {
  seq += 1;
  return { id: `m${seq}`, kind, text, createdAt: '2026-09-21T00:00:00.000Z', ...detail };
}
function said(target: ConsoleTargetAgent, text: string, detail: Partial<ConsoleMessage> = {}): ConsoleMessage {
  return message('assistant', text, { ...target, status: 'ok', resumeKey: consoleResumeKey(target), ...detail });
}

const twoRounds = [
  message('user', '给个缓存方案'),
  said(codex, '方案甲：先读后写'),
  message('user', '再细一点'),
  said(codex, '方案甲细化：加版本号'),
];

describe('控制台 @ 别家时补上它没看到的对话', () => {
  it('目标从未说过话时取全部，按口径标记并以当前消息收尾', () => {
    const result = projectConsoleCatchup(twoRounds, kimi, '上面的方案有什么问题', { fresh: false, mentions });
    expect(result.included).toBe(4);
    expect(result.omitted).toBe(0);
    expect(result.text).toBe([
      '以下是本对话中你尚未看到的部分，仅供了解背景；只需回应最后的「当前消息」。',
      '[用户]\n给个缓存方案',
      '[模型回复 codex/gpt-5]\n方案甲：先读后写',
      '[用户]\n再细一点',
      '[模型回复 codex/gpt-5]\n方案甲细化：加版本号',
      '[当前消息]\n上面的方案有什么问题',
    ].join('\n\n'));
    expect(result.text.startsWith(CONSOLE_CATCHUP_PREFIX)).toBe(true);
  });

  it('目标说过话时只取它最后一条之后的；fresh 时仍取全部', () => {
    const messages = [
      ...twoRounds,
      message('user', '@kimi 上面的方案有什么问题'),
      said(kimi, '版本号没说怎么回收'),
      message('user', '补上回收'),
      said(codex, '加 TTL'),
    ];
    const resumed = projectConsoleCatchup(messages, kimi, '继续', { fresh: false, mentions });
    expect(resumed.included).toBe(2);
    expect(resumed.text).toContain('[用户]\n补上回收\n\n[模型回复 codex/gpt-5]\n加 TTL\n\n[当前消息]\n继续');
    expect(resumed.text).not.toContain('方案甲');
    expect(resumed.text).not.toContain('版本号没说怎么回收');

    const fresh = projectConsoleCatchup(messages, kimi, '继续', { fresh: true, mentions });
    expect(fresh.included).toBe(8);
    expect(fresh.text).toContain('方案甲：先读后写');
    expect(fresh.text).toContain('[模型回复 kimi/kimi-code/k3]\n版本号没说怎么回收');

    // 回到 codex：只补 kimi 那一轮，不含 codex 自己说过的。
    const back = projectConsoleCatchup(messages.slice(0, 6), codex, '你怎么看', { fresh: false, mentions });
    expect(back.included).toBe(2);
    expect(back.text).toContain('[用户]\n上面的方案有什么问题');
    expect(back.text).not.toContain('方案甲');
  });

  it('目标的上一条之后没有新对话时不补', () => {
    const messages = [...twoRounds, message('user', '@kimi 看看'), said(kimi, '看过了')];
    const result = projectConsoleCatchup(messages, kimi, '继续', { fresh: false, mentions });
    expect(result).toEqual({ text: '继续', included: 0, omitted: 0 });
  });

  it('同 agent 不同 model 或不同 provider 视为没说过', () => {
    const messages = [
      message('user', '第一问'),
      said({ ...kimi, model: 'kimi-code/k2' }, '旧型号的回答'),
      said({ ...kimi, provider: 'other' }, '别的来源的回答'),
    ];
    const result = projectConsoleCatchup(messages, kimi, '第二问', { fresh: false, mentions });
    expect(result.included).toBe(3);
    expect(result.text).toContain('[模型回复 kimi/kimi-code/k2]\n旧型号的回答');
  });

  it('有 resumeKey 的回复只按它认会话，显示用的 model 不一样也命中', () => {
    const pi: ConsoleTargetAgent = { agent: 'pi', provider: 'deepseek', model: 'deepseek-flash' };
    const messages = [
      message('user', '第一问'),
      // 引擎报回的真实型号是 provider/model 形状，与派发目标的 model 不等。
      said(pi, '第一答', { model: 'deepseek/deepseek-flash', resumeKey: consoleResumeKey(pi) }),
      message('user', '@codex 第二问'),
      said(codex, '第二答', { resumeKey: consoleResumeKey(codex) }),
    ];
    const result = projectConsoleCatchup(messages, pi, '第三问', { fresh: false, mentions });
    expect(result.included).toBe(2);
    expect(result.text).not.toContain('第一问');
    expect(result.text).toContain('[用户]\n第二问\n\n[模型回复 codex/gpt-5]\n第二答');
  });

  it('resumeKey 对不上时不当锚点，即使三项全等', () => {
    const messages = [
      message('user', '第一问'),
      said(kimi, '别的会话说的', { resumeKey: consoleResumeKey({ ...kimi, provider: 'other' }) }),
    ];
    const result = projectConsoleCatchup(messages, kimi, '第二问', { fresh: false, mentions });
    expect(result.included).toBe(2);
    expect(result.text).toContain('[用户]\n第一问');
  });

  it('没有 resumeKey 的回复不当锚点：旧消息、模式派发结果、没成功的那一轮', () => {
    const asked = message('user', '第一问');
    const without = (detail: Partial<ConsoleMessage>) => projectConsoleCatchup(
      [asked, said(kimi, '回复', { resumeKey: undefined, ...detail })], kimi, '第二问', { fresh: false, mentions },
    );
    // 三项全等也不算：正文照收，连同用户那条一起补。
    expect(without({}).included).toBe(2);
    expect(without({ stage: 1, role: '执行' }).included).toBe(2);
    // 没成功的正文不收，只补用户那条。
    for (const status of ['error', 'timeout', 'quota_exhausted', 'identity_mismatch', 'validation_failed']) {
      const result = without({ status });
      expect(result.included).toBe(1);
      expect(result.text).toContain('[用户]\n第一问\n\n[当前消息]');
    }
  });

  it('只有成功且没走备选的那一轮才记续聊会话键', () => {
    expect(consoleReplyResumeKey(kimi, { status: 'ok', fallbackTo: '', sessionId: 's1' }, '你好')).toBe(consoleResumeKey(kimi));
    expect(consoleReplyResumeKey(kimi, { status: 'ok', fallbackTo: 'codex/gpt-5', sessionId: 's1' }, '你好')).toBe('');
    expect(consoleReplyResumeKey(kimi, { status: 'error', fallbackTo: '', sessionId: 's1' }, '你好')).toBe('');
    expect(consoleReplyResumeKey(kimi, { status: 'timeout', fallbackTo: '', sessionId: 's1' }, '你好')).toBe('');
    // 没回会话 id：没有可续的会话，下一轮按新会话补全部。
    expect(consoleReplyResumeKey(kimi, { status: 'ok', fallbackTo: '', sessionId: '' }, '你好')).toBe('');
    // 原生命令那一轮没补对话，不能当「它看到这里了」。
    expect(consoleReplyResumeKey(kimi, { status: 'ok', fallbackTo: '', sessionId: 's1' }, '/compact')).toBe('');
  });

  it('resumeKey 存盘读盘后还在，空串不留', () => {
    const data = new Map<string, string>();
    const storage = {
      getItem: (key: string) => data.get(key) ?? null,
      setItem: (key: string, value: string) => { data.set(key, value); },
    } as unknown as Storage;
    const session: TerminalSession = {
      id: 's35', tool: 'console', folderPath: null, targetAgent: kimi, resumeId: {},
      messages: [
        said(kimi, '有键', { resumeKey: consoleResumeKey(kimi) }),
        said(kimi, '空键', { resumeKey: '' }),
      ],
    };
    persistConsoleSessions([session], storage);
    const restored = loadConsoleSessions(storage)[0].messages ?? [];
    expect(restored[0].resumeKey).toBe(consoleResumeKey(kimi));
    expect('resumeKey' in restored[1]).toBe(false);
  });

  it('历史里的原生命令不收，带 @ 的剥完再判', () => {
    const messages = [
      message('user', '/compact'),
      message('user', '@kimi /skill:pdf 读一下'),
      message('user', '看一下 /tmp 目录'),
    ];
    const result = projectConsoleCatchup(messages, codex, '问', { fresh: true, mentions });
    expect(result.included).toBe(1);
    expect(result.text).toContain('[用户]\n看一下 /tmp 目录\n\n[当前消息]\n问');
    expect(result.text).not.toContain('/compact');
    expect(result.text).not.toContain('/skill:pdf');
  });

  it('system 消息、没成功的回复、空正文不进', () => {
    const messages = [
      message('user', '第一问'),
      message('system', '上次的会话已失效，已开新会话'),
      said(codex, '状态：失败\n原因：断网', { status: 'error' }),
      said(codex, '状态：超时\n原因：执行超过 600 秒', { status: 'timeout' }),
      said(codex, '状态：额度用尽\n原因：quota', { status: 'quota_exhausted' }),
      said(codex, ''),
      said(codex, '  \n'),
      said(codex, '正常回答'),
    ];
    const result = projectConsoleCatchup(messages, kimi, '第二问', { fresh: false, mentions });
    expect(result.included).toBe(2);
    expect(result.text).not.toContain('已失效');
    expect(result.text).not.toContain('断网');
    expect(result.text).not.toContain('超时');
    expect(result.text).not.toContain('额度用尽');
    expect(result.text).toContain('[用户]\n第一问\n\n[模型回复 codex/gpt-5]\n正常回答\n\n[当前消息]');
  });

  it('模型消息缺 agent 或 model 时省掉那一项，两项都缺写「模型」', () => {
    const messages = [
      message('assistant', '甲', { agent: 'codex' }),
      message('assistant', '乙', { model: 'gpt-5' }),
      message('assistant', '丙'),
    ];
    const result = projectConsoleCatchup(messages, kimi, '问', { fresh: false, mentions });
    expect(result.text).toContain('[模型回复 codex]\n甲');
    expect(result.text).toContain('[模型回复 gpt-5]\n乙');
    expect(result.text).toContain('[模型回复 模型]\n丙');
  });

  it('用户消息开头可识别的 @显示名 被剥，剥完为空的跳过，认不出的原样留', () => {
    const messages = [
      message('user', '@Kimi Code 看一下这个'),
      message('user', '@codex'),
      message('user', '@nobody 你好'),
    ];
    const result = projectConsoleCatchup(messages, codex, '问', { fresh: true, mentions });
    expect(result.included).toBe(2);
    expect(result.text).toContain('[用户]\n看一下这个\n\n[用户]\n@nobody 你好\n\n[当前消息]\n问');
  });

  it('单条超长保留头 3000 尾 800，中间写省略字数', () => {
    const body = `${'甲'.repeat(3000)}${'乙'.repeat(1200)}${'丙'.repeat(800)}`;
    const result = projectConsoleCatchup([message('user', body)], kimi, '问', { fresh: true, mentions });
    expect(result.included).toBe(1);
    expect(result.text).toContain(`[用户]\n${'甲'.repeat(3000)}…[省略 1200 字]…${'丙'.repeat(800)}\n\n[当前消息]`);
    expect(result.text).not.toContain('乙');

    // 正好 4000 字不截。
    const exact = '丁'.repeat(4000);
    const kept = projectConsoleCatchup([message('user', exact)], kimi, '问', { fresh: true, mentions });
    expect(kept.text).toContain(exact);
    expect(kept.text).not.toContain('省略');
  });

  it('截断点不落在代理对中间，省略字数按实际算', () => {
    const emoji = '\u{1F600}';
    // 头部切点（第 3000 个码元）正好落在一个 emoji 的前半；尾部切点落在另一个的后半。
    const body = `${'a'.repeat(2999)}${emoji}${'b'.repeat(1000)}${emoji}${'c'.repeat(799)}`;
    expect(body.length).toBe(4802);
    const result = projectConsoleCatchup([message('user', body)], kimi, '问', { fresh: true, mentions });
    expect(result.text).toContain(`${'a'.repeat(2999)}…[省略 1004 字]…${'c'.repeat(799)}\n\n[当前消息]`);
    expect(/[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/.test(result.text)).toBe(false);
  });

  it('总长超限时从最早的一条起丢，omitted 计数并在各段最前提示', () => {
    // 每段 = 「[用户]\n」5 字 + 3900 字 = 3905；6 段 23430 不超，7 段 27335 超。
    const messages = Array.from({ length: 9 }, (_, index) => message('user', `${index}`.repeat(3900)));
    const result = projectConsoleCatchup(messages, kimi, '问', { fresh: true, mentions });
    expect(result.included).toBe(6);
    expect(result.omitted).toBe(3);
    expect(result.text.startsWith(`${CONSOLE_CATCHUP_PREFIX}\n\n[更早的 3 条对话已省略]\n\n[用户]\n3333`)).toBe(true);
    expect(result.text).not.toContain('2222');
    expect(result.text.endsWith('[当前消息]\n问')).toBe(true);
    expect(consoleCatchupNotice(result, 'Kimi Code')).toBe('已附带此前 6 条对话给 Kimi Code（更早的 3 条已省略）');
  });

  it('开头为 / 的原生命令原样返回', () => {
    expect(projectConsoleCatchup(twoRounds, kimi, '/compact', { fresh: true, mentions }))
      .toEqual({ text: '/compact', included: 0, omitted: 0 });
    expect(projectConsoleCatchup(twoRounds, kimi, '/skill:pdf 读一下', { fresh: false, mentions }).text)
      .toBe('/skill:pdf 读一下');
  });

  it('没有可补内容时 text 等于 outbound，提示为空', () => {
    const empty = projectConsoleCatchup([], kimi, '你好', { fresh: true, mentions });
    expect(empty).toEqual({ text: '你好', included: 0, omitted: 0 });
    expect(consoleCatchupNotice(empty, 'Kimi Code')).toBe('');
    const onlySystem = projectConsoleCatchup([message('system', '请输入要发送的内容。')], kimi, '你好', { fresh: true, mentions });
    expect(onlySystem.text).toBe('你好');
    expect(consoleCatchupNotice({ text: '', included: 4, omitted: 0 }, 'Kimi Code')).toBe('已附带此前 4 条对话给 Kimi Code');
  });
});
