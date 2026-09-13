interface TaskEmptyStateProps {
  onNew?: () => void;
}

export type TaskEmptyView = 'list' | 'note' | 'prompt';

export function getTaskNewHandler(
  view: TaskEmptyView,
  onAdd: () => void,
  onAddPrompt: () => void,
): () => void {
  return view === 'prompt' ? onAddPrompt : onAdd;
}

export function TaskEmptyState({ onNew }: TaskEmptyStateProps) {
  return (
    <div className="task-empty">
      <div className="task-empty-text">还没有任务</div>
      {onNew && (
        <button type="button" className="task-empty-new" onClick={onNew}>
          新建
        </button>
      )}
    </div>
  );
}
