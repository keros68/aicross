export const TOOLS_INSTALLED_EVENT = 'aicross:tools-installed';

export interface ToolScanSnapshot {
  installed: Record<string, boolean>;
  scanned: boolean;
}

let snapshot: ToolScanSnapshot = { installed: {}, scanned: false };

export function getToolScanSnapshot(): ToolScanSnapshot {
  return snapshot;
}

export function publishToolScan(installed: Record<string, boolean>): void {
  snapshot = { installed, scanned: true };
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent<ToolScanSnapshot>(TOOLS_INSTALLED_EVENT, { detail: snapshot }));
  }
}
