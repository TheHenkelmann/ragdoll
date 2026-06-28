// SPDX-License-Identifier: AGPL-3.0-only

type QueryBodyItem = {
  text: string;
  top_k: number;
  rerank: boolean;
  rerank_candidates: number;
  min_semantic_score: number;
  min_rerank_score: number;
  hybrid?: boolean;
  bm25_weight?: number;
  filter?: unknown;
};

function pyValue(v: unknown, indent = 0): string {
  const pad = " ".repeat(indent);
  if (v === null || v === undefined) return "None";
  if (typeof v === "boolean") return v ? "True" : "False";
  if (typeof v === "number") return String(v);
  if (typeof v === "string") return JSON.stringify(v);
  if (Array.isArray(v)) {
    if (v.length === 0) return "[]";
    return `[${v.map((item) => pyValue(item)).join(", ")}]`;
  }
  if (typeof v === "object") {
    const entries = Object.entries(v as Record<string, unknown>);
    if (entries.length === 0) return "{}";
    const inner = entries
      .map(([k, val]) => `${pad}    ${JSON.stringify(k)}: ${pyValue(val, indent + 4)}`)
      .join(",\n");
    return `{\n${inner}\n${pad}}`;
  }
  return "None";
}

function goValue(v: unknown): string {
  if (v === null || v === undefined) return "nil";
  if (typeof v === "boolean") return v ? "true" : "false";
  if (typeof v === "number") return String(v);
  if (typeof v === "string") return JSON.stringify(v);
  if (Array.isArray(v)) return `[]interface{}{${v.map(goValue).join(", ")}}`;
  if (typeof v === "object") {
    const entries = Object.entries(v as Record<string, unknown>);
    return `map[string]interface{}{${entries.map(([k, val]) => `${JSON.stringify(k)}: ${goValue(val)}`).join(", ")}}`;
  }
  return "nil";
}

export function buildSnippets(
  baseUrl: string,
  releaseTag: string,
  queryBody: QueryBodyItem[],
) {
  const urlComment =
    "Use /api/v1/stages/{STAGE_TAG}/queries for consistent mapping to the endpoint pinned in the stage.";
  const releaseUrl = `${baseUrl}/api/v1/releases/${releaseTag}/queries?store_payload=false`;
  const item = queryBody[0]!;
  const pyPayload = `[${pyValue(item)}]`;

  const curl = `# Replace with a real API key
API_KEY="your-api-key-here"

curl -sS -X POST '${releaseUrl}' \\
  # ${urlComment}
  -H "Authorization: Bearer $API_KEY" \\
  -H 'Content-Type: application/json' \\
  -d '${JSON.stringify(queryBody)}'`;

  const python = `# Replace with a real API key
API_KEY = "your-api-key-here"

import requests

payload = ${pyPayload}
# ${urlComment}
resp = requests.post(
    "${releaseUrl}",
    headers={"Authorization": f"Bearer {API_KEY}"},
    json=payload,
)
resp.raise_for_status()
print(resp.json())`;

  const javascript = `// Replace with a real API key
const API_KEY = "your-api-key-here";

const payload = ${JSON.stringify(queryBody, null, 2)};
// ${urlComment}
const resp = await fetch("${releaseUrl}", {
  method: "POST",
  headers: {
    Authorization: \`Bearer \${API_KEY}\`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify(payload),
});
if (!resp.ok) throw new Error(\`\${resp.status} \${await resp.text()}\`);
console.log(await resp.json());`;

  const goSnippet = `// Replace with a real API key
API_KEY := "your-api-key-here"

payload := ${goValue(queryBody)}
// ${urlComment}
body, _ := json.Marshal(payload)
req, _ := http.NewRequest("POST", "${releaseUrl}", bytes.NewReader(body))
req.Header.Set("Authorization", "Bearer "+API_KEY)
req.Header.Set("Content-Type", "application/json")
resp, err := http.DefaultClient.Do(req)
if err != nil { panic(err) }
defer resp.Body.Close()
if resp.StatusCode >= 400 {
    b, _ := io.ReadAll(resp.Body)
    panic(fmt.Sprintf("%d %s", resp.StatusCode, string(b)))
}
var out map[string]interface{}
json.NewDecoder(resp.Body).Decode(&out)
fmt.Println(out)`;

  const rust = `// Replace with a real API key
const API_KEY: &str = "your-api-key-here";

let payload = ${JSON.stringify(queryBody)};
// ${urlComment}
let client = reqwest::Client::new();
let resp = client
    .post("${releaseUrl}")
    .bearer_auth(API_KEY)
    .json(&payload)
    .send()
    .await?;
resp.error_for_status()?;
println!("{:?}", resp.json::<serde_json::Value>().await?);`;

  const java = `// Replace with a real API key
String API_KEY = "your-api-key-here";

String body = ${JSON.stringify(JSON.stringify(queryBody))};
// ${urlComment}
HttpClient client = HttpClient.newHttpClient();
HttpRequest request = HttpRequest.newBuilder()
    .uri(URI.create("${releaseUrl}"))
    .header("Authorization", "Bearer " + API_KEY)
    .header("Content-Type", "application/json")
    .POST(HttpRequest.BodyPublishers.ofString(body))
    .build();
HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
if (response.statusCode() >= 400) {
    throw new RuntimeException(response.statusCode() + " " + response.body());
}
System.out.println(response.body());`;

  return { curl, python, javascript, go: goSnippet, rust, java };
}
