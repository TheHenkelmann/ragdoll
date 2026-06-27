// SPDX-License-Identifier: AGPL-3.0-only

import { useMemo, useState, useId } from "react";
import { evaluatePassword, generatePassword } from "../utils/password";

type Props = {
  value: string;
  onChange: (value: string) => void;
  id?: string;
  label?: string;
  required?: boolean;
  autoComplete?: string;
  showStrength?: boolean;
  showGenerate?: boolean;
  disabled?: boolean;
};

function EyeIcon({ off }: { off?: boolean }) {
  if (off) {
    return (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
        <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
        <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
        <line x1="1" y1="1" x2="23" y2="23" />
      </svg>
    );
  }
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

export function PasswordField({
  value,
  onChange,
  id,
  label,
  required,
  autoComplete,
  showStrength = false,
  showGenerate = false,
  disabled = false,
}: Props) {
  const autoId = useId();
  const inputId = id ?? autoId;
  const [visible, setVisible] = useState(false);
  const evaluation = useMemo(() => (showStrength ? evaluatePassword(value) : null), [showStrength, value]);

  return (
    <label className="block space-y-1 text-sm" htmlFor={inputId}>
      {label && <span>{label}</span>}
      <div className="relative flex items-center gap-2">
        <input
          id={inputId}
          className="input pr-10"
          type={visible ? "text" : "password"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          required={required}
          autoComplete={autoComplete}
          disabled={disabled}
          aria-label={label}
        />
        <button
          type="button"
          className="absolute right-2 rounded-md p-1 text-[var(--muted)] hover:text-[var(--text)]"
          onClick={() => setVisible((v) => !v)}
          aria-label={visible ? "Hide password" : "Show password"}
          tabIndex={-1}
        >
          <EyeIcon off={visible} />
        </button>
      </div>
      {showStrength && evaluation && (
        <div className="space-y-2 pt-1">
          <div
            className="h-1.5 overflow-hidden rounded-full"
            style={{ background: "var(--border)" }}
            aria-hidden
          >
            <div
              className="h-full transition-all"
              style={{
                width: `${Math.round(evaluation.score * 100)}%`,
                background: evaluation.isValid ? "var(--positive)" : "var(--danger)",
              }}
            />
          </div>
          <ul className="space-y-0.5 text-xs text-[var(--muted)]">
            {evaluation.checks.map((check) => (
              <li key={check.id} style={{ color: check.passed ? "var(--positive)" : undefined }}>
                {check.passed ? "✓" : "○"} {check.label}
              </li>
            ))}
          </ul>
        </div>
      )}
      {showGenerate && (
        <button
          type="button"
          className="btn-secondary text-xs"
          onClick={() => onChange(generatePassword())}
          disabled={disabled}
        >
          Generate strong password
        </button>
      )}
    </label>
  );
}
