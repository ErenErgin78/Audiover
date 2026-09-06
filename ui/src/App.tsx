import { useEffect, useState } from "react";
import { useAudioStore } from "./store/audioStore";
import { api, getBackendKind } from "./hooks/useApi";
import { useMeter } from "./hooks/useMeter";
import { useInWindowHotkeys } from "./hooks/useInWindowHotkeys";
import { useI18n } from "./hooks/useI18n";
import Layout from "./components/layout/Layout";
import VoicePage from "./pages/VoicePage";
import SoundboardPage from "./pages/SoundboardPage";
import SettingsPage from "./pages/SettingsPage";

function ActivePage() {
  const page = useAudioStore((s) => s.activePage);
  switch (page) {
    case "voice":      return <VoicePage />;
    case "soundboard": return <SoundboardPage />;
    case "settings":   return <SettingsPage />;
  }
}

async function probeDevServer(timeoutMs = 2500): Promise<boolean> {
  try {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), timeoutMs);
    await fetch("http://127.0.0.1:5173/", { mode: "no-cors", signal: ctrl.signal });
    clearTimeout(timer);
    return true;
  } catch {
    return false;
  }
}

function ConnectionFailed({ error, onRetry }: { error: string; onRetry: () => void }) {
  const { t } = useI18n();
  const [devServerUp, setDevServerUp] = useState<boolean | null>(null);

  useEffect(() => {
    probeDevServer().then(setDevServerUp);
  }, []);

  return (
    <div
      className="flex items-center justify-center h-screen p-6"
      style={{ background: "var(--bg-base)", color: "var(--text)" }}
    >
      <div
        className="rounded-xl p-6 flex flex-col gap-3"
        style={{
          background: "var(--bg-card)",
          border: "1px solid var(--red)",
          maxWidth: 560,
          width: "100%",
        }}
      >
        <div className="flex items-center gap-2">
          <span style={{ fontSize: 22 }}>🔌</span>
          <h1 style={{ color: "var(--red)", fontWeight: 900, fontSize: 16, margin: 0 }}>
            {t.app.connectionFailedTitle}
          </h1>
        </div>
        <p style={{ color: "var(--text-muted)", fontSize: 12, margin: 0 }}>
          {t.app.connectionFailedBody}
        </p>
        <div
          className="rounded-lg px-3 py-2 font-mono"
          style={{ background: "#0B0D14", border: "1px solid var(--border)", fontSize: 11.5, color: "var(--yellow)" }}
        >
          {error}
        </div>
        <div className="flex flex-col gap-1" style={{ fontSize: 12 }}>
          <div>
            <span style={{ color: "var(--text-muted)" }}>{t.app.backendKindLabel}: </span>
            <span style={{ fontWeight: 700 }}>{getBackendKind()}</span>
          </div>
          <div>
            <span style={{ color: "var(--text-muted)" }}>{t.app.devServerLabel}: </span>
            <span style={{ fontWeight: 700, color: devServerUp ? "var(--green)" : "var(--red)" }}>
              {devServerUp === null ? "…" : devServerUp ? t.app.devServerReachable : t.app.devServerUnreachable}
            </span>
          </div>
        </div>
        <div style={{ color: "var(--text-muted)", fontSize: 12 }}>
          <div>• {t.app.launchHelpDev}</div>
          <div>• {t.app.launchHelpInstalled}</div>
        </div>
        <button
          onClick={onRetry}
          className="px-4 py-2 rounded-lg text-sm font-bold self-start"
          style={{ background: "linear-gradient(90deg, var(--accent2), var(--accent))", color: "#fff", border: "none", cursor: "pointer" }}
        >
          {t.app.retryButton}
        </button>
      </div>
    </div>
  );
}

export default function App() {
  const initFromState = useAudioStore((s) => s.initFromState);
  const initialized = useAudioStore((s) => s.initialized);
  const [bootstrapError, setBootstrapError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  // In-window hotkey listener (Tier 3 fallback + window-focus support)
  useInWindowHotkeys();

  // WebKitGTK viewport shift safeguard
  useEffect(() => {
    const handleScroll = () => {
      if (window.scrollX !== 0 || window.scrollY !== 0) {
        window.scrollTo(0, 0);
      }
    };
    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  // Bootstrap: fetch full state on startup (with error surfacing instead
  // of an infinite spinner when no backend is reachable).
  useEffect(() => {
    let cancelled = false;
    setBootstrapError(null);
    api
      .getState()
      .then((s) => {
        if (!cancelled) initFromState(s);
      })
      .catch((e: unknown) => {
        if (!cancelled) setBootstrapError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [initFromState, attempt]);

  // VU Meter polling
  useMeter();

  if (bootstrapError) {
    return <ConnectionFailed error={bootstrapError} onRetry={() => setAttempt((a) => a + 1)} />;
  }

  if (!initialized) {
    return (
      <div
        className="flex items-center justify-center h-screen"
        style={{ background: "var(--bg-base)", color: "var(--accent)" }}
      >
        <div className="flex flex-col items-center gap-3">
          <span style={{ fontSize: 32 }}>🎙</span>
          <span style={{ fontWeight: 900, fontSize: 18, letterSpacing: 2 }}>AUDIOVER</span>
          <span style={{ color: "var(--text-muted)", fontSize: 12 }}>Bağlanıyor...</span>
        </div>
      </div>
    );
  }

  return (
    <Layout>
      <ActivePage />
    </Layout>
  );
}
