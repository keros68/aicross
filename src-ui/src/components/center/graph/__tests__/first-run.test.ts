import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { firstRunSteps, shouldShowFirstRun, type FirstRunFacts } from '../first-run';
import { FirstRunGuide } from '../FirstRunGuide';

const facts = (over: Partial<FirstRunFacts> = {}): FirstRunFacts => ({
  graphCount: 0,
  resourceCount: 0,
  folderPath: null,
  ...over,
});

describe('shouldShowFirstRun', () => {
  it('shows only when there is no graph and no accepted resource', () => {
    expect(shouldShowFirstRun(facts())).toBe(true);
  });

  it('hides once a graph exists', () => {
    expect(shouldShowFirstRun(facts({ graphCount: 1 }))).toBe(false);
  });

  it('hides once a resource is accepted', () => {
    expect(shouldShowFirstRun(facts({ resourceCount: 3 }))).toBe(false);
  });

  it('hides while either fact is still unknown', () => {
    expect(shouldShowFirstRun(facts({ graphCount: null }))).toBe(false);
    expect(shouldShowFirstRun(facts({ resourceCount: null }))).toBe(false);
    expect(shouldShowFirstRun(facts({ graphCount: null, resourceCount: null }))).toBe(false);
  });

  it('does not depend on the working folder', () => {
    expect(shouldShowFirstRun(facts({ folderPath: 'D:/work' }))).toBe(true);
    expect(shouldShowFirstRun(facts({ graphCount: 2, folderPath: null }))).toBe(false);
  });
});

describe('firstRunSteps', () => {
  it('lists the three steps in order', () => {
    expect(firstRunSteps(facts()).map((s) => s.key)).toEqual(['folder', 'resources', 'preset']);
    expect(firstRunSteps(facts()).map((s) => s.action)).toEqual(['打开目录', '接入资源', '选预设']);
  });

  it('marks a step done from the matching fact', () => {
    const done = firstRunSteps(facts({ folderPath: 'D:/work', resourceCount: 2 }));
    expect(done[0].done).toBe(true);
    expect(done[1].done).toBe(true);
    expect(done[2].done).toBe(false);
    const fresh = firstRunSteps(facts());
    expect(fresh.every((s) => !s.done)).toBe(true);
  });

  it('gives every step exactly one line of guidance', () => {
    for (const step of firstRunSteps(facts())) {
      expect(step.hint).not.toContain('\n');
      expect(step.hint.length).toBeGreaterThan(0);
    }
  });
});

describe('FirstRunGuide', () => {
  it('renders every step as a button', () => {
    const markup = renderToStaticMarkup(FirstRunGuide({ facts: facts(), onStep: () => {} }));
    expect(markup).toContain('打开目录');
    expect(markup).toContain('接入资源');
    expect(markup).toContain('选预设');
    expect(markup.match(/<button/g)?.length).toBe(3);
    expect(markup).not.toContain('已完成');
  });

  it('labels a completed step', () => {
    const markup = renderToStaticMarkup(
      FirstRunGuide({ facts: facts({ folderPath: 'D:/work' }), onStep: () => {} }),
    );
    expect(markup).toContain('已完成');
  });
});
