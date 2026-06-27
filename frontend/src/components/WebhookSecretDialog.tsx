// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useMemo, useState } from "react";
import { getWebhookSecret } from "../api/client";
import {
  WEBHOOK_SNIPPET_LANGS,
  WebhookSnippetLang,
  webhookVerificationSnippets,
} from "../utils/webhookVerificationSnippets";

type Props = {
  releaseTag: string;
  webhookId: string;
  webhookUrl: string;
  onClose: () => void;
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

function CopyIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <rect x="9" y="9" width="13" height="13" rx="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
  );
}

function CopyButton({
  text,
  label,
  className = "btn-secondary shrink-0 text-xs",
}: {
  text: string;
  label: string;
  className?: string;
}) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  }

  return (
    <button
      type="button"
      className={className}
      onClick={() => void copy()}
      aria-label={label}
      title={label}
    >
      {copied ? "Copied" : <CopyIcon />}
    </button>
  );
}

function SnippetCopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  }

  return (
    <button
      type="button"
      className="absolute right-2 top-2 rounded-md border px-2 py-1 text-xs transition-all"
      style={{
        borderColor: "var(--border)",
        background: copied ? "color-mix(in srgb, var(--accent) 25%, var(--surface))" : "var(--surface)",
        color: copied ? "var(--accent)" : "var(--muted)",
      }}
      onClick={() => void copy()}
      aria-label="Copy snippet"
    >
      {copied ? "Copied" : "Copy"}
    </button>
  );
}

export function WebhookSecretDialog({ releaseTag, webhookId, webhookUrl, onClose }: Props) {
  const [secret, setSecret] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [visible, setVisible] = useState(false);
  const [snippetLang, setSnippetLang] = useState<WebhookSnippetLang>("python");

  useEffect(() => {
    setLoading(true);
    setError(null);
    void getWebhookSecret(releaseTag, webhookId)
      .then((res) => setSecret(res.secret))
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, [releaseTag, webhookId]);

  const maskedSecret = useMemo(() => {
    if (!secret) return "";
    return "•".repeat(Math.max(secret.length, 16));
  }, [secret]);

  const displaySnippets = useMemo(() => {
    if (!secret) return null;
    return webhookVerificationSnippets(secret, visible);
  }, [secret, visible]);

  const copySnippets = useMemo(() => {
    if (!secret) return null;
    return webhookVerificationSnippets(secret, true);
  }, [secret]);

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="card w-full max-w-3xl max-h-[90vh] overflow-y-auto space-y-5"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h3 className="text-lg font-semibold">Webhook signing secret</h3>
            <p className="mt-1 truncate text-sm text-[var(--muted)]">{webhookUrl}</p>
          </div>
          <button type="button" className="btn-secondary shrink-0" onClick={onClose}>
            Close
          </button>
        </div>

        {loading && <p className="text-sm text-[var(--muted)]">Loading secret…</p>}
        {error && <p className="text-sm text-error">{error}</p>}

        {secret && (
          <>
            <label className="block space-y-1 text-sm">
              <span>Secret</span>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  className="rounded-md border p-2 text-[var(--muted)] hover:text-[var(--text)]"
                  style={{ borderColor: "var(--border)" }}
                  onClick={() => setVisible((v) => !v)}
                  aria-label={visible ? "Hide secret" : "Show secret"}
                >
                  <EyeIcon off={visible} />
                </button>
                <input
                  className="input flex-1 font-mono text-sm"
                  type={visible ? "text" : "password"}
                  value={visible ? secret : maskedSecret}
                  readOnly
                  aria-label="Webhook secret"
                />
                <CopyButton text={secret} label="Copy secret" />
              </div>
            </label>

            <div className="space-y-2 text-sm text-[var(--muted)]">
              <p>
                Ragdoll signs every webhook with <strong className="text-[var(--text)]">HMAC-SHA256</strong>.
                Each request includes:
              </p>
              <ul className="list-disc space-y-1 pl-5">
                <li>
                  <code className="text-xs">X-Ragdoll-Timestamp</code> — Unix seconds when the request was sent
                </li>
                <li>
                  <code className="text-xs">X-Ragdoll-Signature</code> —{" "}
                  <code className="text-xs">sha256=&lt;hex&gt;</code> over{" "}
                  <code className="text-xs">{"{timestamp}.{raw_body}"}</code>
                </li>
              </ul>
              <p>
                Read the raw request body without re-serializing JSON, build{" "}
                <code className="text-xs">timestamp + "." + body</code>, compute HMAC-SHA256 with this
                webhook&apos;s secret, and compare it to the header value using a constant-time comparison.
              </p>
            </div>

            <div className="space-y-3">
              <div className="flex flex-wrap gap-2">
                {WEBHOOK_SNIPPET_LANGS.map((lang) => (
                  <button
                    key={lang.id}
                    type="button"
                    className={`btn-secondary ${snippetLang === lang.id ? "btn-toggle-active" : ""}`}
                    onClick={() => setSnippetLang(lang.id)}
                  >
                    {lang.label}
                  </button>
                ))}
              </div>
              <div className="relative">
                {displaySnippets && copySnippets && (
                  <>
                    <SnippetCopyButton text={copySnippets[snippetLang]} />
                    <pre
                      className="overflow-x-auto rounded-lg border p-4 pr-20 font-mono text-xs"
                      style={{ borderColor: "var(--border)", background: "var(--bg)" }}
                    >
                      {displaySnippets[snippetLang]}
                    </pre>
                  </>
                )}
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
