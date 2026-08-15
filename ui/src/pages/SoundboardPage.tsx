import { useEffect, useRef, useState } from "react";
import { useAudioStore } from "../store/audioStore";
import { api, type Sound } from "../hooks/useApi";

const PROGRESS_INTERVAL_MS = 40;

/* ── Key Normalizer ──────────────────────────────────────────── */
function normalizeKey(e: KeyboardEvent): string | null {
  // Ignore lone modifier keys
  if (["Control", "Shift", "Alt", "Meta", "AltGraph", "OS"].includes(e.key)) {
    return null;
  }

  const code = e.code;

  // Function keys F1 - F12
  if (/^F([1-9]|1[0-2])$/.test(code)) {
    return code;
  }

  // Digits 0 - 9
  if (/^Digit([0-9])$/.test(code)) {
    return code.replace("Digit", "");
  }

  // Letters A - Z
  if (/^Key([A-Z])$/i.test(code)) {
    return code.replace("Key", "").toUpperCase();
  }

  // Numpad
  if (/^Numpad([0-9])$/.test(code)) {
    return "KP" + code.replace("Numpad", "");
  }
  if (code === "NumpadAdd") return "KPPLUS";
  if (code === "NumpadSubtract") return "KPMINUS";
  if (code === "NumpadMultiply") return "KPASTERISK";
  if (code === "NumpadDivide") return "KPSLASH";
  if (code === "NumpadEnter") return "KPENTER";
  if (code === "NumpadDecimal") return "KPDOT";

  // Common keys
  if (code === "Space" || e.key === " ") return "SPACE";
  if (code === "Tab" || e.key === "Tab") return "TAB";
  if (code === "Enter" || e.key === "Enter") return "ENTER";
  if (code === "Backspace" || e.key === "Backspace") return "BACKSPACE";
  if (code === "Delete" || e.key === "Delete") return "DELETE";
  if (code === "Insert" || e.key === "Insert") return "INSERT";
  if (code === "Home" || e.key === "Home") return "HOME";
  if (code === "End" || e.key === "End") return "END";
  if (code === "PageUp" || e.key === "PageUp") return "PAGEUP";
  if (code === "PageDown" || e.key === "PageDown") return "PAGEDOWN";
  if (code === "ArrowUp" || e.key === "ArrowUp") return "UP";
  if (code === "ArrowDown" || e.key === "ArrowDown") return "DOWN";
  if (code === "ArrowLeft" || e.key === "ArrowLeft") return "LEFT";
  if (code === "ArrowRight" || e.key === "ArrowRight") return "RIGHT";
  if (code === "Minus" || e.key === "-") return "MINUS";
  if (code === "Equal" || e.key === "=") return "EQUAL";
  if (code === "CapsLock" || e.key === "CapsLock") return "CAPSLOCK";

  // Fallback for single characters (e.g. punctuation, international keys)
  if (e.key && e.key.length === 1) {
    return e.key.toUpperCase();
  }

  return null;
}

/* ── Hotkey Modal ────────────────────────────────────────────── */
interface HotkeyModalProps {
  sound: Sound;
  onClose: () => void;
  onSave: (hotkey: string) => void;
}

function HotkeyModal({ sound, onClose, onSave }: HotkeyModalProps) {
  const [selectedKey, setSelectedKey] = useState<string>(sound.hotkey || "");
  const [isListening, setIsListening] = useState<boolean>(true);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        onClose();
        return;
      }

      const key = normalizeKey(e);
      if (key) {
        setSelectedKey(key);
        setIsListening(false);
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [onClose]);

  const handleClear = () => {
    onSave("");
    onClose();
  };

  const handleConfirm = () => {
    onSave(selectedKey);
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      style={{
        backgroundColor: "rgba(0, 0, 0, 0.75)",
        backdropFilter: "blur(8px)",
      }}
      onClick={onClose}
    >
      <div
        className="w-full max-w-sm rounded-2xl p-5 flex flex-col gap-4 relative shadow-2xl"
        style={{
          background: "var(--bg-card)",
          border: "1px solid var(--border-hover)",
          boxShadow: "0 10px 40px rgba(0, 0, 0, 0.7), 0 0 25px rgba(0, 229, 255, 0.15)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col min-w-0 pr-2">
            <span style={{ color: "var(--accent)", fontSize: 11, fontWeight: 700, letterSpacing: 0.5, textTransform: "uppercase" }}>
              Kısayol Tuşu Ata
            </span>
            <h2 className="text-sm font-bold text-white truncate" title={sound.name}>
              {sound.name}
            </h2>
          </div>
          <button
            onClick={onClose}
            className="w-7 h-7 rounded-lg flex items-center justify-center transition-colors text-xs shrink-0 cursor-pointer"
            style={{ background: "var(--bg-surface)", color: "var(--text-muted)" }}
          >
            ✕
          </button>
        </div>

        {/* Key Capture Area */}
        <div
          onClick={() => setIsListening(true)}
          className="flex flex-col items-center justify-center py-6 px-4 rounded-xl cursor-pointer transition-all gap-2.5"
          style={{
            background: "var(--bg-base)",
            border: isListening
              ? "2px solid var(--accent)"
              : "2px solid var(--border)",
            boxShadow: isListening ? "0 0 16px rgba(0, 229, 255, 0.25)" : "none",
          }}
        >
          {selectedKey ? (
            <kbd
              className="px-5 py-2.5 rounded-xl font-mono font-black text-xl tracking-wider"
              style={{
                background: "linear-gradient(135deg, #1f253d, #141724)",
                border: "2px solid var(--accent)",
                color: "var(--accent)",
                boxShadow: "0 4px 12px rgba(0, 229, 255, 0.3)",
              }}
            >
              {selectedKey}
            </kbd>
          ) : (
            <div
              className="w-12 h-12 rounded-xl flex items-center justify-center border border-dashed"
              style={{ borderColor: "var(--text-dim)", color: "var(--text-muted)" }}
            >
              <span className="text-xl">⌨️</span>
            </div>
          )}

          <div className="text-center">
            <p className="text-xs font-semibold" style={{ color: isListening ? "var(--accent)" : "var(--text)" }}>
              {isListening ? "Klavyeden tek bir tuşa basın..." : "Tuş seçildi (Değiştirmek için tıkla)"}
            </p>
            <p className="text-[11px] mt-0.5" style={{ color: "var(--text-muted)" }}>
              Örn: F1 - F12, 1 - 9, A - Z, SPACE
            </p>
          </div>
        </div>

        {/* Footer Actions */}
        <div className="flex items-center gap-2 pt-1">
          {sound.hotkey && (
            <button
              onClick={handleClear}
              type="button"
              className="px-2.5 py-1.5 rounded-lg text-xs font-bold transition-opacity hover:opacity-80 cursor-pointer"
              style={{
                background: "rgba(255, 23, 68, 0.15)",
                color: "var(--red)",
                border: "1px solid rgba(255, 23, 68, 0.3)",
              }}
            >
              Kaldır
            </button>
          )}

          <div className="flex-1" />

          <button
            onClick={onClose}
            type="button"
            className="px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer"
            style={{ background: "var(--bg-surface)", color: "var(--text-muted)" }}
          >
            Vazgeç
          </button>

          <button
            onClick={handleConfirm}
            disabled={!selectedKey}
            type="button"
            className="px-4 py-1.5 rounded-lg text-xs font-bold transition-all disabled:opacity-40 cursor-pointer"
            style={{
              background: "linear-gradient(90deg, var(--accent2), var(--accent))",
              color: "#fff",
              boxShadow: selectedKey ? "0 0 10px rgba(0, 229, 255, 0.3)" : "none",
            }}
          >
            Kaydet
          </button>
        </div>
      </div>
    </div>
  );
}

/* ── Sound Card ──────────────────────────────────────────────── */
interface SoundCardProps {
  sound: Sound;
  progress: number;
  isPlaying: boolean;
  onRemove: (id: string) => void;
  onUpdate: (id: string, patch: Partial<Pick<Sound, "volume" | "loop" | "hotkey">>) => void;
  onAssignHotkey: (sound: Sound) => void;
}

function SoundCard({ sound, progress, isPlaying, onRemove, onUpdate, onAssignHotkey }: SoundCardProps) {
  const handlePlay = () => {
    if (isPlaying) api.pauseSound(sound.id);
    else api.playSound(sound.id);
  };

  const handleStop = () => api.stopSound(sound.id);

  return (
    <div
      className="flex flex-col gap-2 rounded-xl p-3 transition-all"
      style={{
        background: "var(--bg-card)",
        border: "1px solid var(--border)",
      }}
      onMouseEnter={(e) => (e.currentTarget.style.borderColor = "var(--accent2)")}
      onMouseLeave={(e) => (e.currentTarget.style.borderColor = "var(--border)")}
    >
      {/* Header */}
      <div className="flex items-center gap-2">
        <span className="font-bold text-sm flex-1 truncate" style={{ color: "#fff" }}>
          {sound.name}
        </span>
        <button
          onClick={() => onAssignHotkey(sound)}
          className="px-2 py-0.5 rounded text-xs font-mono cursor-pointer transition-all hover:scale-105"
          style={{
            background: sound.hotkey ? "rgba(0, 229, 255, 0.15)" : "#262B42",
            border: sound.hotkey ? "1px solid var(--accent)" : "1px solid transparent",
            color: "var(--accent)",
            fontSize: 11,
          }}
          title="Kısayol tuşu ata / değiştir"
        >
          {sound.hotkey || "+ Key"}
        </button>
      </div>

      {/* Progress Bar */}
      <div
        className="overflow-hidden rounded-sm"
        style={{ height: 4, background: "var(--border)" }}
      >
        <div
          style={{
            height: "100%",
            width: `${progress * 100}%`,
            background: isPlaying
              ? "linear-gradient(90deg, var(--accent2), var(--accent))"
              : "var(--border-hover)",
            transition: "width 0.04s linear",
          }}
        />
      </div>

      {/* Controls */}
      <div className="flex items-center gap-1.5">
        <button
          onClick={handlePlay}
          className="flex-1 py-1 rounded-lg text-xs font-bold transition-colors cursor-pointer"
          style={{
            background: isPlaying ? "var(--yellow)" : "var(--green)",
            color: "#000",
          }}
        >
          {isPlaying ? "⏸ Pause" : "▶ Play"}
        </button>
        <button
          onClick={handleStop}
          className="py-1 px-2.5 rounded-lg text-xs font-bold cursor-pointer"
          style={{ background: "var(--bg-surface)", color: "var(--text)" }}
        >
          ■
        </button>
        <label className="flex items-center gap-1 cursor-pointer text-xs" style={{ color: "var(--text-muted)" }}>
          <input
            type="checkbox"
            checked={sound.loop}
            onChange={(e) => onUpdate(sound.id, { loop: e.target.checked })}
            className="accent-[var(--accent)] cursor-pointer"
          />
          Loop
        </label>
        <button
          onClick={() => {
            if (confirm(`'${sound.name}' silinsin mi?`)) onRemove(sound.id);
          }}
          className="ml-auto text-sm cursor-pointer"
          style={{ color: "var(--red)", background: "transparent", border: "none" }}
        >
          ✕
        </button>
      </div>

      {/* Volume Slider */}
      <div className="flex items-center gap-2">
        <span style={{ color: "var(--text-muted)", fontSize: 11 }}>Vol</span>
        <input
          type="range"
          min={0}
          max={150}
          value={Math.round(sound.volume * 100)}
          onChange={(e) => onUpdate(sound.id, { volume: Number(e.target.value) / 100 })}
          className="flex-1"
          style={{ accentColor: "var(--accent)" }}
        />
        <span style={{ color: "var(--text-muted)", fontSize: 11, minWidth: 36, textAlign: "right" }}>
          {Math.round(sound.volume * 100)}%
        </span>
      </div>
    </div>
  );
}

function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

/* ── SoundboardPage ──────────────────────────────────────────── */
export default function SoundboardPage() {
  const sounds = useAudioStore((s) => s.sounds);
  const setSounds = useAudioStore((s) => s.setSounds);
  const upsertSound = useAudioStore((s) => s.upsertSound);
  const removeSound = useAudioStore((s) => s.removeSound);

  const [search, setSearch] = useState("");
  const [progress, setProgress] = useState<Record<string, { is_playing: boolean; progress: number }>>({});
  const [assigningSound, setAssigningSound] = useState<Sound | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const dragCounter = useRef(0);

  // Load sounds on mount
  useEffect(() => {
    api.getSounds().then(setSounds);
  }, [setSounds]);

  // Progress polling
  useEffect(() => {
    intervalRef.current = setInterval(async () => {
      const data = await api.getAllProgress();
      setProgress(data);
    }, PROGRESS_INTERVAL_MS);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, []);

  const handleAdd = async () => {
    const res = await api.addSoundFile();
    if (res.ok && res.sound) upsertSound(res.sound);
    else if (res.error) alert(res.error);
  };

  const handleDragEnter = (e: React.DragEvent) => {
    e.preventDefault();
    dragCounter.current += 1;
    if (e.dataTransfer.items && e.dataTransfer.items.length > 0) {
      setIsDragging(true);
    }
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    dragCounter.current -= 1;
    if (dragCounter.current <= 0) {
      dragCounter.current = 0;
      setIsDragging(false);
    }
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    dragCounter.current = 0;
    setIsDragging(false);

    const files = Array.from(e.dataTransfer.files);
    for (const file of files) {
      const isMedia =
        file.type.startsWith("audio/") ||
        file.type.startsWith("video/") ||
        /\.(mp3|wav|ogg|flac|m4a|aac|mp4|mkv|mov|webm|opus)$/i.test(file.name);
      if (isMedia) {
        try {
          const b64 = await readFileAsBase64(file);
          const res = await api.addSoundData(file.name, b64);
          if (res.ok && res.sound) upsertSound(res.sound);
          else if (res.error) alert(res.error);
        } catch (err) {
          console.error("Drop sound error:", err);
        }
      }
    }
  };

  const handleRemove = async (id: string) => {
    await api.removeSound(id);
    removeSound(id);
  };

  const handleUpdate = async (id: string, patch: Partial<Pick<Sound, "volume" | "loop" | "hotkey">>) => {
    await api.updateSound(id, patch);
    const updated = sounds.find((s) => s.id === id);
    if (updated) upsertSound({ ...updated, ...patch });
  };

  const handleSaveHotkey = (hotkey: string) => {
    if (assigningSound) {
      handleUpdate(assigningSound.id, { hotkey });
    }
  };

  const filtered = search
    ? sounds.filter((s) => s.name.toLowerCase().includes(search.toLowerCase()))
    : sounds;

  return (
    <div
      className="flex flex-col h-full p-4 gap-4 relative"
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      {/* Drag & Drop Visual Overlay */}
      {isDragging && (
        <div
          className="absolute inset-0 z-40 flex flex-col items-center justify-center p-6 pointer-events-none"
          style={{
            backgroundColor: "rgba(15, 17, 26, 0.85)",
            backdropFilter: "blur(6px)",
          }}
        >
          <div
            className="w-full h-full rounded-2xl flex flex-col items-center justify-center gap-4 border-2 border-dashed"
            style={{
              borderColor: "var(--accent)",
              background: "rgba(0, 229, 255, 0.05)",
              boxShadow: "0 0 30px rgba(0, 229, 255, 0.2) inset",
            }}
          >
            <span className="text-5xl animate-bounce">📥</span>
            <p className="text-base font-bold text-white tracking-wide">
              Ses Dosyalarını Buraya Bırakın
            </p>
            <p className="text-xs" style={{ color: "var(--text-muted)" }}>
              MP3, WAV, OGG, FLAC, AAC, MP4 vb. desteklenir
            </p>
          </div>
        </div>
      )}

      {/* Toolbar */}
      <div className="flex items-center gap-3">
        <button
          onClick={handleAdd}
          className="px-4 py-2 rounded-lg text-xs font-bold cursor-pointer transition-opacity hover:opacity-90"
          style={{
            background: "linear-gradient(90deg, var(--accent2), var(--accent))",
            color: "#fff",
          }}
        >
          ➕ Ses Dosyası Ekle
        </button>
        <button
          onClick={() => api.stopAllSounds()}
          className="px-4 py-2 rounded-lg text-xs font-bold cursor-pointer transition-opacity hover:opacity-90"
          style={{ background: "var(--red)", color: "#fff" }}
        >
          ⏹ TÜM SESLERİ DURDUR
        </button>
        <div className="flex-1" />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="🔍 Ses ara..."
          className="px-3 py-2 rounded-lg text-xs"
          style={{
            background: "var(--bg-surface)",
            border: "1px solid var(--border)",
            color: "var(--text)",
            outline: "none",
            width: 200,
          }}
        />
      </div>

      {/* Grid */}
      {filtered.length === 0 ? (
        <div
          className="flex-1 flex flex-col items-center justify-center gap-3"
          style={{ color: "var(--text-muted)" }}
        >
          <span style={{ fontSize: 40 }}>🔊</span>
          <p className="text-sm">Henüz ses yok. Yukarıdan dosya ekle.</p>
        </div>
      ) : (
        <div
          className="grid gap-3 overflow-y-auto pb-2"
          style={{ gridTemplateColumns: "repeat(auto-fill, minmax(260px, 1fr))" }}
        >
          {filtered.map((sound) => {
            const prog = progress[sound.id];
            return (
              <SoundCard
                key={sound.id}
                sound={sound}
                progress={prog?.progress ?? 0}
                isPlaying={prog?.is_playing ?? false}
                onRemove={handleRemove}
                onUpdate={handleUpdate}
                onAssignHotkey={(s) => setAssigningSound(s)}
              />
            );
          })}
        </div>
      )}

      {/* Modern Hotkey Capture Modal */}
      {assigningSound && (
        <HotkeyModal
          sound={assigningSound}
          onClose={() => setAssigningSound(null)}
          onSave={handleSaveHotkey}
        />
      )}
    </div>
  );
}
