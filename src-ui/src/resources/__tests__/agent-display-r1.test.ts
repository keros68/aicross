import { describe, expect, it } from 'vitest';
import { agentModelDisplay } from '../agent-display';
import { routeLine } from '../../graph/dispatch';
import type { GraphNode } from '../../graph/types';
import dispatchOverlaySource from '../../components/center/graph/DispatchOverlay.tsx?raw';
import nodePanelSource from '../../components/right/orchestration/NodePanel.tsx?raw';
import orchestrationSource from '../../components/right/orchestration/OrchestrationPanel.tsx?raw';
import presetBuilderSource from '../../components/right/orchestration/PresetBuilder.tsx?raw';
import centerPanelSource from '../../components/center/CenterPanel.tsx?raw';
import commanderLauncherSource from '../../components/center/CommanderLauncher.tsx?raw';
import resourcesTabSource from '../../components/resources/ResourcesTab.tsx?raw';
import resourcesOverlaySource from '../../components/resources/ResourcesOverlay.tsx?raw';
import historySource from '../../components/right/HistoryBoard.tsx?raw';
import brandMarksSource from '../../components/resources/brand-marks.ts?raw';
import productSource from '../../../../docs/PRODUCT.md?raw';
import manualSource from '../../../../docs/MANUAL.md?raw';
import readmeSource from '../../../../README.md?raw';
import agentsSource from '../../../../AGENTS.md?raw';
import protoSource from '../../../../docs/proto/index.html?raw';
import rustResourcesSource from '../../../../src/resources.rs?raw';

describe('TASK-15-R1 显示层名称与图标', () => {
  it('G1 数据仍保留机器 id，四个渲染面统一转显示名', () => {
    const node = {
      id: 'n1', kind: 'turn', user: { text: '', attachments: [] }, parents: ['root'], created_at: '',
      target: { agent: 'pi', provider: 'bigmodel', model: 'glm', role: '执行', thinking: '中', visibility: '共享', write: '只读' },
    } satisfies GraphNode;
    expect(routeLine(node, [], '全')).toContain('pi/glm');
    expect(agentModelDisplay('pi', 'glm', 'bigmodel')).toBe('pi agent/bigmodel/glm');
    for (const source of [dispatchOverlaySource, nodePanelSource, orchestrationSource, presetBuilderSource]) {
      expect(source).toContain('agentModelDisplay(');
    }
  });

  it('启动器、左栏资源、接入页、启动卡和历史行都接入 AgentBadge', () => {
    for (const source of [commanderLauncherSource, resourcesTabSource, resourcesOverlaySource, centerPanelSource, historySource]) {
      expect(source).toContain('AgentBadge');
    }
    expect(centerPanelSource).toContain('claude: <AgentBadge agentId="claude"');
    expect(historySource).toContain("['claude', 'codex', 'kimicode', 'pi']");
  });

  it('图标表无 simple-icons 路径数据与 glyph 分支', () => {
    expect(brandMarksSource).not.toContain('BRAND_MARKS');
    expect(brandMarksSource).not.toContain("kind: 'glyph'");
    expect(brandMarksSource).toContain("src: '/icons/tools/claude.svg'");
    expect(brandMarksSource).toContain("src: '/icons/tools/codex.svg'");
    expect(brandMarksSource).toContain("kimicode.png?inline");
    expect(brandMarksSource).toContain("pi.svg?raw");
  });

  it('文档同步四个显示名、Coffee 图标与冷启动直启', () => {
    expect(productSource).toContain('Claude Code / Codex / Kimi Code / pi agent');
    expect(productSource).toContain('Coffee CLI fork 中已有的工具图标');
    expect(productSource).not.toContain('用 π 字形');
    expect(manualSource).toContain('应用冷启动的首个 tab');
    expect(readmeSource).toContain('Kimi Code / pi agent');
    expect(agentsSource).toContain('Kimi Code / pi agent');
    expect(protoSource.slice(0, 200)).toContain('历史快照');
  });

  it('Kimi 探测调用引用统一常量', () => {
    expect(rustResourcesSource).toMatch(/"Moonshot",\s*AGENT_NAME_KIMI,/);
  });
});
