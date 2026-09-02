import { useAudioStore } from "../../store/audioStore";
import { api } from "../../hooks/useApi";
import { useI18n } from "../../hooks/useI18n";
import clsx from "clsx";

/** VU Meter bar — scaleX animasyonlu, tek kanal */
function VUBar({ peak }: { peak: number }) {
  const scale = Math.min(peak, 1);
  return (
    <div
      className="overflow-hidden rounded-sm"
      style={{ width: 120, height: 8, background: "var(--border)" }}
    >
      <div
        className="vu-track h-full"
        style={{ transform: `scaleX(${scale})` }}
      />
    </div>
  );
}

export default function Header() {
  const engineActive = useAudioStore((s) => s.engineActive);
  const isMuted = useAudioStore((s) => s.isMuted);
  const hearMyself = useAudioStore((s) => s.hearMyself);
  const meters = useAudioStore((s) => s.meters);
  const setEngineActive = useAudioStore((s) => s.setEngineActive);
  const setMuted = useAudioStore((s) => s.setMuted);
  const setHearMyself = useAudioStore((s) => s.setHearMyself);
  const { t } = useI18n();

  const handleEngineToggle = async () => {
    const res = await api.setEngineActive(!engineActive);
    setEngineActive(res.active);
  };

  const handleMuteToggle = async () => {
    const next = !isMuted;
    await api.setMuted(next);
    setMuted(next);
  };

  const handleHearMyselfToggle = async () => {
    const next = !hearMyself;
    await api.setHearMyself(next);
    setHearMyself(next);
  };

  return (
    <header
      className="flex items-center gap-4 px-4 py-2 shrink-0"
      style={{
        background: "#12141F",
        borderBottom: "1px solid var(--border)",
        height: 56,
      }}
    >
      {/* Logo */}
      <div className="flex flex-col leading-tight mr-2">
        <span
          className="font-black tracking-widest"
          style={{ color: "var(--accent)", fontSize: 17 }}
        >
          AUDIOVER
        </span>
        <span style={{ color: "var(--text-muted)", fontSize: 10 }}>
          Voice & Soundboard Engine
        </span>
      </div>

      <div className="flex-1" />

      {/* Engine Toggle */}
      <button
        onClick={handleEngineToggle}
        className={clsx(
          "px-4 py-1.5 rounded-lg text-xs font-bold transition-colors cursor-pointer",
          engineActive
            ? "text-black"
            : "text-white"
        )}
        style={{
          background: engineActive ? "var(--green)" : "var(--red)",
        }}
      >
        {engineActive ? t.header.engineActive : t.header.engineStopped}
      </button>

      {/* Mute Mic */}
      <button
        onClick={handleMuteToggle}
        className={clsx(
          "px-3 py-1.5 rounded-lg text-xs font-semibold border transition-colors cursor-pointer",
          isMuted
            ? "text-white border-red-500"
            : "border-[var(--border-hover)] text-[var(--text)]"
        )}
        style={{
          background: isMuted ? "var(--red)" : "var(--bg-surface)",
        }}
      >
        {isMuted ? t.header.muted : t.header.muteMic}
      </button>

      {/* Hear Myself */}
      <button
        onClick={handleHearMyselfToggle}
        className={clsx(
          "px-3 py-1.5 rounded-lg text-xs font-semibold border transition-colors cursor-pointer",
          hearMyself
            ? "text-white border-purple-400"
            : "border-[var(--border-hover)] text-[var(--text)]"
        )}
        style={{
          background: hearMyself ? "var(--accent2)" : "var(--bg-surface)",
        }}
      >
        {t.header.hearMyself}
      </button>

      {/* VU Meters */}
      <div className="flex flex-col gap-1">
        <div className="flex items-center gap-2">
          <span style={{ color: "var(--text-muted)", fontSize: 10, width: 26 }}>{t.header.in}</span>
          <VUBar peak={meters.in_peak} />
        </div>
        <div className="flex items-center gap-2">
          <span style={{ color: "var(--text-muted)", fontSize: 10, width: 26 }}>{t.header.out}</span>
          <VUBar peak={meters.out_peak} />
        </div>
      </div>
    </header>
  );
}
