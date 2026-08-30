import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { api } from "../api";
import type { TerminalTab } from "../types";

type OutputPayload = { terminalId: string; data: string };
type ExitPayload = { terminalId: string; code: number | null };

export function TerminalView({
  tab,
  active,
  onExit,
  onError,
}: {
  tab: TerminalTab;
  active: boolean;
  onExit: (id: string) => void;
  onError: (message: string) => void;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const fitRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    const terminal = new Terminal({
      cursorBlink: true,
      convertEol: false,
      fontFamily: '"Cascadia Mono", "IBM Plex Mono", Consolas, monospace',
      fontSize: 13,
      scrollback: 5000,
      theme: terminalTheme(),
    });
    const fit = new FitAddon();
    fitRef.current = fit;
    terminal.loadAddon(fit);
    terminal.open(hostRef.current!);
    fit.fit();

    let disposed = false;
    const cleanupListeners: Array<() => void> = [];
    Promise.all([
      listen<OutputPayload>("pty-output", ({ payload }) => {
        if (payload.terminalId !== tab.id) return;
        const binary = atob(payload.data);
        const bytes = Uint8Array.from(binary, (value) => value.charCodeAt(0));
        terminal.write(bytes);
      }),
      listen<ExitPayload>("pty-exit", ({ payload }) => {
        if (payload.terminalId === tab.id) onExit(tab.id);
      }),
    ])
      .then(async (unlisteners) => {
        if (disposed) {
          unlisteners.forEach((unlisten) => unlisten());
          return;
        }
        cleanupListeners.push(...unlisteners);
        try {
          await api.openTerminal({
            terminalId: tab.id,
            toolId: tab.toolId,
            rows: terminal.rows,
            cols: terminal.cols,
            env: tab.env,
          });
          terminal.focus();
        } catch (error) {
          onError(String(error));
          onExit(tab.id);
        }
      })
      .catch((error) => onError(String(error)));

    const dataSubscription = terminal.onData((data) => {
      void api.writeTerminal(tab.id, data).catch(() => onExit(tab.id));
    });
    const resizeSubscription = terminal.onResize(({ rows, cols }) => {
      void api.resizeTerminal(tab.id, rows, cols).catch(() => undefined);
    });
    const observer = new ResizeObserver(() => {
      if (!hostRef.current || hostRef.current.offsetParent === null) return;
      fit.fit();
    });
    observer.observe(hostRef.current!);

    return () => {
      disposed = true;
      cleanupListeners.forEach((unlisten) => unlisten());
      observer.disconnect();
      dataSubscription.dispose();
      resizeSubscription.dispose();
      terminal.dispose();
    };
  }, [onError, onExit, tab.env, tab.id, tab.toolId]);

  useEffect(() => {
    if (active) requestAnimationFrame(() => fitRef.current?.fit());
  }, [active]);

  return <div className="terminal-host" ref={hostRef} hidden={!active} />;
}

function terminalTheme() {
  const dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  return dark
    ? { background: "#14161c", foreground: "#e8e9ed", cursor: "#86a6e6" }
    : { background: "#171a21", foreground: "#eef0f3", cursor: "#9fb9ee" };
}

