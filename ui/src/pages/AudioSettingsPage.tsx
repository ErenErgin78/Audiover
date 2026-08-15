import { useEffect, useState } from "react";
import { api, type AudioDevicesState } from "../hooks/useApi";

const BUFFER_OPTIONS = [
  { label: "128 samples (~2.7 ms)", value: 128 },
  { label: "256 samples (~5.3 ms) [Önerilen]", value: 256 },
  { label: "512 samples (~10.7 ms)", value: 512 },
  { label: "1024 samples (~21.3 ms)", value: 1024 },
];

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

  const load = async () => {
    const d = await api.getAudioDevices();
    setDevState(d);
  };

  useEffect(() => { load(); }, []);

  if (!devState) {
    return (
      <div className="flex items-center justify-center h-full" style={{ color: "var(--text-muted)" }}>
        Yükleniyor...
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
    <div className="flex flex-col gap-4 p-6 overflow-y-auto">
      <h1 style={{ color: "var(--accent)", fontWeight: 900, fontSize: 20, letterSpacing: 1 }}>
        Audio & Routing
      </h1>

      {/* Device Selection */}
      <Card title="Audio Devices & Hardware I/O">
        <Row label="Fiziksel Mikrofon Girişi:">
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
        <Row label="Kulaklık / Monitör Çıkışı:">
          <select
            value={devState.current_monitor ?? "null"}
            onChange={(e) => handleMonitorChange(e.target.value)}
            style={selectStyle}
          >
            <option value="null">Devre Dışı / Yok</option>
            {devState.outputs.map((d) => (
              <option key={d.index} value={d.index}>
                [{d.index}] {d.name}{d.is_default ? " (Default)" : ""}
              </option>
            ))}
          </select>
        </Row>
        <button
          onClick={load}
          className="self-start px-4 py-1.5 rounded-lg text-xs font-semibold"
          style={{ background: "var(--bg-surface)", border: "1px solid var(--border)", color: "var(--text)" }}
        >
          🔄 Cihazları Yenile
        </button>
      </Card>

      {/* PipeWire Status */}
      <Card title="PipeWire Virtual Audio Routing">
        <div style={{ color: "var(--text-muted)", fontSize: 12, lineHeight: 1.8 }}>
          <div>
            <strong style={{ color: "var(--text)" }}>Sanal Sink:</strong>{" "}
            <code style={{ color: "var(--accent)", background: "var(--bg-surface)", padding: "1px 6px", borderRadius: 4 }}>
              Audiover_Sink
            </code>
          </div>
          <div>
            <strong style={{ color: "var(--text)" }}>Sanal Mikrofon:</strong>{" "}
            <code style={{ color: "var(--accent)", background: "var(--bg-surface)", padding: "1px 6px", borderRadius: 4 }}>
              Audiover_Virtual_Microphone
            </code>
          </div>
          <div style={{ marginTop: 6 }}>
            Discord, OBS, oyunlarda mikrofon olarak{" "}
            <strong>Audiover_Virtual_Microphone</strong>'ı seç.
          </div>
        </div>
      </Card>

      {/* Latency */}
      <Card title="Latency & Performance">
        <Row label="Buffer Size (Frames):">
          <select
            value={devState.block_size}
            onChange={(e) => handleBufferChange(e.target.value)}
            style={selectStyle}
          >
            {BUFFER_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>{o.label}</option>
            ))}
          </select>
        </Row>
      </Card>

      {/* Audio Levels */}
      <Card title="Audio Levels & Monitoring">
        <Row label="Hear Myself (Loopback):">
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
            <span style={{ color: "var(--text-muted)", fontSize: 12 }}>Kendi sesini kulaklıktan duy</span>
          </label>
        </Row>
        <Row label="Hear Soundboard:">
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
            <span style={{ color: "var(--text-muted)", fontSize: 12 }}>Soundboard seslerini kulaklıktan duy</span>
          </label>
        </Row>
        <Row label="Mikrofon Girişi Ses:">
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
        <Row label="Kulaklık Monitör Ses:">
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
