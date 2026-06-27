// SPDX-License-Identifier: AGPL-3.0-only

type Props = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  disabled?: boolean;
  id?: string;
};

export function Toggle({ checked, onChange, label, disabled = false, id }: Props) {
  return (
    <label
      className={`flex items-center gap-3 text-sm ${disabled ? "cursor-default opacity-60" : "cursor-pointer"}`}
    >
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => !disabled && onChange(!checked)}
        className="relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]"
        style={{ background: checked ? "var(--accent)" : "var(--border)" }}
      >
        <span
          className="inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform"
          style={{ transform: checked ? "translateX(20px)" : "translateX(2px)" }}
        />
      </button>
      {label && <span>{label}</span>}
    </label>
  );
}
