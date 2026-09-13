import { Children, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { getTaskNewHandler, TaskEmptyState } from '../components/right/TaskEmptyState';

describe('TaskEmptyState', () => {
  it('renders the empty message and forwards the new action', () => {
    const onNew = vi.fn();
    const element = TaskEmptyState({ onNew });
    const markup = renderToStaticMarkup(element);
    const button = Children.toArray(element.props.children)
      .find(child => typeof child === 'object' && child !== null && 'type' in child && child.type === 'button') as ReactElement<{ onClick: () => void }>;

    expect(markup).toContain('还没有任务');
    expect(markup).toContain('新建');
    button.props.onClick();
    expect(onNew).toHaveBeenCalledOnce();
  });

  it('connects list and note to task creation, and prompt to prompt creation', () => {
    const onAdd = vi.fn();
    const onAddPrompt = vi.fn();

    getTaskNewHandler('list', onAdd, onAddPrompt)();
    getTaskNewHandler('note', onAdd, onAddPrompt)();
    getTaskNewHandler('prompt', onAdd, onAddPrompt)();

    expect(onAdd).toHaveBeenCalledTimes(2);
    expect(onAddPrompt).toHaveBeenCalledOnce();
  });
});
