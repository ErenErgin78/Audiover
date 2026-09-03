import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, getBackendKind, type Diagnostics, type LogEntry } from "../hooks/useApi";
import { useI18n } from "../hooks/useI18n";

const POLL_MS = 1000;
const MAX_ROWS = 400;

const LEVEL_COLORS: Record<string, string> = {
  ERROR: "var(--red)",
  WARN: "var(--yellow)",
  INFO: "var(--accent)",
  DEBUG: "var(--text-muted)",
  TRACE: "var(--text-dim)",
};

function formatTime(tsMs: number): string {
  const d = new Date(tsMs);
  const p = (n: number, w = 2) => String(n).padStart(w, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`;
}

async function probeDevServer(timeoutMs = 2500): Promise<boolean> {
  try {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), timeoutMs);
    // Same document is enough: a no-cors request distinguishes
    // "server up" (opaque response) from "connection refused" (throws).
    await fetch("http://127.0.0.1:5173/", { mode: "no-cors", signal: ctrl.signal });
    clearTimeout(timer);
    return true;
  } catch {
    return false;
  }
}

export default function LogsPage() {
  const { t } = useI18n();
  const [backendKind] = useState(getBackendKind());
  const [devServerUp, setDevServerUp] = useState<boolean | null>(null);
  const [diagnostics, setDiagnostics] = useState<Diagnostics | null>(null);
  const [diagError, setDiagError] = useState<string | null>(null);
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [levelFilter, setLevelFilter] = useState<string>("ALL");
  const [paused, setPaused] = useState(false);
  const [copied, setCopied] = useState(false);
  const lastSeq = useRef<number | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  const refreshDiagnostics = useCallback(async () => {
    try {
      const d = await api.getDiagnostics();
      setDiagnostics(d);
      setDiagError(null);
    } catch (e) {
      setDiagError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    probeDevServer().then(setDevServerUp);
    refreshDiagnostics();
  }, [refreshDiagnostics]);

  // Live tail of the backend log ring buffer.
  useEffect(() => {
    let cancelled = false;
    const tick = async () => {
      if (pausedRef.current || backendKind === "none") return;
      try {
        const batch = await api.getLogs(lastSeq.current);
        if (cancelled || batch.length === 0) return;
        lastSeq.current = batch[batch.length - 1].seq;
        setEntries((prev) => [...prev, ...batch].slice(-MAX_ROWS));
      } catch {
        // Backend temporarily unreachable: keep old rows, keep polling.
      }
    };
    tick();
    const id = setInterval(tick, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [backendKind]);

  // Autoscroll on new rows (unless paused).
  useEffect(() => {
    const el = scrollRef.current;
    if (el && !paused) el.scrollTop = el.scrollHeight;
  }, [entries, paused]);

  const visible = useMemo(
    () => (levelFilter === "ALL" ? entries : entries.filter((e) => e.level === levelFilter)),
    [entries, levelFilter]
  );

  const copyAll = async () => {
    const text = visible.map((e) => `[${formatTime(e.ts_ms)} ${e.level} ${e.target}] ${e.message}`).join("\n");
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const clearAll = async () => {
    try {
      await api.clearLogs();
    } catch {
      // Fall through to local clear even if the backend call fails.
    }
    lastSeq.current = null;
    setEntries([]);
  };

  const copyDump = async () => {
    if (!diagnostics) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(diagnostics, null, 2));
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // clipboard unavailable: no-op
    }
  };

  const cardStyle: React.CSSProperties = {
    background: "var(--bg-card)",
    border: "1px solid var(--border)",
  };
  const labelStyle: React.CSSProperties = { color: "var(--text-muted)", fontSize: 12 };
  const valueStyle: React.CSSProperties = { color: "var(--text)", fontSize: 12, fontWeight: 600 };

  const backendLabel =
    backendKind === "tauri"
      ? t.logs.backendTauri
      : backendKind === "pywebview"
        ? t.logs.backendPyWebView
        : t.logs.backendNone;

  return (
    <div className="flex flex-col gap-4 p-6 overflow-y-auto max-w-5xl">
      <h1 style={{ color: "var(--accent)", fontWeight: 900, fontSize: 20, letterSpacing: 1 }}>
        {t.logs.title}
      </h1>

      {/* ── Connection status ─────────────────────────────── */}
      <section className="rounded-xl p-5 flex flex-col gap-2" style={cardStyle}>
        <h2 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 14, margin: 0 }}>
          {t.logs.statusCard}
        </h2>
        <div className="grid grid-cols-2 gap-x-6 gap-y-1.5" style={{ maxWidth: 640 }}>
          <span style={labelStyle}>{t.logs.backendRow}</span>
          <span style={valueStyle}>{backendLabel}</span>

          <span style={labelStyle}>{t.app.devServerLabel}</span>
          <span style={{ ...valueStyle, color: devServerUp ? "var(--green)" : "var(--red)" }}>
            {devServerUp === null
              ? "…"
              : devServerUp
                ? t.app.devServerReachable
                : t.app.devServerUnreachable}
          </span>

          <span style={labelStyle}>{t.logs.engineRow}</span>
          <span style={{
            ...valueStyle,
            color: diagnostics ? (diagnostics.engine_active ? "var(--green)" : "var(--yellow)") : "var(--text-muted)",
          }}>
            {diagnostics ? (diagnostics.engine_active ? t.logs.engineOn : t.logs.engineOff) : "…"}
          </span>

          {diagnostics && (
            <>
              <span style={labelStyle}>{t.logs.virtualSinkRow}</span>
              <span style={{ ...valueStyle, color: diagnostics.virtual_sink_found ? "var(--green)" : "var(--red)" }}>
                {diagnostics.virtual_sink_found ? t.logs.found : t.logs.missing}
              </span>
              <span style={labelStyle}>{t.logs.pactlRow}</span>
              <span style={{ ...valueStyle, color: diagnostics.pactl_available ? "var(--green)" : "var(--red)" }}>
                {diagnostics.pactl_available ? t.logs.found : t.logs.missing}
              </span>
              <span style={labelStyle}>{t.logs.devicesRow}</span>
              <span style={valueStyle}>{diagnostics.input_count} / {diagnostics.output_count}</span>
              <span style={labelStyle}>{t.logs.hotkeyRow}</span>
              <span style={valueStyle}>{diagnostics.hotkey_backend}</span>
            </>
          )}
        </div>
        {backendKind === "pywebview" && (
          <p style={{ color: "var(--yellow)", fontSize: 12, margin: "4px 0 0" }}>
            {t.logs.pywebviewUnsupported}
          </p>
        )}
        {diagError && (
          <p style={{ color: "var(--red)", fontSize: 12, margin: "4px 0 0" }}>{diagError}</p>
        )}
      </section>

      {/* ── Live log viewer ───────────────────────────────── */}
      <section className="rounded-xl p-5 flex flex-col gap-3" style={cardStyle}>
        <div className="flex items-center justify-between flex-wrap gap-2">
          <h2 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 14, margin: 0 }}>
            {t.logs.viewerCard}
          </h2>
          <div className="flex items-center gap-2">
            <select
              value={levelFilter}
              onChange={(e) => setLevelFilter(e.target.value)}
              style={{ fontSize: 12 }}
            >
              <option value="ALL">{t.logs.levelAll}</option>
              <option value="ERROR">ERROR</option>
              <option value="WARN">WARN</option>
              <option value="INFO">INFO</option>
              <option value="DEBUG">DEBUG</option>
              <option value="TRACE">TRACE</option>
            </select>
            <button
              onClick={() => setPaused((p) => !p)}
              className="px-3 py-1 rounded-lg text-xs font-bold"
              style={{ background: "var(--bg-surface)", border: "1px solid var(--border-hover)", color: "var(--text)" }}
            >
              {paused ? t.logs.resume : t.logs.pause}
            </button>
            <button
              onClick={copyAll}
              className="px-3 py-1 rounded-lg text-xs font-bold"
              style={{ background: "var(--bg-surface)", border: "1px solid var(--border-hover)", color: "var(--text)" }}
            >
              {copied ? t.logs.copied : t.logs.copy}
            </button>
            <button
              onClick={clearAll}
              className="px-3 py-1 rounded-lg text-xs font-bold"
              style={{ background: "var(--bg-surface)", border: "1px solid var(--border-hover)", color: "var(--red)" }}
            >
              {t.logs.clear}
            </button>
          </div>
        </div>
        <div
          ref={scrollRef}
          className="rounded-lg px-3 py-2 font-mono overflow-y-auto"
          style={{ background: "#0B0D14", border: "1px solid var(--border)", height: 320, fontSize: 11.5, lineHeight: 1.5 }}
        >
          {visible.length === 0 ? (
            <span style={{ color: "var(--text-muted)" }}>{t.logs.empty}</span>
          ) : (
            visible.map((e) => (
              <div key={e.seq} style={{ whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
                <span style={{ color: "var(--text-dim)" }}>[{formatTime(e.ts_ms)}]</span>{" "}
                <span style={{ color: LEVEL_COLORS[e.level] ?? "var(--text)", fontWeight: 700 }}>{e.level}</span>{" "}
                <span style={{ color: "var(--accent2)" }}>{e.target}</span>{" "}
                <span style={{ color: "var(--text)" }}>{e.message}</span>
              </div>
            ))
          )}
        </div>
      </section>

      {/* ── Diagnostics dump ──────────────────────────────── */}
      <section className="rounded-xl p-5 flex flex-col gap-3" style={cardStyle}>
        <div className="flex items-center justify-between">
          <h2 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 14, margin: 0 }}>
            {t.logs.diagCard}
          </h2>
          <div className="flex items-center gap-2">
            <button
              onClick={refreshDiagnostics}
              className="px-3 py-1 rounded-lg text-xs font-bold"
              style={{ background: "var(--bg-surface)", border: "1px solid var(--border-hover)", color: "var(--text)" }}
            >
              {t.app.retryButton}
            </button>
            <button
              onClick={copyDump}
              disabled={!diagnostics}
              className="px-3 py-1 rounded-lg text-xs font-bold"
              style={{ background: "var(--bg-surface)", border: "1px solid var(--border-hover)", color: "var(--text)" }}
            >
              {t.logs.diagCopy}
            </button>
          </div>
        </div>
        <pre
          className="rounded-lg px-3 py-2 overflow-auto font-mono"
          style={{ background: "#0B0D14", border: "1px solid var(--border)", fontSize: 11.5, maxHeight: 280, color: "var(--text)" }}
        >
          {diagnostics ? JSON.stringify(diagnostics, null, 2) : diagError ?? "…"}
        </pre>
      </section>
    </div>
  );
}
