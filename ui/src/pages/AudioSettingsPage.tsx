import { useEffect, useState } from "react";
import { api, type AudioDevicesState } from "../hooks/useApi";
import { useI18n } from "../hooks/useI18n";

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section
      className="rounded-xl p-4 flex flex-col gap-3"
      style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
    >
      <h2 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 13, margin: 0 }}>{title}</h2>
      {children}
    </section>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3">
      <span style={{ color: "var(--text-muted)", fontSize: 12, minWidth: 180 }}>{label}</span>
      {children}
    </div>
  );
}

function SliderWithLabel({ value, min, max, onChange, format }: {
  value: number; min: number; max: number;
  onChange: (v: number) => void;
  format: (v: number) => string;
}) {
  return (
    <div className="flex items-center gap-2 flex-1">
      <input type="range" min={min} max={max} value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="flex-1" style={{ accentColor: "var(--accent)" }} />
      <span style={{ color: "var(--accent)", fontSize: 12, minWidth: 44, textAlign: "right" }}>
        {format(value)}
      </span>
    </div>
  );
}

export default function AudioSettingsPage() {
  const [devState, setDevState] = useState<AudioDevicesState | null>(null);
  const { t } = useI18n();

  const bufferOptions = [
    { label: `128 samples (~2.7 ms)`, value: 128 },
    { label: `256 samples (~5.3 ms) [${t.audio.recommended}]`, value: 256 },
    { label: `512 samples (~10.7 ms)`, value: 512 },
    { label: `1024 samples (~21.3 ms)`, value: 1024 },
  ];

  const load = async () => {
    const d = await api.getAudioDevices();
    setDevState(d);
  };

  useEffect(() => { load(); }, []);

  if (!devState) {
    return (
      <div className="flex items-center justify-center h-full" style={{ color: "var(--text-muted)" }}>
        {t.audio.loading}
      </div>
    );
  }

  const handleInputChange = async (idxStr: string) => {
    const idx = Number(idxStr);
    await api.setInputDevice(idx);
    setDevState((d) => d ? { ...d, current_input: idx } : d);
  };

  const handleMonitorChange = async (idxStr: string) => {
    const idx = idxStr === "null" ? null : Number(idxStr);
    await api.setMonitorDevice(idx);
    setDevState((d) => d ? { ...d, current_monitor: idx } : d);
  };

  const handleBufferChange = async (sizeStr: string) => {
    const size = Number(sizeStr);
    await api.setBufferSize(size);
    setDevState((d) => d ? { ...d, block_size: size } : d);
  };

  const selectStyle = {
    background: "var(--bg-surface)",
    border: "1px solid var(--border)",
    color: "var(--text)",
    borderRadius: 8,
    padding: "6px 10px",
    fontSize: 12,
    flex: 1,
    outline: "none",
  } as React.CSSProperties;

  return (
    <div className="flex flex-col gap-4 p-6 overflow-y-auto max-w-4xl">
      <h1 style={{ color: "var(--accent)", fontWeight: 900, fontSize: 20, letterSpacing: 1 }}>
        {t.audio.title}
      </h1>

      {/* Device Selection */}
      <Card title={t.audio.hardwareCard}>
        <Row label={t.audio.inputLabel}>
          <select
            value={devState.current_input ?? ""}
            onChange={(e) => handleInputChange(e.target.value)}
            style={selectStyle}
          >
            {devState.inputs.map((d) => (
              <option key={d.index} value={d.index}>
                [{d.index}] {d.name}{d.is_default ? " (Default)" : ""}
              </option>
            ))}
          </select>
        </Row>
        <Row label={t.audio.monitorLabel}>
          <select
            value={devState.current_monitor ?? "null"}
            onChange={(e) => handleMonitorChange(e.target.value)}
            style={selectStyle}
          >
            <option value="null">{t.audio.noneDisabled}</option>
            {devState.outputs.map((d) => (
              <option key={d.index} value={d.index}>
                [{d.index}] {d.name}{d.is_default ? " (Default)" : ""}
              </option>
            ))}
          </select>
        </Row>
        <button
          onClick={load}
          className="self-start px-4 py-1.5 rounded-lg text-xs font-semibold cursor-pointer"
          style={{ background: "var(--bg-surface)", border: "1px solid var(--border)", color: "var(--text)" }}
        >
          {t.audio.refreshDevices}
        </button>
      </Card>

      {/* PipeWire Status */}
      <Card title={t.audio.routingCard}>
        <div style={{ color: "var(--text-muted)", fontSize: 12, lineHeight: 1.8 }}>
          <div>
            <strong style={{ color: "var(--text)" }}>{t.audio.virtualSink}</strong>{" "}
            <code style={{ color: "var(--accent)", background: "var(--bg-surface)", padding: "1px 6px", borderRadius: 4 }}>
              Audiover_Sink
            </code>
          </div>
          <div>
            <strong style={{ color: "var(--text)" }}>{t.audio.virtualMic}</strong>{" "}
            <code style={{ color: "var(--accent)", background: "var(--bg-surface)", padding: "1px 6px", borderRadius: 4 }}>
              Audiover_Virtual_Microphone
            </code>
          </div>
          <div style={{ marginTop: 6 }}>
            {t.audio.routingHelp}
          </div>
        </div>
      </Card>

      {/* Latency */}
      <Card title={t.audio.latencyCard}>
        <Row label={t.audio.bufferSize}>
          <select
            value={devState.block_size}
            onChange={(e) => handleBufferChange(e.target.value)}
            style={selectStyle}
          >
            {bufferOptions.map((o) => (
              <option key={o.value} value={o.value}>{o.label}</option>
            ))}
          </select>
        </Row>
      </Card>

      {/* Audio Levels */}
      <Card title={t.audio.levelsCard}>
        <Row label={t.audio.hearMyselfLabel}>
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={devState.hear_myself}
              onChange={async (e) => {
                await api.setHearMyself(e.target.checked);
                setDevState((d) => d ? { ...d, hear_myself: e.target.checked } : d);
              }}
              className="w-4 h-4 accent-[var(--accent)]"
            />
            <span style={{ color: "var(--text-muted)", fontSize: 12 }}>{t.audio.hearMyselfLabel}</span>
          </label>
        </Row>
        <Row label={t.audio.hearSoundboardLabel}>
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={devState.hear_soundboard}
              onChange={async (e) => {
                await api.setHearSoundboard(e.target.checked);
                setDevState((d) => d ? { ...d, hear_soundboard: e.target.checked } : d);
              }}
              className="w-4 h-4 accent-[var(--accent)]"
            />
            <span style={{ color: "var(--text-muted)", fontSize: 12 }}>{t.audio.hearSoundboardLabel}</span>
          </label>
        </Row>
        <Row label={t.audio.micGainLabel}>
          <SliderWithLabel
            value={Math.round(devState.mic_gain * 100)} min={0} max={200}
            format={(v) => `${v}%`}
            onChange={async (v) => {
              const gain = v / 100;
              await api.setMicGain(gain);
              setDevState((d) => d ? { ...d, mic_gain: gain } : d);
            }}
          />
        </Row>
        <Row label={t.audio.monitorGainLabel}>
          <SliderWithLabel
            value={Math.round(devState.monitor_gain * 100)} min={0} max={200}
            format={(v) => `${v}%`}
            onChange={async (v) => {
              const gain = v / 100;
              await api.setMonitorGain(gain);
              setDevState((d) => d ? { ...d, monitor_gain: gain } : d);
            }}
          />
        </Row>
      </Card>
    </div>
  );
}
