import { useEffect, useState } from "react";
import { api } from "../hooks/useApi";
import { useI18n } from "../hooks/useI18n";

interface HotkeyStatus {
  has_permission: boolean;
  hotkeys: Array<{ action: string; key: string }>;
}

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

  return (
    <div className="flex flex-col gap-4 p-6 overflow-y-auto max-w-4xl">
      <h1 style={{ color: "var(--accent)", fontWeight: 900, fontSize: 20, letterSpacing: 1 }}>
        {t.hotkeys.title}
      </h1>

      {/* Permission Status */}
      <section
        className="rounded-xl p-4"
        style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
      >
        <h2 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 13, marginBottom: 10 }}>
          {t.hotkeys.statusCard}
        </h2>
        {status.has_permission ? (
          <div>
            <p style={{ color: "var(--green)", fontWeight: 700, fontSize: 13 }}>
              {t.hotkeys.statusActive}
            </p>
            <p style={{ color: "var(--text-muted)", fontSize: 12, marginTop: 4 }}>
              {t.hotkeys.statusActiveDesc}
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <p style={{ color: "var(--yellow)", fontWeight: 700, fontSize: 13 }}>
              {t.hotkeys.statusInactive}
            </p>
            <p style={{ color: "var(--text-muted)", fontSize: 12 }}>
              {t.hotkeys.statusInactiveDesc}{" "}
              <code style={{ color: "var(--accent)" }}>input</code>:
            </p>
            <code
              className="px-3 py-2 rounded-lg text-xs"
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
