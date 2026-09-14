// 首启引导条：对话图起始视图上的三步提示。判定见 first-run.ts。
// 每步点了跳到对应位置：打开目录（系统选择器）、接入资源（接入页覆盖层）、
// 选预设（右栏「方案与编排」）。

import { firstRunSteps, type FirstRunFacts, type FirstRunStepKey } from './first-run';

interface FirstRunGuideProps {
  facts: FirstRunFacts;
  onStep: (key: FirstRunStepKey) => void;
}

export function FirstRunGuide({ facts, onStep }: FirstRunGuideProps) {
  const steps = firstRunSteps(facts);
  return (
    <div className="graph-firstrun">
      <div className="graph-firstrun-title">先做这三步</div>
      <ol className="graph-firstrun-list">
        {steps.map((step, i) => (
          <li key={step.key}>
            <button
              type="button"
              className={`graph-firstrun-step${step.done ? ' done' : ''}`}
              onClick={() => onStep(step.key)}
            >
              <span className="graph-firstrun-ord" aria-hidden="true">
                {i + 1}
              </span>
              <span className="graph-firstrun-body">
                <span className="graph-firstrun-action">
                  {step.action}
                  {step.done && <span className="graph-firstrun-done">已完成</span>}
                </span>
                <span className="graph-firstrun-hint">{step.hint}</span>
              </span>
            </button>
          </li>
        ))}
      </ol>
    </div>
  );
}
