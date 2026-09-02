import { useEffect } from "react";
import { useAudioStore } from "./store/audioStore";
import { api } from "./hooks/useApi";
import { useMeter } from "./hooks/useMeter";
import Layout from "./components/layout/Layout";
import VoicePage from "./pages/VoicePage";
import SoundboardPage from "./pages/SoundboardPage";
import AudioSettingsPage from "./pages/AudioSettingsPage";
import HotkeysPage from "./pages/HotkeysPage";
import SettingsPage from "./pages/SettingsPage";

function ActivePage() {
  const page = useAudioStore((s) => s.activePage);
  switch (page) {
    case "voice":      return <VoicePage />;
    case "soundboard": return <SoundboardPage />;
    case "audio":      return <AudioSettingsPage />;
    case "hotkeys":    return <HotkeysPage />;
    case "settings":   return <SettingsPage />;
  }
}

export default function App() {
  const initFromState = useAudioStore((s) => s.initFromState);
  const initialized = useAudioStore((s) => s.initialized);

  // Bootstrap: fetch full state on startup
  useEffect(() => {
    api.getState().then(initFromState);
  }, [initFromState]);

  // VU Meter polling
  useMeter();

  if (!initialized) {
    return (
      <div
        className="flex items-center justify-center h-screen"
        style={{ background: "var(--bg-base)", color: "var(--accent)" }}
      >
        <div className="flex flex-col items-center gap-3">
          <span style={{ fontSize: 32 }}>🎙</span>
          <span style={{ fontWeight: 900, fontSize: 18, letterSpacing: 2 }}>AUDIOVER</span>
          <span style={{ color: "var(--text-muted)", fontSize: 12 }}>Bağlanıyor...</span>
        </div>
      </div>
    );
  }

  return (
    <Layout>
      <ActivePage />
    </Layout>
  );
}
