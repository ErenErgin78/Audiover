import clsx from "clsx";
import { useAudioStore, type PageId } from "../../store/audioStore";
import { useI18n } from "../../hooks/useI18n";

export default function Sidebar() {
  const activePage = useAudioStore((s) => s.activePage);
  const setActivePage = useAudioStore((s) => s.setActivePage);
  const { t } = useI18n();

  const navItems: { page: PageId; icon: string; label: string }[] = [
    { page: "voice",      icon: "🎙",  label: t.nav.voice      },
    { page: "soundboard", icon: "🔊",  label: t.nav.soundboard },
    { page: "audio",      icon: "🎚",   label: t.nav.audio      },
    { page: "hotkeys",    icon: "⌨",  label: t.nav.hotkeys    },
    { page: "settings",   icon: "⚙",   label: t.nav.settings   },
  ];

  return (
    <aside
      className="flex flex-col pt-3 shrink-0"
      style={{
        width: 210,
        background: "var(--bg-sidebar)",
        borderRight: "1px solid var(--border)",
      }}
    >
      {navItems.map(({ page, icon, label }) => {
        const active = activePage === page;
        return (
          <button
            key={page}
            onClick={() => setActivePage(page)}
            className={clsx(
              "flex items-center gap-3 mx-2.5 my-0.5 px-4 rounded-lg text-sm font-medium transition-all text-left cursor-pointer",
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
            <span className="truncate">{label}</span>
          </button>
        );
      })}
    </aside>
  );
}
