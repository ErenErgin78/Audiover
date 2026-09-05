interface ToggleSwitchProps {
  id?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  "aria-label"?: string;
}

export default function ToggleSwitch({
  id,
  checked,
  onChange,
  disabled = false,
  "aria-label": ariaLabel,
}: ToggleSwitchProps) {
  return (
    <label
      htmlFor={id}
      className={`inline-flex items-center cursor-pointer select-none shrink-0 ${
        disabled ? "opacity-50 cursor-not-allowed pointer-events-none" : ""
      }`}
      style={{ verticalAlign: "middle" }}
    >
      <input
        id={id}
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        disabled={disabled}
        aria-label={ariaLabel}
        className="sr-only peer"
      />
      <div
        className="peer-focus-visible:ring-2 peer-focus-visible:ring-[var(--accent)] peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-[var(--bg-card)]"
        style={{
          position: "relative",
          width: 36,
          height: 20,
          borderRadius: 10,
          backgroundColor: checked ? "var(--accent)" : "var(--border)",
          boxShadow: checked ? "0 0 8px rgba(0, 229, 255, 0.4)" : "none",
          transition: "background-color 0.2s ease, box-shadow 0.2s ease",
          flexShrink: 0,
        }}
      >
        <div
          style={{
            position: "absolute",
            top: 3,
            left: 3,
            width: 14,
            height: 14,
            borderRadius: "50%",
            backgroundColor: checked ? "#0F111A" : "var(--text-muted)",
            transform: checked ? "translateX(16px)" : "translateX(0px)",
            transition: "transform 0.2s cubic-bezier(0.4, 0, 0.2, 1), background-color 0.2s ease",
            pointerEvents: "none",
          }}
        />
      </div>
    </label>
  );
}
