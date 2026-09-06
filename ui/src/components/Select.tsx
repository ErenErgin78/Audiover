import { useState, useRef, useEffect } from "react";

export interface SelectOption<T extends string | number = string | number> {
  value: T;
  label: string;
  disabled?: boolean;
}

export interface SelectProps<T extends string | number = string | number> {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  style?: React.CSSProperties;
  id?: string;
  "aria-label"?: string;
}

export default function Select<T extends string | number = string | number>({
  value,
  options,
  onChange,
  placeholder = "",
  disabled = false,
  className = "",
  style,
  id,
  "aria-label": ariaLabel,
}: SelectProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const selectedOption = options.find((opt) => opt.value === value);
  const displayText = selectedOption ? selectedOption.label : (placeholder || String(value ?? ""));

  useEffect(() => {
    if (!isOpen) return;

    const handleOutsideClick = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setIsOpen(false);
      }
    };

    document.addEventListener("mousedown", handleOutsideClick);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleOutsideClick);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen]);

  const handleSelect = (option: SelectOption<T>) => {
    if (option.disabled) return;
    onChange(option.value);
    setIsOpen(false);
  };

  return (
    <div
      ref={containerRef}
      className={`relative flex-1 min-w-0 ${isOpen ? "z-30" : "z-10"} ${className}`}
      style={style}
    >
      <button
        id={id}
        type="button"
        disabled={disabled}
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        onClick={() => !disabled && setIsOpen((prev) => !prev)}
        className="w-full flex items-center justify-between gap-2 rounded-lg text-left transition-all cursor-pointer select-none"
        style={{
          background: "var(--bg-surface)",
          border: isOpen ? "1px solid var(--accent)" : "1px solid var(--border)",
          color: "var(--text)",
          padding: "6px 10px",
          fontSize: 12,
          outline: "none",
          boxShadow: isOpen ? "0 0 0 1px var(--accent)" : "none",
          opacity: disabled ? 0.5 : 1,
          cursor: disabled ? "not-allowed" : "pointer",
        }}
        onMouseEnter={(e) => {
          if (!isOpen && !disabled) e.currentTarget.style.borderColor = "var(--border-hover)";
        }}
        onMouseLeave={(e) => {
          if (!isOpen && !disabled) e.currentTarget.style.borderColor = "var(--border)";
        }}
      >
        <span className="truncate flex-1" title={displayText}>
          {displayText}
        </span>
        <svg
          className="shrink-0 transition-transform duration-200"
          style={{
            transform: isOpen ? "rotate(180deg)" : "rotate(0deg)",
            color: isOpen ? "var(--accent)" : "var(--text-muted)",
          }}
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>

      {isOpen && (
        <div
          role="listbox"
          className="absolute left-0 right-0 top-full mt-1.5 py-1 rounded-lg overflow-y-auto max-h-56 z-50 select-none shadow-2xl"
          style={{
            background: "#161824",
            border: "1px solid var(--border)",
            boxShadow: "0 8px 24px rgba(0, 0, 0, 0.6)",
          }}
        >
          {options.map((option) => {
            const isSelected = option.value === value;
            return (
              <div
                key={String(option.value)}
                role="option"
                aria-selected={isSelected}
                onClick={() => handleSelect(option)}
                className={`px-3 py-2 text-xs flex items-center justify-between gap-2 transition-colors ${
                  option.disabled
                    ? "opacity-40 cursor-not-allowed"
                    : "cursor-pointer"
                }`}
                style={{
                  background: isSelected
                    ? "rgba(0, 229, 255, 0.12)"
                    : "transparent",
                  color: isSelected
                    ? "var(--accent)"
                    : option.disabled
                    ? "var(--text-dim)"
                    : "var(--text)",
                  fontWeight: isSelected ? 600 : 400,
                }}
                onMouseEnter={(e) => {
                  if (!isSelected && !option.disabled) {
                    e.currentTarget.style.background = "var(--bg-surface)";
                    e.currentTarget.style.color = "var(--text)";
                  }
                }}
                onMouseLeave={(e) => {
                  if (!isSelected && !option.disabled) {
                    e.currentTarget.style.background = "transparent";
                    e.currentTarget.style.color = "var(--text)";
                  }
                }}
              >
                <span className="truncate flex-1" title={option.label}>
                  {option.label}
                </span>
                {isSelected && (
                  <span className="shrink-0 text-xs font-bold" style={{ color: "var(--accent)" }}>
                    ✓
                  </span>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
