import type { DispatchableModel, Graph, Target } from './types.ts';
import { ancestorsOf } from './layout.ts';

// 在已接入列表里查 Target 对应的模型（agent 留空时只按 model 匹配）。
// 界面各处「查厂商色 / 判未接入」统一用这一个。
export function findModel(
  models: DispatchableModel[],
  target: Target,
): DispatchableModel | undefined {
  return models.find(
    (m) => m.id === target.model && (target.agent === '' || m.agent === target.agent),
  );
}

// 校验提示（只标注，不阻止），返回中文提示数组
export function validateNode(
  graph: Graph,
  nodeId: string,
  models: DispatchableModel[],
): string[] {
  const node = graph.nodes[nodeId];
  if (!node) return [];
  const hints: string[] = [];
  const target = node.target;

  if (target && (target.role === '验收' || target.role === '验证') && target.write !== '只读') {
    hints.push('验收/验证节点只能只读');
  }
  if (target && target.visibility === '盲' && target.write !== '只读') {
    hints.push('盲节点只能只读');
  }

  if (target && (target.role === '验收' || target.role === '验证')) {
    const own = findModel(models, target);
    if (own) {
      const sameVendor = ancestorsOf(graph, nodeId).some((aid) => {
        const at = graph.nodes[aid].target;
        if (!at || (at.role !== '执行' && at.role !== '出方案')) return false;
        const am = findModel(models, at);
        return am !== undefined && am.vendor === own.vendor;
      });
      if (sameVendor) hints.push('复核，非独立验证');
    }
  }

  if (target && target.model !== '' && !findModel(models, target)) {
    hints.push('未接入');
  }

  if (node.kind === 'accept' && node.parents.length < 2) {
    hints.push('扇入不足');
  }

  return hints;
}
