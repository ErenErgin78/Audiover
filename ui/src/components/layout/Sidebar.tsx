import clsx from "clsx";
import { useAudioStore } from "../../store/audioStore";

type Page = "voice" | "soundboard" | "audio" | "hotkeys";

const NAV_ITEMS: { page: Page; icon: string; label: string }[] = [
  { page: "voice",      icon: "🎙",  label: "Voice Changer"   },
  { page: "soundboard", icon: "🔊",  label: "Soundboard"      },
  { page: "audio",      icon: "⚙",   label: "Audio & Routing" },
  { page: "hotkeys",    icon: "⌨",   label: "Global Hotkeys"  },
];

export default function Sidebar() {
  const activePage = useAudioStore((s) => s.activePage);
  const setActivePage = useAudioStore((s) => s.setActivePage);

  return (
    <aside
      className="flex flex-col pt-3 shrink-0"
      style={{
        width: 210,
        background: "var(--bg-sidebar)",
        borderRight: "1px solid var(--border)",
      }}
    >
      {NAV_ITEMS.map(({ page, icon, label }) => {
        const active = activePage === page;
        return (
          <button
            key={page}
            onClick={() => setActivePage(page)}
            className={clsx(
              "flex items-center gap-3 mx-2.5 my-0.5 px-4 rounded-lg text-sm font-medium transition-all text-left",
              active
                ? "text-white font-bold"
                : "text-[var(--text-muted)] hover:bg-[#1E2235] hover:text-white"
            )}
            style={{
              height: 48,
              background: active
                ? "linear-gradient(90deg, var(--accent2), var(--accent))"
                : undefined,
            }}
          >
            <span style={{ fontSize: 16 }}>{icon}</span>
            <span>{label}</span>
          </button>
        );
      })}
    </aside>
  );
}
