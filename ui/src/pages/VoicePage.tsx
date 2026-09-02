import { useState, useCallback } from "react";
import { useAudioStore } from "../store/audioStore";
import { api, type PresetConfig } from "../hooks/useApi";
import { useI18n } from "../hooks/useI18n";

const DEFAULT_PRESET_NAMES = ["Clean", "Deep Voice"];

const PRESET_META: Record<string, { icon: string; descKey?: "clean" | "deep" }> = {
  Clean:        { icon: "🎙", descKey: "clean" },
  "Deep Voice": { icon: "🔊", descKey: "deep" },
};

/* ── DSP Slider Row ──────────────────────────────────────────── */
interface SliderRowProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  format: (v: number) => string;
  onChange: (v: number) => void;
  enabled?: boolean;
  onToggle?: (v: boolean) => void;
}

function SliderRow({ label, value, min, max, step = 1, format, onChange, enabled, onToggle }: SliderRowProps) {
  return (
    <div className="flex items-center gap-2.5">
      {onToggle !== undefined ? (
        <label className="flex items-center gap-2 cursor-pointer shrink-0" style={{ width: 135 }}>
          <input
            type="checkbox"
            checked={enabled ?? false}
            onChange={(e) => onToggle(e.target.checked)}
            className="w-4 h-4 rounded accent-[var(--accent)] cursor-pointer shrink-0"
          />
          <span className="truncate" style={{ color: "var(--text-muted)", fontSize: 12 }} title={label}>
            {label}
          </span>
        </label>
      ) : (
        <span
          className="shrink-0 truncate"
          style={{ color: "var(--text-muted)", fontSize: 12, width: 135 }}
          title={label}
        >
          {label}
        </span>
      )}
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="flex-1 min-w-0"
        style={{ accentColor: "var(--accent)" }}
      />
      <span
        className="shrink-0 text-right font-mono"
        style={{ color: "var(--accent)", fontSize: 12, width: 50 }}
      >
        {format(value)}
      </span>
    </div>
  );
}

/* ── DSP Section Card ────────────────────────────────────────── */
function DspSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section
      className="rounded-xl p-4 flex flex-col gap-3"
      style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
    >
      <h3 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 13, margin: 0 }}>{title}</h3>
      {children}
    </section>
  );
}

/* ── DSP Drawer (side panel) ─────────────────────────────────── */
interface DspDrawerProps {
  dsp: PresetConfig;
  onDspChange: (patch: Partial<PresetConfig>) => void;
  onClose: () => void;
  activePreset: string;
  onNew: () => void;
  onDelete: (name: string) => void;
  onReset: (name: string) => void;
}

function DspDrawer({ dsp, onDspChange, onClose, activePreset, onNew, onDelete, onReset }: DspDrawerProps) {
  const { t } = useI18n();
  const isBuiltin = DEFAULT_PRESET_NAMES.includes(activePreset);

  const handleDelete = () => {
    if (!isBuiltin && confirm(`'${activePreset}' ${t.voice.confirmDelete}`)) onDelete(activePreset);
  };

  const handleReset = () => {
    if (isBuiltin && confirm(`'${activePreset}' ${t.voice.confirmReset}`)) onReset(activePreset);
  };

  return (
    <div
      className="flex flex-col h-full shrink-0"
      style={{
        width: 390,
        borderLeft: "1px solid var(--border)",
        background: "#12141F",
      }}
    >
      {/* Drawer Header: Geri + Preset adı + Eylem butonları */}
      <div
        className="flex items-center gap-2 px-4 py-3 shrink-0"
        style={{ borderBottom: "1px solid var(--border)" }}
      >
        <button
          onClick={onClose}
          className="text-sm font-semibold px-3 py-1.5 rounded-lg shrink-0 cursor-pointer transition-colors"
          style={{ background: "var(--bg-surface)", color: "var(--text)" }}
        >
          {t.voice.back}
        </button>

        {/* Aktif preset adı */}
        <div className="flex-1 min-w-0 px-1">
          <div style={{ color: "var(--text-muted)", fontSize: 10, marginBottom: 1 }}>{t.voice.editing}</div>
          <div className="truncate font-bold" style={{ color: "var(--accent)", fontSize: 14 }}>
            {activePreset}
            {isBuiltin && (
              <span style={{ color: "var(--text-muted)", fontWeight: 400, fontSize: 11, marginLeft: 6 }}>
                {t.voice.defaultBadge}
              </span>
            )}
          </div>
        </div>

        <button
          onClick={onNew}
          className="px-2.5 py-1.5 rounded-lg text-xs font-bold shrink-0 cursor-pointer transition-opacity hover:opacity-90"
          style={{ background: "#1A2738", color: "var(--accent)", border: "1px solid var(--accent)" }}
        >
          {t.voice.newPreset}
        </button>
        {isBuiltin ? (
          <button
            onClick={handleReset}
            title="Reset"
            className="px-2.5 py-1.5 rounded-lg text-xs font-bold shrink-0 cursor-pointer transition-opacity hover:opacity-90"
            style={{ background: "var(--bg-surface)", color: "var(--text-muted)", border: "1px solid var(--border)" }}
          >
            {t.voice.reset}
          </button>
        ) : (
          <button
            onClick={handleDelete}
            title="Delete"
            className="px-2.5 py-1.5 rounded-lg text-xs font-bold shrink-0 cursor-pointer transition-opacity hover:opacity-90"
            style={{ background: "#2D1A24", color: "var(--red)", border: "1px solid #5A2030" }}
          >
            {t.voice.delete}
          </button>
        )}
      </div>

      {/* DSP Controls */}
      <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-4">
        {/* Bypass */}
        <label className="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={dsp.bypass ?? false}
            onChange={(e) => onDspChange({ bypass: e.target.checked })}
            className="w-4 h-4 accent-[var(--accent)]"
          />
          <span className="text-sm" style={{ color: "var(--text-muted)" }}>
            {t.voice.bypass}
          </span>
        </label>

        <DspSection title={t.voice.pitchShifter}>
          <SliderRow
            label={t.voice.pitchShift}
            value={Math.round((dsp.pitch ?? 0) * 10)}
            min={-120} max={120}
            format={(v) => `${v >= 0 ? "+" : ""}${(v / 10).toFixed(1)} st`}
            onChange={(v) => onDspChange({ pitch: v / 10 })}
          />
        </DspSection>

        <DspSection title={t.voice.robotic}>
          <SliderRow
            label={t.voice.roboticMod}
            value={dsp.robot ? 1 : 0} min={0} max={1}
            format={() => ""}
            onChange={() => {}}
            enabled={dsp.robot}
            onToggle={(v) => onDspChange({ robot: v })}
          />
          <SliderRow
            label={t.voice.modFreq}
            value={dsp.rfreq ?? 150} min={50} max={500}
            format={(v) => `${v} Hz`}
            onChange={(v) => onDspChange({ rfreq: v })}
          />
          <SliderRow
            label={t.voice.robotMix}
            value={Math.round((dsp.rmix ?? 0) * 100)} min={0} max={100}
            format={(v) => `${v}%`}
            onChange={(v) => onDspChange({ rmix: v / 100 })}
          />
        </DspSection>

        <DspSection title={t.voice.spatial}>
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={dsp.radio ?? false}
              onChange={(e) => onDspChange({ radio: e.target.checked })}
              className="w-4 h-4 accent-[var(--accent)]"
            />
            <span style={{ color: "var(--text-muted)", fontSize: 12 }}>{t.voice.walkieTalkie}</span>
          </label>
          <SliderRow
            label={t.voice.distortion}
            value={Math.round((dsp.drive ?? 0) * 100)} min={0} max={100}
            format={(v) => `${v}%`}
            onChange={(v) => onDspChange({ drive: v / 100 })}
            enabled={dsp.dist}
            onToggle={(v) => onDspChange({ dist: v })}
          />
          <SliderRow
            label={t.voice.reverb}
            value={Math.round((dsp.rwet ?? 0) * 100)} min={0} max={100}
            format={(v) => `${v}%`}
            onChange={(v) => onDspChange({ rwet: v / 100 })}
            enabled={dsp.rev}
            onToggle={(v) => onDspChange({ rev: v })}
          />
          <SliderRow
            label={t.voice.chorus}
            value={Math.round((dsp.cdepth ?? 0) * 100)} min={0} max={100}
            format={(v) => `${v}%`}
            onChange={(v) => onDspChange({ cdepth: v / 100 })}
            enabled={dsp.chorus}
            onToggle={(v) => onDspChange({ chorus: v })}
          />
          <SliderRow
            label={t.voice.noiseGate}
            value={dsp.gate_db ?? -65} min={-80} max={-30}
            format={(v) => `${v} dB`}
            onChange={(v) => onDspChange({ gate_db: v })}
            enabled={dsp.gate}
            onToggle={(v) => onDspChange({ gate: v })}
          />
        </DspSection>
      </div>
    </div>
  );
}

/* ── VoicePage ───────────────────────────────────────────────── */
export default function VoicePage() {
  const activePreset = useAudioStore((s) => s.activePreset);
  const presets = useAudioStore((s) => s.presets);
  const setActivePreset = useAudioStore((s) => s.setActivePreset);
  const setPresets = useAudioStore((s) => s.setPresets);
  const updatePreset = useAudioStore((s) => s.updatePreset);
  const { t } = useI18n();

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [dsp, setDsp] = useState<PresetConfig>(
    () => presets[activePreset] ?? presets["Clean"] ?? ({} as PresetConfig)
  );

  const handlePresetClick = async (name: string) => {
    const res = await api.applyPreset(name);
    if (res.ok) {
      setActivePreset(name);
      setDsp(presets[name] ?? ({} as PresetConfig));
      setDrawerOpen(true);
    }
  };

  const handleDspChange = useCallback(
    async (patch: Partial<PresetConfig>) => {
      const next = { ...dsp, ...patch };
      setDsp(next);
      updatePreset(activePreset, next);
      await api.updateDsp(next);
    },
    [dsp, activePreset, updatePreset]
  );

  const handleNewPreset = async () => {
    const name = prompt(t.voice.newPresetPrompt);
    if (!name?.trim()) return;
    const res = await api.createPreset(name.trim(), dsp);
    if (res.ok && res.presets) {
      setPresets(res.presets);
      if (res.name) {
        setActivePreset(res.name);
        setDsp(res.presets[res.name] ?? dsp);
      }
    } else if (res.error) {
      alert(res.error);
    }
  };

  const handleResetPreset = async (name: string) => {
    const res = await api.resetPreset(name);
    if (res.ok && res.presets && res.config) {
      setPresets(res.presets);
      setDsp(res.config);
    } else if (res.error) {
      alert(res.error);
    }
  };

  const handleDeletePreset = async (name: string) => {
    const res = await api.deletePreset(name);
    if (res.ok && res.presets) {
      setPresets(res.presets);
      if (res.active) {
        setActivePreset(res.active);
        setDsp(res.presets[res.active] ?? ({} as PresetConfig));
      }
    }
  };

  const presetList = Object.keys(presets);
  const cols = presetList.length <= 4 ? 2 : 3;

  return (
    <div className="flex h-full">
      {/* Sol: Preset Grid */}
      <div className="flex flex-col flex-1 min-w-0 p-6 gap-5 overflow-y-auto">
        {/* Başlık */}
        <div>
          <h1 style={{ color: "var(--accent)", fontWeight: 900, fontSize: 20, letterSpacing: 1 }}>
            {t.voice.title}
          </h1>
          <p style={{ color: "var(--text-muted)", fontSize: 12, marginTop: 2 }}>
            {t.voice.subtitle}
            {drawerOpen && (
              <span style={{ color: "var(--text)" }}>
                {" "}· {t.voice.active}:{" "}
                <strong style={{ color: "var(--accent)" }}>{activePreset}</strong>
              </span>
            )}
          </p>
        </div>

        {/* Preset Kartları */}
        <div
          className="grid gap-5 place-items-center"
          style={{ gridTemplateColumns: `repeat(${cols}, 220px)` }}
        >
          {presetList.map((name) => {
            const meta = PRESET_META[name] ?? { icon: "★" };
            const isActive = name === activePreset;
            const isEditing = isActive && drawerOpen;
            return (
              <button
                key={name}
                onClick={() => handlePresetClick(name)}
                className="flex flex-col items-center justify-center gap-1.5 rounded-2xl transition-all cursor-pointer"
                style={{
                  width: 220,
                  height: 130,
                  background: isActive
                    ? "linear-gradient(135deg, var(--accent2), var(--accent))"
                    : "var(--bg-card)",
                  border: `2px solid ${isEditing ? "#fff" : isActive ? "var(--accent)" : "var(--border)"}`,
                  boxShadow: isActive ? "0 0 20px rgba(0,229,255,0.18)" : undefined,
                }}
              >
                <span style={{ fontSize: 26 }}>{meta.icon}</span>
                <span style={{ fontWeight: 900, fontSize: 16, color: "#fff" }}>{name}</span>
                <span
                  style={{
                    fontSize: 11,
                    color: isActive ? "rgba(255,255,255,0.75)" : "var(--text-muted)",
                  }}
                >
                  {isEditing ? `✎ ${t.voice.editing}` : name}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      {/* Sağ: DSP Drawer */}
      {drawerOpen && (
        <DspDrawer
          dsp={dsp}
          onDspChange={handleDspChange}
          onClose={() => setDrawerOpen(false)}
          activePreset={activePreset}
          onNew={handleNewPreset}
          onReset={handleResetPreset}
          onDelete={handleDeletePreset}
        />
      )}
    </div>
  );
}

