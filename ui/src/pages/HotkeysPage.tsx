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

  return (
    <div className="flex flex-col gap-4 p-6 overflow-y-auto max-w-4xl">
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
      </section>
    </div>
  );
}

