import { useEffect, useState } from "react";
import { api, type AudioDevicesState } from "../../hooks/useApi";
import { useI18n } from "../../hooks/useI18n";
import ToggleSwitch from "../ToggleSwitch";
import Select from "../Select";

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section
      className="rounded-xl p-4 flex flex-col gap-3 w-full min-w-0"
      style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
    >
      <h2 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 13, margin: 0 }}>{title}</h2>
      {children}
    </section>
  );
}

function Row({
  label,
  children,
  htmlFor,
  labelWidth = 180,
}: {
  label: string;
  children: React.ReactNode;
  htmlFor?: string;
  labelWidth?: number;
}) {
  return (
    <div className="flex items-center gap-3 w-full min-w-0">
      {htmlFor ? (
        <label
          htmlFor={htmlFor}
          className="cursor-pointer select-none shrink-0"
          style={{ color: "var(--text-muted)", fontSize: 12, minWidth: labelWidth }}
        >
          {label}
        </label>
      ) : (
        <span
          className="shrink-0"
          style={{ color: "var(--text-muted)", fontSize: 12, minWidth: labelWidth }}
        >
          {label}
        </span>
      )}
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
    <div className="flex items-center gap-2 flex-1 min-w-0">
      <input type="range" min={min} max={max} value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="flex-1 min-w-0" style={{ accentColor: "var(--accent)" }} />
      <span style={{ color: "var(--accent)", fontSize: 12, minWidth: 44, textAlign: "right", flexShrink: 0 }}>
        {format(value)}
      </span>
    </div>
  );
}

export default function AudioSettingsSection() {
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
      <div className="py-6 text-center" style={{ color: "var(--text-muted)" }}>
        {t.audio.loading}
      </div>
    );
  }

  const handleInputChange = async (val: number | string) => {
    const idx = Number(val);
    await api.setInputDevice(idx);
    setDevState((d) => d ? { ...d, current_input: idx } : d);
  };

  const handleMonitorChange = async (val: number | string) => {
    const idx = val === "null" ? null : Number(val);
    await api.setMonitorDevice(idx);
    setDevState((d) => d ? { ...d, current_monitor: idx } : d);
  };

  const handleBufferChange = async (val: number | string) => {
    const size = Number(val);
    await api.setBufferSize(size);
    setDevState((d) => d ? { ...d, block_size: size } : d);
  };

  return (
    <>
      {/* Device Selection */}
      <Card title={t.audio.hardwareCard}>
        <Row label={t.audio.inputLabel}>
          <Select
            value={devState.current_input ?? ""}
            onChange={handleInputChange}
            options={devState.inputs.map((d) => ({
              value: d.index,
              label: `[${d.index}] ${d.name}${d.is_default ? " (Default)" : ""}`,
            }))}
          />
        </Row>
        <Row label={t.audio.monitorLabel}>
          <Select
            value={devState.current_monitor ?? "null"}
            onChange={handleMonitorChange}
            options={[
              { value: "null", label: t.audio.noneDisabled },
              ...devState.outputs.map((d) => ({
                value: d.index,
                label: `[${d.index}] ${d.name}${d.is_default ? " (Default)" : ""}`,
              })),
            ]}
          />
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
          <Select
            value={devState.block_size}
            onChange={handleBufferChange}
            options={bufferOptions}
          />
        </Row>
      </Card>

      {/* Audio Levels */}
      <Card title={t.audio.levelsCard}>
        <Row label={t.audio.hearMyselfLabel} htmlFor="hear-myself-toggle" labelWidth={250}>
          <ToggleSwitch
            id="hear-myself-toggle"
            checked={devState.hear_myself}
            onChange={async (checked) => {
              await api.setHearMyself(checked);
              setDevState((d) => d ? { ...d, hear_myself: checked } : d);
            }}
          />
        </Row>
        <Row label={t.audio.hearSoundboardLabel} htmlFor="hear-soundboard-toggle" labelWidth={250}>
          <ToggleSwitch
            id="hear-soundboard-toggle"
            checked={devState.hear_soundboard}
            onChange={async (checked) => {
              await api.setHearSoundboard(checked);
              setDevState((d) => d ? { ...d, hear_soundboard: checked } : d);
            }}
          />
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
    </>
  );
}
