export function stampMenuShouldClose(event: KeyboardEvent | MouseEvent): boolean {
  if (event.type === 'keydown') return (event as KeyboardEvent).key === 'Escape';
  const target = event.target as { closest?: (selector: string) => unknown } | null;
  return !target?.closest?.('.graph-stamp-menu, .graph-stamp-trigger');
}
