// SPDX-License-Identifier: AGPL-3.0-only

import { ReactNode, useEffect, useRef, useState } from "react";

type Props = {
  open: boolean;
  typeLabel: "release" | "stage";
  tag: string;
  onClose: () => void;
  onConfirm: () => void | Promise<void>;
};

function EyeIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

export function ViewButton({ onClick }: { onClick: () => void }) {
  return (
    <button type="button" className="btn-view" onClick={onClick}>
      <EyeIcon />
      View
    </button>
  );
}

export function EnterIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <path d="M9 10l-4 4 4 4" />
      <path d="M20 4v7a4 4 0 0 1-4 4H5" />
    </svg>
  );
}

export function EscBadge() {
  return <span className="text-sm font-semibold tracking-wide">Esc</span>;
}

function ForkIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <circle cx="6" cy="5" r="2.5" />
      <circle cx="18" cy="5" r="2.5" />
      <circle cx="12" cy="19" r="2.5" />
      <path d="M6 7.5v3a3 3 0 0 0 3 3h6a3 3 0 0 0 3-3v-3" />
      <path d="M12 13.5v3" />
    </svg>
  );
}

function PencilIcon() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4z" />
    </svg>
  );
}

export function DeleteConfirmDialog({ open, typeLabel, tag, onClose, onConfirm }: Props) {
  const [confirmText, setConfirmText] = useState("");
  const [busy, setBusy] = useState(false);
  const expected = `${typeLabel}/${tag}`;

  useEffect(() => {
    if (open) setConfirmText("");
  }, [open, tag]);

  if (!open) return null;

  async function confirm() {
    setBusy(true);
    try {
      await onConfirm();
      onClose();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      onClick={onClose}
    >
      <div
        className="card w-full max-w-md space-y-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-lg font-semibold">Delete {typeLabel}?</h3>
        <p className="text-sm text-[var(--muted)]">
          This action cannot be undone. Type <code>{expected}</code> to confirm.
        </p>
        <input
          className="input"
          value={confirmText}
          onChange={(e) => setConfirmText(e.target.value)}
          placeholder={expected}
          autoFocus
        />
        <div className="flex justify-end gap-2">
          <button type="button" className="btn-secondary" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button
            type="button"
            className="btn-danger"
            disabled={confirmText !== expected || busy}
            onClick={() => void confirm()}
          >
            Delete
          </button>
        </div>
      </div>
    </div>
  );
}

export function InlineTagInput({
  initial = "",
  maxLength,
  onSubmit,
  onCancel,
}: {
  initial?: string;
  maxLength: number;
  onSubmit: (tag: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [tag, setTag] = useState(initial);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  async function submit() {
    const trimmed = tag.trim();
    if (!trimmed || busy) return;
    setBusy(true);
    try {
      await onSubmit(trimmed);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="create-input-wrap shrink-0">
      <input
        ref={inputRef}
        className="input min-w-[160px]"
        value={tag}
        maxLength={maxLength}
        placeholder="tag"
        disabled={busy}
        onChange={(e) => setTag(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") void submit();
          if (e.key === "Escape") onCancel();
        }}
      />
      <button
        type="button"
        className="icon-btn-submit"
        disabled={!tag.trim() || busy}
        aria-label="Confirm"
        onClick={() => void submit()}
      >
        <EnterIcon />
      </button>
      <button
        type="button"
        className="icon-btn-submit"
        disabled={busy}
        aria-label="Cancel"
        onClick={onCancel}
      >
        <EscBadge />
      </button>
    </div>
  );
}

export function CreateTagControl({
  label,
  maxLength,
  onCreate,
}: {
  label: string;
  maxLength: number;
  onCreate: (tag: string) => Promise<void>;
}) {
  const [creating, setCreating] = useState(false);

  if (!creating) {
    return (
      <button type="button" className="btn-secondary shrink-0 !py-3" onClick={() => setCreating(true)}>
        {label}
      </button>
    );
  }

  return (
    <InlineTagInput
      maxLength={maxLength}
      onSubmit={async (tag) => {
        await onCreate(tag);
        setCreating(false);
      }}
      onCancel={() => setCreating(false)}
    />
  );
}

export function ForkControl({
  sourceTag,
  maxLength,
  onFork,
}: {
  sourceTag: string;
  maxLength: number;
  onFork: (tag: string) => Promise<void>;
}) {
  const [forking, setForking] = useState(false);

  if (!forking) {
    return (
      <button type="button" className="btn-view" onClick={() => setForking(true)}>
        <ForkIcon />
        Fork
      </button>
    );
  }

  return (
    <InlineTagInput
      initial={`${sourceTag}-fork`.slice(0, maxLength)}
      maxLength={maxLength}
      onSubmit={async (tag) => {
        await onFork(tag);
        setForking(false);
      }}
      onCancel={() => setForking(false)}
    />
  );
}

export function EditableTag({
  tag,
  maxLength,
  onRename,
  subtitle,
}: {
  tag: string;
  maxLength: number;
  onRename: (tag: string) => Promise<void>;
  subtitle?: ReactNode;
}) {
  const [editing, setEditing] = useState(false);

  if (editing) {
    return (
      <div className="min-w-0 flex-1">
        <InlineTagInput
          initial={tag}
          maxLength={maxLength}
          onSubmit={async (next) => {
            if (next !== tag) await onRename(next);
            setEditing(false);
          }}
          onCancel={() => setEditing(false)}
        />
      </div>
    );
  }

  return (
    <div className="min-w-0 flex-1">
      <div className="flex items-center gap-2">
        <span className="truncate font-medium">{tag}</span>
        <button
          type="button"
          className="icon-btn shrink-0"
          aria-label="Edit tag"
          title="Edit tag"
          onClick={() => setEditing(true)}
        >
          <PencilIcon />
        </button>
      </div>
      {subtitle}
    </div>
  );
}
