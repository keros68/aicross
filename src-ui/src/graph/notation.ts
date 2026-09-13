import type {
  DispatchableModel,
  Preset,
  Stage,
  Target,
  Thinking,
  VisibilityKind,
  WriteMode,
} from './types.ts';
import { defaultThinking, newNodeId, roleDefaults } from './types.ts';

export interface ParseResult {
  preset?: Preset;
  error?: { pos: number; message: string };
  unresolved?: string[];
}

interface Suffixes {
  role?: string;
  thinking?: Thinking;
  visibility?: VisibilityKind;
  write?: WriteMode;
}

class ParseError extends Error {
  pos: number;
  constructor(pos: number, message: string) {
    super(message);
    this.pos = pos;
  }
}

const THINKING_FROM_TOKEN: Record<string, Thinking> = {
  off: '关',
  low: '低',
  mid: '中',
  high: '高',
};
const THINKING_TO_TOKEN: Record<Thinking, string> = {
  关: 'off',
  低: 'low',
  中: 'mid',
  高: 'high',
};
const VISIBILITY_TO_TOKEN: Record<VisibilityKind, string> = {
  共享: 'shared',
  盲: 'blind',
  指定上游: 'upstream',
};
const WRITE_TO_TOKEN: Record<WriteMode, string> = {
  只读: 'ro',
  单写者: 'rw',
  worktree: 'wt',
};

function isSpace(ch: string | undefined): boolean {
  return ch !== undefined && /\s/.test(ch);
}

// 词元终止符：空白、后缀起始符、分组符、阶段箭头
function isTokenStop(line: string, i: number): boolean {
  const ch = line[i];
  if (ch === undefined) return true;
  if (isSpace(ch)) return true;
  if ('@#!:|[]→'.includes(ch)) return true;
  if (ch === '-' && line[i + 1] === '>') return true;
  return false;
}

export function parseNotation(line: string, models: DispatchableModel[]): ParseResult {
  const unresolved: string[] = [];
  try {
    const stages = parseStages(line, models, unresolved);
    const preset: Preset = { id: newNodeId(), name: '', builtin: false, stages };
    return unresolved.length > 0 ? { preset, unresolved } : { preset };
  } catch (e) {
    if (e instanceof ParseError) {
      return { error: { pos: e.pos, message: e.message }, ...(unresolved.length > 0 ? { unresolved } : {}) };
    }
    throw e;
  }
}

// 在 bracket 深度 0 处按 → / -> 切分阶段，返回 [起点, 终点) 区间
function splitStages(line: string): Array<[number, number]> {
  const spans: Array<[number, number]> = [];
  let depth = 0;
  let start = 0;
  let i = 0;
  while (i < line.length) {
    const ch = line[i];
    if (ch === '[') {
      if (depth > 0) throw new ParseError(i, '不支持嵌套括号');
      depth = 1;
      i++;
    } else if (ch === ']') {
      if (depth === 0) throw new ParseError(i, "多余的 ']'");
      depth = 0;
      i++;
    } else if (depth === 0 && (ch === '→' || (ch === '-' && line[i + 1] === '>'))) {
      spans.push([start, i]);
      i += ch === '→' ? 1 : 2;
      start = i;
    } else {
      i++;
    }
  }
  if (depth > 0) throw new ParseError(line.indexOf('['), "缺少 ']'");
  spans.push([start, line.length]);
  return spans;
}

function parseStages(line: string, models: DispatchableModel[], unresolved: string[]): Stage[] {
  if (line.trim() === '') throw new ParseError(0, '内容为空');
  const spans = splitStages(line);
  return spans.map(([s, e]) => parseStage(line, s, e, models, unresolved));
}

function skipWs(line: string, i: number, end: number): number {
  while (i < end && isSpace(line[i])) i++;
  return i;
}

function parseStage(
  line: string,
  start: number,
  end: number,
  models: DispatchableModel[],
  unresolved: string[],
): Stage {
  let i = skipWs(line, start, end);
  if (i >= end) throw new ParseError(start, '阶段为空');

  if (line[i] === '[') {
    const close = line.indexOf(']', i + 1);
    if (close === -1 || close >= end) throw new ParseError(i, "缺少 ']'");
    const items: ParsedItem[] = [];
    let itemStart = i + 1;
    for (let j = i + 1; j <= close; j++) {
      if (j === close || line[j] === '|') {
        items.push(parseItem(line, itemStart, j, models, unresolved));
        itemStart = j + 1;
      }
    }
    i = skipWs(line, close + 1, end);
    const [groupSuffixes, next] = parseSuffixes(line, i, end);
    i = skipWs(line, next, end);
    if (i < end) throw new ParseError(i, `多余内容 '${line.slice(i, end).trim()}'`);
    // 组后缀作为组内每个节点的缺省，节点自己的后缀优先
    return { nodes: items.map((item) => finalizeTarget(item, groupSuffixes)) };
  }

  const item = parseItem(line, i, end, models, unresolved);
  return { nodes: [finalizeTarget(item, {})] };
}

interface ParsedItem {
  agent: string;
  model: string;
  own: Suffixes; // 节点自己显式写的后缀
}

// 解析单个节点：模型词元 + 自身后缀
function parseItem(
  line: string,
  start: number,
  end: number,
  models: DispatchableModel[],
  unresolved: string[],
): ParsedItem {
  let i = skipWs(line, start, end);
  const tokenStart = i;
  while (i < end && !isTokenStop(line, i)) i++;
  const ref = line.slice(tokenStart, i);
  if (ref === '') throw new ParseError(start, '缺少模型名');
  const [own, next] = parseSuffixes(line, i, end);
  i = skipWs(line, next, end);
  if (i < end) throw new ParseError(i, `多余内容 '${line.slice(i, end).trim()}'`);
  const { agent, model } = resolveModel(ref, tokenStart, models, unresolved);
  return { agent, model, own };
}

// 合成最终 Target：节点后缀 > 组后缀 > 缺省（role 缺省 '执行'，thinking 缺省 '中'，
// visibility/write 缺省取 roleDefaults(role)）
function finalizeTarget(item: ParsedItem, group: Suffixes): Target {
  const own = item.own;
  const role = own.role ?? group.role ?? '执行';
  const defs = roleDefaults(role);
  return {
    agent: item.agent,
    model: item.model,
    role,
    thinking: own.thinking ?? group.thinking ?? defaultThinking(),
    visibility: own.visibility ?? group.visibility ?? defs.visibility,
    write: own.write ?? group.write ?? defs.write,
  };
}

function parseSuffixes(line: string, start: number, end: number): [Suffixes, number] {
  const out: Suffixes = {};
  let i = start;
  for (;;) {
    i = skipWs(line, i, end);
    const ch = line[i];
    if (ch === '@' || ch === '#' || ch === ':') {
      const pos = i;
      i++;
      const wordStart = i;
      while (i < end && /[a-z0-9]/.test(line[i])) i++;
      const word = line.slice(wordStart, i);
      if (ch === '@') {
        if (word === 'blind') out.visibility = '盲';
        else if (word === 'shared') out.visibility = '共享';
        else if (word === 'upstream') out.visibility = '指定上游';
        else throw new ParseError(pos, `未知可见性后缀 '@${word}'，可选 blind/shared/upstream`);
      } else if (ch === '#') {
        const t = THINKING_FROM_TOKEN[word];
        if (!t) throw new ParseError(pos, `未知思考后缀 '#${word}'，可选 off/low/mid/high`);
        out.thinking = t;
      } else {
        if (word === 'ro') out.write = '只读';
        else if (word === 'rw') out.write = '单写者';
        else if (word === 'wt') out.write = 'worktree';
        else throw new ParseError(pos, `未知写权限后缀 ':${word}'，可选 ro/rw/wt`);
      }
    } else if (ch === '!') {
      i++;
      const roleStart = i;
      while (i < end && !isTokenStop(line, i)) i++;
      const role = line.slice(roleStart, i);
      if (role === '') throw new ParseError(i - 1, "'!' 后缺少角色名");
      out.role = role;
    } else {
      break;
    }
  }
  return [out, i];
}

function resolveModel(
  ref: string,
  pos: number,
  models: DispatchableModel[],
  unresolved: string[],
): { agent: string; model: string } {
  if (ref === '?') return { agent: '', model: '' };
  const slash = ref.indexOf('/');
  if (slash >= 0) {
    const agent = ref.slice(0, slash);
    const model = ref.slice(slash + 1);
    const hit = models.find((m) => m.agent === agent && m.id === model);
    if (!hit && !unresolved.includes(ref)) unresolved.push(ref);
    return hit ? { agent: hit.agent, model: hit.id } : { agent, model };
  }
  const candidates = models.filter((m) => m.id === ref || `${m.agent}/${m.id}` === ref);
  if (candidates.length > 1) {
    const list = candidates.map((c) => `${c.agent}/${c.id}`).join('、');
    throw new ParseError(pos, `模型 '${ref}' 匹配不唯一，候选：${list}，请写成 agent/model`);
  }
  if (candidates.length === 1) return { agent: candidates[0].agent, model: candidates[0].id };
  if (!unresolved.includes(ref)) unresolved.push(ref);
  return { agent: '', model: ref };
}

function printNode(target: Target): string {
  const modelRef = target.agent !== '' ? `${target.agent}/${target.model}` : target.model !== '' ? target.model : '?';
  return (
    modelRef +
    `!${target.role}` +
    `#${THINKING_TO_TOKEN[target.thinking]}` +
    `@${VISIBILITY_TO_TOKEN[target.visibility]}` +
    `:${WRITE_TO_TOKEN[target.write]}`
  );
}

// 正则形式：每个节点都显式打印全部后缀，单节点阶段不加括号
function printText(preset: Preset): string {
  return preset.stages
    .map((stage) =>
      stage.nodes.length === 1
        ? printNode(stage.nodes[0])
        : `[${stage.nodes.map(printNode).join(' | ')}]`,
    )
    .join(' → ');
}

export interface NotationPrint {
  text: string;
  // 一行记法不表达的字段（09-13 修正轮 R1 定：记法只表达 agent/model、角色、
  // thinking、可见性种类、写权限）：'provider' | '档位' | '指定上游'，去重。
  // 指定上游的节点 id 列表永不进记法，故 visibility=指定上游 一律计入（即使 upstreams 为空）。
  lossy: string[];
}

export function printNotation(preset: Preset): NotationPrint {
  const lossy = new Set<string>();
  for (const stage of preset.stages) {
    for (const t of stage.nodes) {
      if (t.provider !== undefined && t.provider !== '') lossy.add('provider');
      if (t.tier !== undefined && t.tier !== '') lossy.add('档位');
      if (t.visibility === '指定上游') lossy.add('指定上游');
    }
  }
  return { text: printText(preset), lossy: [...lossy] };
}
