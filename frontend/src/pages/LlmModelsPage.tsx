// SPDX-License-Identifier: AGPL-3.0-only

import { ReactNode, useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import {
  LlmCredentialRecord,
  LlmModelRecord,
  LlmModelTestResult,
  api,
  testLlmModel,
} from "../api/client";
import { useAuth } from "../context/AuthContext";
import { useSnackbar } from "../context/SnackbarContext";
import { PermissionDenied } from "../components/PermissionDenied";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import { pushApiError } from "../utils/snackbarFormat";

import {
  CUSTOM_MODEL_VALUE,
  LLM_PROVIDERS,
  MODEL_PRESETS,
  azureEndpointKind,
  extractAzureDeployment,
  normalizeOpenAiBaseUrl,
  modelDisplayName,
  presetSelectValue,
  presetsForProvider,
  providerLabel,
} from "../llm/providerCatalog";

function credentialsForProvider(
  credentials: LlmCredentialRecord[],
  provider: string,
): LlmCredentialRecord[] {
  return credentials.filter((c) => c.provider === provider);
}

function defaultCredentialId(
  credentials: LlmCredentialRecord[],
  provider: string,
): string {
  const available = credentialsForProvider(credentials, provider);
  return available.length === 1 ? available[0].id : "";
}
function parseVertexEndpoint(endpoint: string): { project_id: string; location: string } {
  try {
    const v = JSON.parse(endpoint) as { project_id?: string; location?: string };
    return { project_id: v.project_id ?? "", location: v.location ?? "global" };
  } catch {
    return { project_id: "", location: "global" };
  }
}

function buildVertexEndpoint(projectId: string, location: string): string {
  return JSON.stringify({
    project_id: projectId.trim(),
    location: location.trim() || "global",
  });
}

function suggestModelTag(provider: string, modelId: string): string {
  const slug = modelId
    .toLowerCase()
    .replace(/[^a-z0-9.]+/g, "-")
    .replace(/^-+|-+$/g, "");
  if (!slug) return provider;
  return `${provider}-${slug}`;
}

function validateVertexServiceAccountJson(raw: string): string | null {
  try {
    const value = JSON.parse(raw.trim()) as Record<string, unknown>;
    for (const key of ["type", "project_id", "private_key", "client_email"]) {
      const field = value[key];
      if (typeof field !== "string" || field.trim() === "") {
        return `Service account JSON missing required field "${key}".`;
      }
    }
    return null;
  } catch {
    return "Service account JSON is not valid JSON.";
  }
}

function ServiceAccountFileField({
  label,
  hint,
  fileName,
  onJson,
}: {
  label: string;
  hint?: string;
  fileName: string | null;
  onJson: (json: string, name: string) => void;
}) {
  const [error, setError] = useState<string | null>(null);

  async function handleFile(file: File | null) {
    if (!file) return;
    setError(null);
    try {
      const text = await file.text();
      const validationError = validateVertexServiceAccountJson(text);
      if (validationError) {
        setError(validationError);
        return;
      }
      onJson(text.trim(), file.name);
    } catch {
      setError("Could not read a valid JSON service account key from that file.");
    }
  }

  return (
    <Field label={label} hint={hint}>
      <input
        className="input text-sm file:mr-3 file:rounded file:border-0 file:bg-[var(--selected)] file:px-3 file:py-1.5 file:text-sm"
        type="file"
        accept=".json,application/json"
        onChange={(e) => void handleFile(e.target.files?.[0] ?? null)}
      />
      {fileName && <span className="text-xs text-[var(--muted)]">Selected: {fileName}</span>}
      {error && <span className="text-xs text-error">{error}</span>}
    </Field>
  );
}

function endpointPlaceholder(provider: string): string {
  if (provider === "openai_compat") {
    return "https://api.openrouter.ai/api/v1/";
  }
  if (provider === "azure") {
    return "…/openai/responses?api-version=… or …/deployments/<name>/chat/completions?api-version=…";
  }
  return "";
}

function Spinner() {
  return (
    <span
      className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-current border-t-transparent"
      aria-hidden
    />
  );
}

function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="block space-y-1 text-sm">
      <span className="font-medium">{label}</span>
      {children}
      {hint && <span className="block text-xs text-[var(--muted)]">{hint}</span>}
    </label>
  );
}

type ModelForm = {
  provider: string;
  credential_id: string;
  tag: string;
  model_preset: string;
  model_name: string;
  endpoint: string;
  vertex_project_id: string;
  vertex_location: string;
};

function emptyModelForm(): ModelForm {
  return {
    provider: "",
    credential_id: "",
    tag: "",
    model_preset: "",
    model_name: "",
    endpoint: "",
    vertex_project_id: "",
    vertex_location: "global",
  };
}

function modelToForm(m: LlmModelRecord): ModelForm {
  const vertex = m.provider === "vertex" ? parseVertexEndpoint(m.endpoint ?? "") : { project_id: "", location: "global" };
  return {
    provider: m.provider,
    credential_id: m.credential_id ?? "",
    tag: m.tag,
    model_preset: presetSelectValue(m.model_name, m.provider),
    model_name: m.model_name,
    endpoint: m.endpoint ?? "",
    vertex_project_id: vertex.project_id,
    vertex_location: vertex.location,
  };
}

function resolvedModelName(form: ModelForm): string {
  if (form.provider === "azure" && form.endpoint.trim()) {
    const deployment = extractAzureDeployment(form.endpoint);
    if (deployment && azureEndpointKind(form.endpoint) === "chat") {
      return deployment;
    }
  }
  if (form.model_preset && form.model_preset !== CUSTOM_MODEL_VALUE) {
    return form.model_preset;
  }
  return form.model_name.trim();
}

function ModelPresetFields({
  form,
  setForm,
  disabled,
  onModelIdChange,
}: {
  form: ModelForm;
  setForm: (f: ModelForm) => void;
  disabled: boolean;
  onModelIdChange?: (modelId: string) => void;
}) {
  const presets = presetsForProvider(form.provider);
  const showCustom = form.model_preset === CUSTOM_MODEL_VALUE;

  function applyModelId(modelId: string, preset: string) {
    setForm({
      ...form,
      model_preset: preset,
      model_name: preset === CUSTOM_MODEL_VALUE ? form.model_name : modelId,
    });
    if (preset !== CUSTOM_MODEL_VALUE && modelId) {
      onModelIdChange?.(modelId);
    }
  }

  return (
    <>
      <Field
        label="Model"
        hint="Pick a preset or enter a custom model id. The provider API tag is stored — friendly names are for display only."
      >
        <select
          className="input"
          value={form.model_preset}
          disabled={disabled}
          onChange={(e) => {
            const value = e.target.value;
            if (value === CUSTOM_MODEL_VALUE) {
              applyModelId(form.model_name, value);
            } else {
              applyModelId(value, value);
            }
          }}
        >
          <option value="" disabled>
            Select model…
          </option>
          {presets.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
          <option value={CUSTOM_MODEL_VALUE}>Custom model id…</option>
        </select>
      </Field>
      {showCustom && (
        <Field label="Custom model id" hint="Exact id from the provider docs (e.g. openai/gpt-oss-20b on Groq).">
          <input
            className="input font-mono text-sm"
            placeholder="model-id"
            value={form.model_name}
            disabled={disabled}
            onChange={(e) => {
              const modelId = e.target.value;
              setForm({ ...form, model_name: modelId });
              if (modelId.trim()) onModelIdChange?.(modelId.trim());
            }}
          />
        </Field>
      )}
    </>
  );
}

function ModelFormModal({
  mode,
  original,
  credentials,
  releaseTag,
  onClose,
  onSaved,
}: {
  mode: "create" | "edit";
  original?: LlmModelRecord;
  credentials: LlmCredentialRecord[];
  releaseTag: string;
  onClose: () => void;
  onSaved: (model: LlmModelRecord, isNew: boolean) => void;
}) {
  const snackbar = useSnackbar();
  const [form, setForm] = useState<ModelForm>(original ? modelToForm(original) : emptyModelForm());
  const [tagTouched, setTagTouched] = useState(mode === "edit");
  const [busy, setBusy] = useState(false);

  function prefillTag(modelId: string) {
    if (mode !== "create" || tagTouched || !form.provider) return;
    setForm((current) => ({ ...current, tag: suggestModelTag(form.provider, modelId) }));
  }

  const providersWithCredentials = LLM_PROVIDERS.filter((p) =>
    credentials.some((c) => c.provider === p.value),
  );
  const providerChosen = form.provider !== "";
  const credentialChosen = form.credential_id !== "";
  const showDetails = providerChosen && credentialChosen;
  const availableCreds = credentialsForProvider(credentials, form.provider);

  useEffect(() => {
    if (!providerChosen) return;
    if (availableCreds.length === 1 && form.credential_id !== availableCreds[0].id) {
      setForm((current) => ({ ...current, credential_id: availableCreds[0].id }));
    }
  }, [providerChosen, availableCreds, form.credential_id]);
  const isVertex = form.provider === "vertex";
  const isAzure = form.provider === "azure";
  const isOpenAiCompat = form.provider === "openai_compat";
  const azureKind = isAzure && form.endpoint.trim() ? azureEndpointKind(form.endpoint) : "unknown";
  const azureDeployment =
    isAzure && form.endpoint.trim() ? extractAzureDeployment(form.endpoint) : null;
  const azureModelFromUrl = azureKind === "chat" && azureDeployment !== null;
  const modelName = resolvedModelName(form);
  const modelSelected =
    azureModelFromUrl ||
    (form.model_preset !== "" &&
      form.model_preset !== CUSTOM_MODEL_VALUE) ||
    (form.model_preset === CUSTOM_MODEL_VALUE && form.model_name.trim() !== "");
  const canSave =
    providerChosen &&
    credentialChosen &&
    modelSelected &&
    form.tag.trim() !== "" &&
    (isVertex ? form.vertex_project_id.trim() !== "" : true) &&
    (isOpenAiCompat ? form.endpoint.trim() !== "" : true) &&
    (isAzure ? form.endpoint.trim() !== "" : true) &&
    (azureModelFromUrl || modelName !== "") &&
    !busy;

  async function save() {
    if (!canSave) return;
    setBusy(true);
    let endpointValue: string | null = null;
    if (isVertex) {
      endpointValue = buildVertexEndpoint(form.vertex_project_id, form.vertex_location);
    } else if (form.endpoint.trim()) {
      endpointValue = isOpenAiCompat
        ? normalizeOpenAiBaseUrl(form.endpoint)
        : form.endpoint.trim();
    }
    const payload = JSON.stringify({
      tag: form.tag.trim(),
      model_name: modelName,
      provider: form.provider,
      endpoint: endpointValue,
      credential_id: form.credential_id || null,
    });
    try {
      const saved =
        mode === "create"
          ? await api<LlmModelRecord>(`/releases/${releaseTag}/llm_models`, { method: "POST", body: payload })
          : await api<LlmModelRecord>(`/releases/${releaseTag}/llm_models/${encodeURIComponent(original!.tag)}`, {
              method: "PUT",
              body: payload,
            });
      onSaved(saved, mode === "create");
    } catch (err) {
      pushApiError(snackbar.error, err);
      setBusy(false);
    }
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card w-full max-w-lg space-y-4" onClick={(e) => e.stopPropagation()}>
        <h3 className="text-lg font-semibold">
          {mode === "create" ? "Add model" : `Edit model: ${original?.tag}`}
        </h3>

        <Field
          label="Provider"
          hint={LLM_PROVIDERS.find((p) => p.value === form.provider)?.hint}
        >
          <select
            className="input"
            value={form.provider}
            disabled={providersWithCredentials.length === 0}
            onChange={(e) => {
              const provider = e.target.value;
              setTagTouched(false);
              setForm({
                ...emptyModelForm(),
                provider,
                credential_id: defaultCredentialId(credentials, provider),
              });
            }}
          >
            <option value="">Select provider…</option>
            {providersWithCredentials.map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
        </Field>

        {providersWithCredentials.length === 0 && (
          <p className="text-sm text-[var(--muted)]">
            No credentials configured yet. Add a credential above before creating a model.
          </p>
        )}

        {providerChosen && availableCreds.length === 1 && (
          <Field label="Credential" hint={`Using credential “${availableCreds[0].name}”.`}>
            <input className="input" value={availableCreds[0].name} disabled />
          </Field>
        )}

        {providerChosen && availableCreds.length > 1 && (
          <Field label="Credential" hint="Only credentials matching the selected provider are shown.">
            <div className="space-y-2">
              {availableCreds.map((c) => (
                <label
                  key={c.id}
                  className="flex cursor-pointer items-center gap-2 rounded border px-3 py-2 text-sm"
                  style={{ borderColor: "var(--border)" }}
                >
                  <input
                    type="radio"
                    name="model-credential"
                    value={c.id}
                    checked={form.credential_id === c.id}
                    onChange={() =>
                      setForm({
                        ...form,
                        credential_id: c.id,
                        tag: mode === "create" ? "" : form.tag,
                        model_preset: mode === "create" ? "" : form.model_preset,
                        model_name: mode === "create" ? "" : form.model_name,
                        endpoint: mode === "create" ? "" : form.endpoint,
                      })
                    }
                  />
                  <span>{c.name}</span>
                </label>
              ))}
            </div>
          </Field>
        )}

        {providerChosen && availableCreds.length === 0 && (
          <p className="text-sm text-[var(--muted)]">
            No credentials for {providerLabel(form.provider)} yet. Create one above first.
          </p>
        )}

        {showDetails && (
          <>
            {isAzure && (
              <Field
                label="Azure URL"
                hint="Paste the full URL from Azure. For deployment URLs the deployment name is used automatically."
              >
                <input
                  className="input font-mono text-xs"
                  placeholder={endpointPlaceholder("azure")}
                  value={form.endpoint}
                  onChange={(e) => {
                    const endpoint = e.target.value;
                    setForm((current) => {
                      const next = { ...current, endpoint };
                      if (mode === "create" && !tagTouched) {
                        const deployment = extractAzureDeployment(endpoint);
                        if (deployment && azureEndpointKind(endpoint) === "chat") {
                          next.tag = suggestModelTag(current.provider, deployment);
                        }
                      }
                      return next;
                    });
                  }}
                />
              </Field>
            )}

            {isOpenAiCompat && (
              <Field
                label="API base URL"
                hint="Base URL only — not the full /chat/completions path (Ragdoll adds that)."
              >
                <input
                  className="input font-mono text-xs"
                  placeholder={endpointPlaceholder("openai_compat")}
                  value={form.endpoint}
                  onChange={(e) => setForm({ ...form, endpoint: e.target.value })}
                />
              </Field>
            )}

            {isVertex && (
              <>
                <Field label="GCP project ID" hint="Required. Your Google Cloud project id.">
                  <input
                    className="input"
                    placeholder="my-gcp-project"
                    value={form.vertex_project_id}
                    onChange={(e) => setForm({ ...form, vertex_project_id: e.target.value })}
                  />
                </Field>
                <Field label="GCP region" hint="e.g. europe-west1 or global.">
                  <input
                    className="input"
                    placeholder="global"
                    value={form.vertex_location}
                    onChange={(e) => setForm({ ...form, vertex_location: e.target.value })}
                  />
                </Field>
              </>
            )}

            {isAzure && azureModelFromUrl ? (
              <Field label="Deployment" hint="Taken from the URL — no separate model field needed.">
                <input className="input font-mono text-sm" value={azureDeployment ?? ""} disabled />
              </Field>
            ) : (
              <ModelPresetFields
                form={form}
                setForm={setForm}
                disabled={false}
                onModelIdChange={prefillTag}
              />
            )}

            <Field label="Tag" hint="Unique identifier used to reference this model in queries.">
              <input
                className="input"
                placeholder="e.g. azure-gpt-5-4-mini"
                value={form.tag}
                onChange={(e) => {
                  setTagTouched(true);
                  setForm({ ...form, tag: e.target.value });
                }}
              />
            </Field>
          </>
        )}

        <div className="flex justify-end gap-2">
          <button type="button" className="btn-secondary" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button type="button" className="btn-primary" disabled={!canSave} onClick={() => void save()}>
            {busy ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}

export function LlmModelsPage() {
  const snackbar = useSnackbar();
  const { releaseTag = "" } = useParams();
  const { status } = useAuth();
  const { can, ready } = usePermissions();
  const canReadModels = can(PERM.llmModels.read);
  const canWriteModels = can(PERM.llmModels.write);
  const canDeleteModels = can(PERM.llmModels.delete);
  const canReadCreds = can(PERM.llmCredentials.read);
  const canWriteCreds = can(PERM.llmCredentials.write);
  const canDeleteCreds = can(PERM.llmCredentials.delete);
  const [credentials, setCredentials] = useState<LlmCredentialRecord[]>([]);
  const [models, setModels] = useState<LlmModelRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");

  const [newCred, setNewCred] = useState({
    name: "",
    provider: "openai",
    api_key: "",
    service_account_json: "",
  });
  const [newCredSaFile, setNewCredSaFile] = useState<string | null>(null);
  const [editCred, setEditCred] = useState<LlmCredentialRecord | null>(null);
  const [editCredKey, setEditCredKey] = useState("");
  const [editCredJson, setEditCredJson] = useState("");
  const [editCredSaFile, setEditCredSaFile] = useState<string | null>(null);
  const [deleteCredTarget, setDeleteCredTarget] = useState<LlmCredentialRecord | null>(null);
  const [deleteCredConfirm, setDeleteCredConfirm] = useState("");

  const [modal, setModal] = useState<{ mode: "create" | "edit"; model?: LlmModelRecord } | null>(
    null,
  );
  const [deleteModelTarget, setDeleteModelTarget] = useState<LlmModelRecord | null>(null);
  const [deleteModelConfirm, setDeleteModelConfirm] = useState("");
  const [testResults, setTestResults] = useState<
    Record<string, { status: "running" | "ok" | "fail"; message?: string }>
  >({});

  async function reload() {
    if (!canReadModels && !canReadCreds) {
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const fetches: Promise<void>[] = [];
      if (canReadCreds) {
        fetches.push(
          api<LlmCredentialRecord[]>(`/releases/${releaseTag}/llm_credentials`).then(setCredentials),
        );
      } else {
        setCredentials([]);
      }
      if (canReadModels) {
        fetches.push(
          api<LlmModelRecord[]>(`/releases/${releaseTag}/llm_models`).then(setModels),
        );
      } else {
        setModels([]);
      }
      await Promise.all(fetches);
    } catch (err) {
      pushApiError(snackbar.error, err);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (!ready) return;
    void reload();
  }, [releaseTag, ready, canReadModels, canReadCreds]);

  async function createCredential() {
    const isVertex = newCred.provider === "vertex";
    if (!newCred.name.trim()) return;
    if (isVertex && !newCred.service_account_json.trim()) return;
    if (!isVertex && !newCred.api_key.trim()) return;
    try {
      await api(`/releases/${releaseTag}/llm_credentials`, {
        method: "POST",
        body: JSON.stringify(
          isVertex
            ? {
                name: newCred.name.trim(),
                provider: newCred.provider,
                service_account_json: newCred.service_account_json.trim(),
              }
            : {
                name: newCred.name.trim(),
                provider: newCred.provider,
                api_key: newCred.api_key.trim(),
              },
        ),
      });
      setNewCred({ name: "", provider: "openai", api_key: "", service_account_json: "" });
      setNewCredSaFile(null);
      await reload();
    } catch (err) {
      pushApiError(snackbar.error, err);
    }
  }

  async function saveCredentialKey() {
    if (!editCred) return;
    const isVertex = editCred.provider === "vertex";
    if (isVertex && !editCredJson.trim()) return;
    if (!isVertex && !editCredKey.trim()) return;
    try {
      await api(`/releases/${releaseTag}/llm_credentials/${editCred.id}`, {
        method: "PUT",
        body: JSON.stringify(
          isVertex
            ? { service_account_json: editCredJson.trim() }
            : { api_key: editCredKey.trim() },
        ),
      });
      setEditCred(null);
      setEditCredKey("");
      setEditCredJson("");
      setEditCredSaFile(null);
      await reload();
    } catch (err) {
      pushApiError(snackbar.error, err);
    }
  }

  async function runTest(tag: string, modelId: string) {
    setTestResults((prev) => ({ ...prev, [modelId]: { status: "running" } }));
    try {
      const res: LlmModelTestResult = await testLlmModel(releaseTag, tag);
      setTestResults((prev) => ({
        ...prev,
        [modelId]: {
          status: res.ok ? "ok" : "fail",
          message: res.ok
            ? `${res.message}${res.latency_ms != null ? ` (${res.latency_ms} ms)` : ""}`
            : res.message,
        },
      }));
    } catch (err) {
      setTestResults((prev) => ({
        ...prev,
        [modelId]: { status: "fail", message: String(err) },
      }));
    }
  }

  async function onModelSaved(model: LlmModelRecord, isNew: boolean) {
    setModal(null);
    await reload();
    // A freshly created model is connection-tested automatically.
    if (isNew) {
      void runTest(model.tag, model.id);
    }
  }

  const filteredModels = models.filter((m) => {
    const q = search.toLowerCase();
    return (
      m.tag.toLowerCase().includes(q) ||
      m.model_name.toLowerCase().includes(q) ||
      m.provider.toLowerCase().includes(q) ||
      (m.credential_name ?? "").toLowerCase().includes(q)
    );
  });

  const deleteModelPhrase = deleteModelTarget
    ? `llm-model/${deleteModelTarget.provider}/${deleteModelTarget.tag}`
    : "";
  const deleteCredPhrase = deleteCredTarget
    ? `llm-key/${deleteCredTarget.provider}/${deleteCredTarget.name}`
    : "";
  const modelsUsingDeleteCred = deleteCredTarget
    ? models.filter((m) => m.credential_id === deleteCredTarget.id)
    : [];

  if (ready && !canReadModels && !canReadCreds) {
    return <PermissionDenied permission={PERM.llmModels.read} />;
  }

  return (
    <div className="space-y-8">
      <div className="space-y-2">
        <h2 className="text-2xl font-semibold">LLM Models</h2>
        <p className="text-sm text-[var(--muted)]">
          Configure BYO LLM credentials and models for release{" "}
          <code className="rounded px-1" style={{ background: "var(--selected)" }}>
            {releaseTag}
          </code>
          . Keys are write-only, encrypted at rest, and never shown again after saving.
        </p>
        {status?.is_superadmin && (
          <p className="text-sm text-[var(--muted)]">
            If you change the{" "}
            <code className="rounded px-1" style={{ background: "var(--selected)" }}>
              RAGDOLL_SECRET
            </code>{" "}
            environment variable, stored credentials can no longer be decrypted and must be
            re-entered. Query requests with generation enabled will stop working immediately until
            credentials are updated.
          </p>
        )}
      </div>

      <section className="card space-y-4">
        <h3 className="font-medium">Credentials</h3>
        <div className="grid items-end gap-3 md:grid-cols-4">
          <Field label="Provider">
            <select
              className="input"
              value={newCred.provider}
              onChange={(e) => {
                setNewCred({
                  ...newCred,
                  provider: e.target.value,
                  api_key: "",
                  service_account_json: "",
                });
                setNewCredSaFile(null);
              }}
            >
              {LLM_PROVIDERS.map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
          </Field>
          {newCred.provider === "vertex" ? (
            <ServiceAccountFileField
              label="Service account key file"
              hint="Select the JSON key file downloaded from GCP. Ragdoll validates required fields before storing it encrypted."
              fileName={newCredSaFile}
              onJson={(json, name) => {
                setNewCred({ ...newCred, service_account_json: json });
                setNewCredSaFile(name);
              }}
            />
          ) : (
            <Field label="API key">
              <input
                className="input"
                type="password"
                placeholder="sk-…"
                value={newCred.api_key}
                onChange={(e) => setNewCred({ ...newCred, api_key: e.target.value })}
              />
            </Field>
          )}
          <Field label="Name">
            <input
              className="input"
              placeholder="e.g. openai-prod"
              value={newCred.name}
              onChange={(e) => setNewCred({ ...newCred, name: e.target.value })}
            />
          </Field>
          <button
            type="button"
            className="btn-primary"
            disabled={!canWriteCreds}
            onClick={() => void createCredential()}
          >
            Add credential
          </button>
        </div>
        <ul className="space-y-2 text-sm">
          {credentials.length === 0 && !loading && (
            <li className="text-[var(--muted)]">No credentials yet.</li>
          )}
          {credentials.map((c) => (
            <li
              key={c.id}
              className="flex flex-wrap items-center justify-between gap-3 rounded border px-3 py-2"
              style={{ borderColor: "var(--border)" }}
            >
              <div>
                <div className="font-medium">{c.name}</div>
                <div className="text-[var(--muted)]">
                  {providerLabel(c.provider)} · updated {c.updated_at}
                </div>
              </div>
              <div className="flex gap-2">
                <button
                  type="button"
                  className="btn-secondary text-xs"
                  disabled={!canWriteCreds}
                  onClick={() => {
                    setEditCred(c);
                    setEditCredKey("");
                    setEditCredJson("");
                    setEditCredSaFile(null);
                  }}
                >
                  {c.provider === "vertex" ? "Update JSON" : "Update key"}
                </button>
                <button
                  type="button"
                  className="btn-danger text-xs"
                  disabled={!canDeleteCreds}
                  onClick={() => {
                    setDeleteCredTarget(c);
                    setDeleteCredConfirm("");
                  }}
                >
                  Delete
                </button>
              </div>
            </li>
          ))}
        </ul>
      </section>

      <section className="space-y-4">
        <h3 className="font-medium">Models</h3>
        <p className="text-sm text-[var(--muted)]">
          Pick a provider, then a credential. Model presets show friendly names but store the
          correct API model id. Use <strong>OpenAI-compatible</strong> for OpenRouter, vLLM, LM
          Studio, etc. Azure deployment URLs include the model — Responses API URLs still need a
          model name.
        </p>

        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            className="btn-primary shrink-0 disabled:cursor-not-allowed disabled:opacity-60"
            disabled={!canWriteModels || credentials.length === 0}
            title={
              !canWriteModels
                ? "Missing write permission"
                : credentials.length === 0
                  ? "Add a credential first"
                  : undefined
            }
            onClick={() => setModal({ mode: "create" })}
          >
            Create model
          </button>
          <input
            className="input min-w-[240px] flex-1"
            placeholder="Search models"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>

        <div className="space-y-2">
          {loading && <p className="text-sm text-[var(--muted)]">Loading…</p>}
          {!loading && filteredModels.length === 0 && (
            <p className="text-sm text-[var(--muted)]">No models found.</p>
          )}
          {filteredModels.map((m) => {
            const test = testResults[m.id];
            return (
              <div
                key={m.id}
                className="flex flex-wrap items-center gap-3 rounded-lg border px-4 py-3"
                style={{ borderColor: "var(--border)" }}
              >
                <div className="min-w-0 flex-1">
                  <div className="font-medium">{m.tag}</div>
                  <div className="text-xs text-[var(--muted)]">
                    {providerLabel(m.provider)} · {modelDisplayName(m.provider, m.model_name)}
                    {m.model_name !== modelDisplayName(m.provider, m.model_name) && (
                      <span className="font-mono"> ({m.model_name})</span>
                    )}{" "}
                    · Credential:{" "}
                    {m.credential_name ?? (m.credential_id ? m.credential_id : "none")}
                  </div>
                  {m.endpoint && (
                    <div className="truncate text-xs text-[var(--muted)]">Endpoint: {m.endpoint}</div>
                  )}
                  {test && (
                    <div
                      className={`mt-1 flex items-center gap-1.5 text-xs ${
                        test.status === "ok"
                          ? "text-emerald-500"
                          : test.status === "fail"
                            ? "text-error"
                            : "text-[var(--muted)]"
                      }`}
                    >
                      {test.status === "running" && <Spinner />}
                      {test.status === "running"
                        ? "Testing connection…"
                        : test.status === "ok"
                          ? `✓ ${test.message ?? "OK"}`
                          : `✗ ${test.message ?? "Failed"}`}
                    </div>
                  )}
                </div>
                <button
                  type="button"
                  className="btn-secondary shrink-0 text-xs"
                  disabled={!canWriteModels || test?.status === "running"}
                  onClick={() => void runTest(m.tag, m.id)}
                >
                  {test?.status === "running" ? (
                    <span className="flex items-center gap-1.5">
                      <Spinner /> Testing…
                    </span>
                  ) : (
                    "Test"
                  )}
                </button>
                <button
                  type="button"
                  className="btn-secondary shrink-0 text-xs"
                  disabled={!canWriteModels}
                  onClick={() => setModal({ mode: "edit", model: m })}
                >
                  Edit
                </button>
                <button
                  type="button"
                  className="btn-danger shrink-0 text-xs"
                  disabled={!canDeleteModels}
                  onClick={() => {
                    setDeleteModelTarget(m);
                    setDeleteModelConfirm("");
                  }}
                >
                  Delete
                </button>
              </div>
            );
          })}
        </div>
      </section>

      {modal && (
        <ModelFormModal
          mode={modal.mode}
          original={modal.model}
          credentials={credentials}
          releaseTag={releaseTag}
          onClose={() => setModal(null)}
          onSaved={(model, isNew) => void onModelSaved(model, isNew)}
        />
      )}

      {editCred && (
        <div className="modal-overlay" onClick={() => setEditCred(null)}>
          <div className="card w-full max-w-md space-y-4" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-semibold">
              {editCred.provider === "vertex" ? "Update service account" : "Update API key"}: {editCred.name}
            </h3>
            <p className="text-sm text-[var(--muted)]">
              Stored credentials cannot be read back. Paste a new value to replace the existing one.
            </p>
            {editCred.provider === "vertex" ? (
              <ServiceAccountFileField
                label="Service account key file"
                hint="Select a new JSON key file to replace the stored credential."
                fileName={editCredSaFile}
                onJson={(json, name) => {
                  setEditCredJson(json);
                  setEditCredSaFile(name);
                }}
              />
            ) : (
              <Field label="New API key">
                <input
                  className="input"
                  type="password"
                  placeholder="sk-…"
                  value={editCredKey}
                  onChange={(e) => setEditCredKey(e.target.value)}
                />
              </Field>
            )}
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary" onClick={() => setEditCred(null)}>
                Cancel
              </button>
              <button
                type="button"
                className="btn-primary"
                disabled={
                  editCred.provider === "vertex" ? !editCredJson.trim() : !editCredKey.trim()
                }
                onClick={() => void saveCredentialKey()}
              >
                Save
              </button>
            </div>
          </div>
        </div>
      )}

      {deleteModelTarget && (
        <div
          className="modal-overlay"
          onClick={() => {
            setDeleteModelTarget(null);
            setDeleteModelConfirm("");
          }}
        >
          <div className="card w-full max-w-md space-y-4" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-semibold">Delete model?</h3>
            <p className="text-sm text-[var(--muted)]">
              Type <code className="rounded px-1" style={{ background: "var(--selected)" }}>{deleteModelPhrase}</code> to confirm.
            </p>
            <input
              className="input font-mono text-sm"
              value={deleteModelConfirm}
              onChange={(e) => setDeleteModelConfirm(e.target.value)}
              placeholder={deleteModelPhrase}
              autoComplete="off"
            />
            <div className="flex justify-end gap-2">
              <button
                type="button"
                className="btn-secondary"
                onClick={() => {
                  setDeleteModelTarget(null);
                  setDeleteModelConfirm("");
                }}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn-danger"
                disabled={deleteModelConfirm !== deleteModelPhrase}
                onClick={() => {
                  void api(`/releases/${releaseTag}/llm_models/${encodeURIComponent(deleteModelTarget.tag)}`, { method: "DELETE" })
                    .then(() => {
                      setDeleteModelTarget(null);
                      setDeleteModelConfirm("");
                      return reload();
                    })
                    .catch((err) => pushApiError(snackbar.error, err));
                }}
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}

      {deleteCredTarget && (
        <div
          className="modal-overlay"
          onClick={() => {
            setDeleteCredTarget(null);
            setDeleteCredConfirm("");
          }}
        >
          <div className="card w-full max-w-md space-y-4" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-semibold">Delete credential?</h3>
            <p className="text-sm text-[var(--muted)]">
              Type{" "}
              <code className="rounded px-1" style={{ background: "var(--selected)" }}>
                {deleteCredPhrase}
              </code>{" "}
              to confirm. Models using this credential will lose their key link and{" "}
              <strong>generation requests referencing those models will fail immediately</strong> until
              you attach a new credential.
            </p>
            {modelsUsingDeleteCred.length > 0 ? (
              <div className="rounded border px-3 py-2 text-sm" style={{ borderColor: "var(--border)" }}>
                <div className="font-medium">Affected models ({modelsUsingDeleteCred.length})</div>
                <ul className="mt-1 list-inside list-disc text-[var(--muted)]">
                  {modelsUsingDeleteCred.map((m) => (
                    <li key={m.id}>
                      {m.tag} · {providerLabel(m.provider)} · {modelDisplayName(m.provider, m.model_name)}
                    </li>
                  ))}
                </ul>
              </div>
            ) : (
              <p className="text-sm text-[var(--muted)]">No models currently reference this credential.</p>
            )}
            <input
              className="input font-mono text-sm"
              value={deleteCredConfirm}
              onChange={(e) => setDeleteCredConfirm(e.target.value)}
              placeholder={deleteCredPhrase}
              autoComplete="off"
            />
            <div className="flex justify-end gap-2">
              <button
                type="button"
                className="btn-secondary"
                onClick={() => {
                  setDeleteCredTarget(null);
                  setDeleteCredConfirm("");
                }}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn-danger"
                disabled={deleteCredConfirm !== deleteCredPhrase}
                onClick={() => {
                  void api(`/releases/${releaseTag}/llm_credentials/${deleteCredTarget.id}`, { method: "DELETE" })
                    .then(() => {
                      setDeleteCredTarget(null);
                      setDeleteCredConfirm("");
                      return reload();
                    })
                    .catch((err) => pushApiError(snackbar.error, err));
                }}
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
