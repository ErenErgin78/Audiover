import { useEffect, useState } from "react";
import { api } from "../hooks/useApi";

interface HotkeyStatus {
  has_permission: boolean;
  hotkeys: Array<{ action: string; key: string }>;
}

export default function HotkeysPage() {
  const [status, setStatus] = useState<HotkeyStatus | null>(null);

  useEffect(() => {
    api.getHotkeyStatus().then(setStatus);
  }, []);

  if (!status) {
    return (
      <div className="flex items-center justify-center h-full" style={{ color: "var(--text-muted)" }}>
        Yükleniyor...
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 p-6 overflow-y-auto">
      <h1 style={{ color: "var(--accent)", fontWeight: 900, fontSize: 20, letterSpacing: 1 }}>
        Global Hotkeys
      </h1>

      {/* Permission Status */}
      <section
        className="rounded-xl p-4"
        style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
      >
        <h2 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 13, marginBottom: 10 }}>
          Wayland & Linux Global Shortcut Status
        </h2>
        {status.has_permission ? (
          <div>
            <p style={{ color: "var(--green)", fontWeight: 700, fontSize: 13 }}>
              ✓ Global Input Access Active (/dev/input)
            </p>
            <p style={{ color: "var(--text-muted)", fontSize: 12, marginTop: 4 }}>
              Hotkeys oyun, Discord ve arka planda çalışan pencerelerde aktif.
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <p style={{ color: "var(--yellow)", fontWeight: 700, fontSize: 13 }}>
              ⚠ /dev/input Erişimi Bulunamadı
            </p>
            <p style={{ color: "var(--text-muted)", fontSize: 12 }}>
              Wayland'da arka plan kısayolları için kullanıcını{" "}
              <code style={{ color: "var(--accent)" }}>input</code> grubuna ekle:
            </p>
            <code
              className="px-3 py-2 rounded-lg text-xs"
              style={{ background: "var(--bg-surface)", color: "var(--accent)", display: "block" }}
            >
              sudo usermod -aG input $USER
            </code>
            <p style={{ color: "var(--text-muted)", fontSize: 11 }}>
              (Komuttan sonra oturumu bir kez kapat/aç)
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
          Global Action Shortcuts
        </h2>
        <table className="w-full text-sm">
          <thead>
            <tr style={{ borderBottom: "1px solid var(--border)" }}>
              <th className="px-4 py-2 text-left" style={{ color: "var(--text-muted)", fontWeight: 600 }}>
                Eylem
              </th>
              <th className="px-4 py-2 text-center" style={{ color: "var(--text-muted)", fontWeight: 600, width: 120 }}>
                Tuş
              </th>
            </tr>
          </thead>
          <tbody>
            {status.hotkeys.map((hk, i) => (
              <tr
                key={i}
                style={{ borderBottom: i < status.hotkeys.length - 1 ? "1px solid var(--border)" : undefined }}
              >
                <td className="px-4 py-3" style={{ color: "var(--text)" }}>{hk.action}</td>
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
          Soundboard ses tuşları her ses kartında ayrıca atanabilir.
        </p>
      </section>
    </div>
  );
}
