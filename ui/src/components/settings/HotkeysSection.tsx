import { useEffect, useState } from "react";
import { api, type HotkeyStatus } from "../../hooks/useApi";
import { useI18n } from "../../hooks/useI18n";
import { normalizeKey } from "../../utils/keyNormalizer";

function actionIdOf(hk: { id?: string; action: string }): string {
  if (hk.id) return hk.id;
  // Back-compat with backends that only send the display label.
  if (hk.action.includes("Mute")) return "mute_mic";
  if (hk.action.includes("Bypass")) return "bypass_dsp";
  if (hk.action.includes("Stop")) return "stop_all";
  if (hk.action.includes("Hear")) return "toggle_hear_myself";
  return hk.action;
}

export default function HotkeysSection() {
  const [status, setStatus] = useState<HotkeyStatus | null>(null);
  const [capturing, setCapturing] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { t } = useI18n();

  useEffect(() => {
    api.getHotkeyStatus().then(setStatus).catch(() => {});
  }, []);

  // Capture the next keypress for remapping.
  useEffect(() => {
    if (!capturing) return;
    document.body.dataset.capturingHotkey = "1";
    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setCapturing(null);
        return;
      }
      const key = normalizeKey(e);
      if (!key) return;
      const actionId = capturing;
      setCapturing(null);
      api
        .setHotkey(actionId, key)
        .then((res: any) => {
          if (res?.ok) {
            setError(null);
            if (Array.isArray(res.hotkeys)) {
              setStatus((s) => (s ? { ...s, hotkeys: res.hotkeys } : s));
            } else {
              setStatus((s) =>
                s
                  ? {
                      ...s,
                      hotkeys: s.hotkeys.map((hk) =>
                        actionIdOf(hk) === actionId ? { ...hk, key } : hk
                      ),
                    }
                  : s
              );
            }
          } else {
            const detail = res?.message || res?.conflict || res?.error || "";
            setError(
              detail ? `${t.hotkeys.conflictPrefix} ${detail}` : t.hotkeys.conflictPrefix
            );
          }
        })
        .catch((err) => {
          setError(String(err?.message ?? err));
        });
    };
    window.addEventListener("keydown", handleKeyDown, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      delete document.body.dataset.capturingHotkey;
    };
  }, [capturing, t]);

  if (!status) {
    return (
      <div className="py-6 text-center" style={{ color: "var(--text-muted)" }}>
        {t.hotkeys.loading}
      </div>
    );
  }

  const getActionLabel = (hk: { id?: string; action: string }) => {
    const id = actionIdOf(hk);
    if (id === "mute_mic") return t.hotkeys.muteMicAction;
    if (id === "bypass_dsp") return t.hotkeys.bypassDspAction;
    if (id === "stop_all") return t.hotkeys.stopAllAction;
    if (id === "toggle_hear_myself") return t.hotkeys.toggleHearMyselfAction;
    return hk.action;
  };

  const handleReset = async () => {
    setError(null);
    try {
      const res: any = await api.resetHotkeys();
      if (res?.ok && Array.isArray(res.hotkeys)) {
        setStatus((s) => (s ? { ...s, hotkeys: res.hotkeys } : s));
      } else {
        const fresh = await api.getHotkeyStatus();
        setStatus(fresh);
      }
    } catch (e) {
      console.error("reset hotkeys failed:", e);
    }
  };

  return (
    <section
      className="rounded-xl overflow-hidden w-full"
      style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
    >
      <div
        className="px-4 py-3 flex items-center gap-3"
        style={{ borderBottom: "1px solid var(--border)" }}
      >
        <h2
          className="flex-1"
          style={{ color: "var(--accent)", fontWeight: 700, fontSize: 13, margin: 0 }}
        >
          {t.hotkeys.actionShortcutsCard}
        </h2>
        <button
          onClick={handleReset}
          className="px-2.5 py-1 rounded-lg text-xs font-semibold cursor-pointer"
          style={{ background: "var(--bg-surface)", color: "var(--text-muted)" }}
        >
          {t.hotkeys.resetDefaults}
        </button>
      </div>
      {capturing && (
        <div
          className="px-4 py-2 text-xs font-semibold"
          style={{
            color: "var(--accent)",
            borderBottom: "1px solid var(--border)",
            background: "rgba(0, 229, 255, 0.06)",
          }}
        >
          {t.hotkeys.pressKeyPrompt}
          <button
            onClick={() => setCapturing(null)}
            className="ml-3 px-2 py-0.5 rounded text-xs cursor-pointer"
            style={{ background: "var(--bg-surface)", color: "var(--text-muted)" }}
          >
            {t.hotkeys.cancel}
          </button>
        </div>
      )}
      {error && (
        <div
          className="px-4 py-2 text-xs font-semibold"
          style={{ color: "var(--red)", borderBottom: "1px solid var(--border)" }}
        >
          {error}
        </div>
      )}
      <table className="w-full text-sm">
        <thead>
          <tr style={{ borderBottom: "1px solid var(--border)" }}>
            <th className="px-4 py-2 text-left" style={{ color: "var(--text-muted)", fontWeight: 600 }}>
              {t.hotkeys.actionHeader}
            </th>
            <th className="px-4 py-2 text-center" style={{ color: "var(--text-muted)", fontWeight: 600, width: 160 }}>
              {t.hotkeys.keyHeader}
            </th>
          </tr>
        </thead>
        <tbody>
          {status.hotkeys.map((hk, i) => {
            const id = actionIdOf(hk);
            const isCapturing = capturing === id;
            return (
              <tr
                key={id || i}
                style={{ borderBottom: i < status.hotkeys.length - 1 ? "1px solid var(--border)" : undefined }}
              >
                <td className="px-4 py-3" style={{ color: "var(--text)" }}>{getActionLabel(hk)}</td>
                <td className="px-4 py-3 text-center">
                  <button
                    onClick={() => {
                      setError(null);
                      setCapturing(id);
                    }}
                    title={t.hotkeys.changeHint}
                    className="px-2 py-1 rounded text-xs font-mono font-bold cursor-pointer transition-all hover:scale-105"
                    style={{
                      background: isCapturing ? "rgba(0, 229, 255, 0.2)" : "var(--bg-surface)",
                      border: isCapturing
                        ? "1px solid var(--accent)"
                        : "1px solid var(--border-hover)",
                      color: "var(--accent)",
                      minWidth: 72,
                    }}
                  >
                    {isCapturing ? "…" : hk.key || "—"}
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}
