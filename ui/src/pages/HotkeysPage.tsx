import { useEffect, useState } from "react";
import { api, type HotkeyStatus } from "../hooks/useApi";
import { useI18n } from "../hooks/useI18n";

export default function HotkeysPage() {
  const [status, setStatus] = useState<HotkeyStatus | null>(null);
  const { t } = useI18n();

  useEffect(() => {
    api.getHotkeyStatus().then(setStatus);
  }, []);

  if (!status) {
    return (
      <div className="flex items-center justify-center h-full" style={{ color: "var(--text-muted)" }}>
        {t.hotkeys.loading}
      </div>
    );
  }

  const getActionLabel = (action: string) => {
    if (action.includes("Mute")) return t.hotkeys.muteMicAction;
    if (action.includes("Bypass")) return t.hotkeys.bypassDspAction;
    if (action.includes("Stop")) return t.hotkeys.stopAllAction;
    if (action.includes("Hear")) return t.hotkeys.toggleHearMyselfAction;
    return action;
  };

  const isPortal = status.backend === "portal";
  const isEvdev = status.backend === "evdev";
  const isInWindow = status.backend === "in_window";

  return (
    <div className="flex flex-col gap-4 p-6 overflow-y-auto max-w-4xl">
      <div className="flex items-center justify-between">
        <h1 style={{ color: "var(--accent)", fontWeight: 900, fontSize: 20, letterSpacing: 1 }}>
          {t.hotkeys.title}
        </h1>
        <span
          className="px-3 py-1 rounded-full text-xs font-bold font-mono tracking-wide"
          style={{
            background: isPortal
              ? "rgba(0, 229, 255, 0.15)"
              : isEvdev
              ? "rgba(0, 230, 118, 0.15)"
              : "rgba(255, 214, 0, 0.15)",
            color: isPortal
              ? "var(--accent)"
              : isEvdev
              ? "var(--green)"
              : "var(--yellow)",
            border: isPortal
              ? "1px solid rgba(0, 229, 255, 0.3)"
              : isEvdev
              ? "1px solid rgba(0, 230, 118, 0.3)"
              : "1px solid rgba(255, 214, 0, 0.3)",
          }}
        >
          {isPortal
            ? t.hotkeys.tierPortalBadge
            : isEvdev
            ? t.hotkeys.tierEvdevBadge
            : t.hotkeys.tierInWindowBadge}
        </span>
      </div>

      {/* Multi-Tier Status Card */}
      <section
        className="rounded-xl p-5 flex flex-col gap-3"
        style={{
          background: "var(--bg-card)",
          border: isPortal || isEvdev ? "1px solid var(--border-hover)" : "1px solid var(--border)",
        }}
      >
        <div className="flex items-center gap-2">
          <span className="text-lg">
            {isPortal ? "🌐" : isEvdev ? "⚡" : "🪟"}
          </span>
          <h2 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 14 }}>
            {isPortal
              ? t.hotkeys.tierPortalTitle
              : isEvdev
              ? t.hotkeys.tierEvdevTitle
              : t.hotkeys.tierInWindowTitle}
          </h2>
        </div>

        {isPortal && (
          <div>
            <p style={{ color: "var(--green)", fontWeight: 600, fontSize: 13 }}>
              {t.hotkeys.tierPortalDesc}
            </p>
          </div>
        )}

        {isEvdev && (
          <div>
            <p style={{ color: "var(--green)", fontWeight: 600, fontSize: 13 }}>
              {t.hotkeys.tierEvdevDesc}
            </p>
          </div>
        )}

        {isInWindow && (
          <div className="flex flex-col gap-2.5">
            <p style={{ color: "var(--yellow)", fontWeight: 600, fontSize: 13 }}>
              {t.hotkeys.tierInWindowDesc}
            </p>
            <p style={{ color: "var(--text-muted)", fontSize: 12 }}>
              {t.hotkeys.tierInWindowHelp}{" "}
              <code style={{ color: "var(--accent)" }}>input</code>:
            </p>
            <code
              className="px-3 py-2 rounded-lg text-xs font-mono"
              style={{ background: "var(--bg-surface)", color: "var(--accent)", display: "block" }}
            >
              sudo usermod -aG input $USER
            </code>
            <p style={{ color: "var(--text-muted)", fontSize: 11 }}>
              {t.hotkeys.statusHelp}
            </p>
          </div>
        )}
      </section>

      {/* Hotkey Table */}
      <section
        className="rounded-xl overflow-hidden"
        style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
      >
        <h2
          className="px-4 py-3"
          style={{
            color: "var(--accent)", fontWeight: 700, fontSize: 13,
            borderBottom: "1px solid var(--border)", margin: 0,
          }}
        >
          {t.hotkeys.actionShortcutsCard}
        </h2>
        <table className="w-full text-sm">
          <thead>
            <tr style={{ borderBottom: "1px solid var(--border)" }}>
              <th className="px-4 py-2 text-left" style={{ color: "var(--text-muted)", fontWeight: 600 }}>
                {t.hotkeys.actionHeader}
              </th>
              <th className="px-4 py-2 text-center" style={{ color: "var(--text-muted)", fontWeight: 600, width: 120 }}>
                {t.hotkeys.keyHeader}
              </th>
            </tr>
          </thead>
          <tbody>
            {status.hotkeys.map((hk, i) => (
              <tr
                key={i}
                style={{ borderBottom: i < status.hotkeys.length - 1 ? "1px solid var(--border)" : undefined }}
              >
                <td className="px-4 py-3" style={{ color: "var(--text)" }}>{getActionLabel(hk.action)}</td>
                <td className="px-4 py-3 text-center">
                  <kbd
                    className="px-2 py-1 rounded text-xs font-mono font-bold"
                    style={{
                      background: "var(--bg-surface)",
                      border: "1px solid var(--border-hover)",
                      color: "var(--accent)",
                    }}
                  >
                    {hk.key}
                  </kbd>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className="px-4 py-3 text-xs" style={{ color: "var(--text-muted)" }}>
          {t.hotkeys.soundboardNote}
        </p>
      </section>
    </div>
  );
}

