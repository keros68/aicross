import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import graphTabSource from '../../components/center/graph/GraphTab.tsx?raw';
import orchestrationPanelSource from '../../components/right/orchestration/OrchestrationPanel.tsx?raw';
import manual from '../../../../docs/MANUAL.md?raw';
import { ExplainContent } from '../../components/right/orchestration/ExplainOverlay.tsx';
import { PresetGroups, PresetSelect } from '../../components/right/orchestration/PresetGroups.tsx';
import { presetGroupOf } from '../label.ts';
import type { Preset } from '../types.ts';

const preset = (id: string, name: string, builtin: boolean): Preset => ({
  id,
  name,
  builtin,
  stages: [],
});

const PRESETS = [
  preset('builtin-jury', '评审模式', true),
  preset('custom-a', '团队流水线', false),
  preset('builtin-blind', '盲审模式', true),
  preset('builtin-swarm', '蜂群模式', true),
  preset('builtin-shared', '共享模式', true),
  preset('builtin-arena', '擂台模式', true),
  preset('builtin-economy', '经济模式', true),
];

describe('TASK-18 模式做减法', () => {
  it('presetGroupOf 按内置 id 与 builtin 分成基础、自定义、更多三类', () => {
    expect(presetGroupOf(PRESETS[4])).toBe('基础模式');
    expect(presetGroupOf(PRESETS[1])).toBe('自定义模式');
    expect(presetGroupOf(PRESETS[0])).toBe('更多模式');
    expect(presetGroupOf(preset('builtin-economy', '同名自定义', false))).toBe('自定义模式');
  });

  it('右栏下拉按基础、自定义、更多排序，默认收起更多模式且经济模式仍为当前项', () => {
    const html = renderToStaticMarkup(
      <PresetSelect presets={PRESETS} selectedId="builtin-economy" onSelect={() => {}} />,
    );
    expect(html).toContain('经济模式');
    expect(html.indexOf('基础模式')).toBeLessThan(html.indexOf('自定义模式'));
    expect(html.indexOf('自定义模式')).toBeLessThan(html.indexOf('更多模式'));
    expect(html).toMatch(/<details class="preset-more">/);
    expect(html).not.toMatch(/<details class="preset-more" open=""/);
    expect(orchestrationPanelSource).toContain("p.id === 'builtin-economy'");
    expect(orchestrationPanelSource).toContain('return (economy ?? all[0])?.id');
  });

  it('分组菜单固定基础模式顺序，自定义居中，更多模式末尾含置灰规划项', () => {
    const html = renderToStaticMarkup(
      <PresetGroups presets={PRESETS} onSelect={() => {}} includeFuture />,
    );
    const names = ['共享模式', '经济模式', '盲审模式', '自定义模式', '擂台模式', '评审模式', '蜂群模式', '红蓝模式（规划中）', '群聊模式（规划中）'];
    for (let i = 1; i < names.length; i += 1) {
      expect(html.indexOf(names[i - 1])).toBeLessThan(html.indexOf(names[i]));
    }
    expect(html).toMatch(/disabled=""[^>]*>红蓝模式（规划中）<\/button>/);
    expect(html).toMatch(/disabled=""[^>]*>群聊模式（规划中）<\/button>/);
  });

  it('图上盖预设菜单复用同一分组组件', () => {
    expect(graphTabSource).toContain('<PresetGroups');
    expect(graphTabSource).toContain('className="graph-stamp-menu"');
  });

  it('讲解页先逐项说明三种基础模式，再单列更多模式', () => {
    const html = renderToStaticMarkup(<ExplainContent />);
    const basicStart = html.indexOf('<h2>基础模式</h2>');
    const moreStart = html.indexOf('<h2>更多模式</h2>');
    expect(basicStart).toBeLessThan(moreStart);
    const basic = html.slice(basicStart, moreStart);
    const more = html.slice(moreStart, html.indexOf('</section>', moreStart));
    for (const group of [
      { html: basic, names: ['共享模式', '经济模式', '盲审模式'] },
      { html: more, names: ['擂台模式', '评审模式', '蜂群模式'] },
    ]) {
      for (let i = 1; i < group.names.length; i += 1) {
        expect(group.html.indexOf(`<strong>${group.names[i - 1]}</strong>`))
          .toBeLessThan(group.html.indexOf(`<strong>${group.names[i]}</strong>`));
      }
    }
  });

  it('手册的内置预设按基础、自定义、更多三层说明', () => {
    const start = manual.indexOf('## 内置预设');
    const end = manual.indexOf('## 蜂群模式', start);
    const section = manual.slice(start, end);
    expect(section.indexOf('### 基础模式')).toBeLessThan(section.indexOf('### 自定义模式'));
    expect(section.indexOf('### 自定义模式')).toBeLessThan(section.indexOf('### 更多模式'));
  });
});
