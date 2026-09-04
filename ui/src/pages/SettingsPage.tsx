import { useI18n } from "../hooks/useI18n";

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section
      className="rounded-xl p-5 flex flex-col gap-3.5"
      style={{ background: "var(--bg-card)", border: "1px solid var(--border)" }}
    >
      <h2 style={{ color: "var(--accent)", fontWeight: 700, fontSize: 13, margin: 0 }}>{title}</h2>
      {children}
    </section>
  );
}

export default function SettingsPage() {
  const { t, language, setLanguage, languages } = useI18n();

  return (
    <div className="flex flex-col gap-5 p-6 overflow-y-auto max-w-4xl">
      <div>
        <h1 style={{ color: "var(--accent)", fontWeight: 900, fontSize: 20, letterSpacing: 1 }}>
          {t.settings.title}
        </h1>
      </div>

      {/* Language Selector */}
      <Card title={t.settings.languageCard}>
        <div className="grid grid-cols-2 gap-4 max-w-md">
          {languages.map((lang) => {
            const isSelected = language === lang.code;
            return (
              <button
                key={lang.code}
                onClick={() => setLanguage(lang.code)}
                className="flex items-center gap-3 p-4 rounded-xl transition-all cursor-pointer text-left"
                style={{
                  background: isSelected
                    ? "linear-gradient(135deg, rgba(0,229,255,0.15), rgba(124,77,255,0.15))"
                    : "var(--bg-surface)",
                  border: isSelected ? "2px solid var(--accent)" : "1px solid var(--border)",
                  boxShadow: isSelected ? "0 0 15px rgba(0,229,255,0.2)" : undefined,
                }}
              >
                <span style={{ fontSize: 28 }}>{lang.flag}</span>
                <div className="flex flex-col">
                  <span className="font-bold text-sm text-white">{lang.label}</span>
                  <span style={{ color: "var(--text-muted)", fontSize: 11 }}>
                    {lang.code === "tr" ? "Türkçe" : "English"}
                  </span>
                </div>
                {isSelected && (
                  <span className="ml-auto text-xs font-bold" style={{ color: "var(--accent)" }}>
                    ✓
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </Card>
    </div>
  );
}
