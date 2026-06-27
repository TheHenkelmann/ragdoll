// SPDX-License-Identifier: AGPL-3.0-only

export type WebhookSnippetLang = "python" | "node" | "rust" | "java";

export const WEBHOOK_SNIPPET_LANGS: { id: WebhookSnippetLang; label: string }[] = [
  { id: "python", label: "Python" },
  { id: "node", label: "Node" },
  { id: "rust", label: "Rust" },
  { id: "java", label: "Java" },
];

function displaySecret(secret: string, visible: boolean): string {
  if (visible) return secret;
  return "•".repeat(Math.max(secret.length, 16));
}

function pyLiteral(secret: string): string {
  return JSON.stringify(secret);
}

function jsLiteral(secret: string): string {
  return JSON.stringify(secret);
}

function rustLiteral(secret: string): string {
  return JSON.stringify(secret);
}

function javaLiteral(secret: string): string {
  return JSON.stringify(secret);
}

export function webhookVerificationSnippet(
  lang: WebhookSnippetLang,
  secret: string,
  visible: boolean,
): string {
  const shown = displaySecret(secret, visible);
  switch (lang) {
    case "python":
      return `import hmac
import hashlib

WEBHOOK_SECRET = ${pyLiteral(shown)}

def verify_webhook(raw_body: bytes, signature_header: str, timestamp_header: str) -> bool:
    if not signature_header.startswith("sha256="):
        return False
    expected = signature_header.removeprefix("sha256=")
    signing_input = f"{timestamp_header}.{raw_body.decode('utf-8')}"
    computed = hmac.new(
        WEBHOOK_SECRET.encode("utf-8"),
        signing_input.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()
    return hmac.compare_digest(computed, expected)`;
    case "node":
      return `const crypto = require("crypto");

const WEBHOOK_SECRET = ${jsLiteral(shown)};

function verifyWebhook(rawBody, signatureHeader, timestampHeader) {
  if (!signatureHeader?.startsWith("sha256=")) return false;
  const expected = signatureHeader.slice("sha256=".length);
  const signingInput = \`\${timestampHeader}.\${rawBody}\`;
  const computed = crypto
    .createHmac("sha256", WEBHOOK_SECRET)
    .update(signingInput)
    .digest("hex");
  try {
    return crypto.timingSafeEqual(Buffer.from(computed), Buffer.from(expected));
  } catch {
    return false;
  }
}`;
    case "rust":
      return `use hmac::{Hmac, Mac};
use sha2::Sha256;

const WEBHOOK_SECRET: &str = ${rustLiteral(shown)};

fn verify_webhook(raw_body: &[u8], signature_header: &str, timestamp_header: &str) -> bool {
    let expected = match signature_header.strip_prefix("sha256=") {
        Some(value) => value,
        None => return false,
    };
    let body = match std::str::from_utf8(raw_body) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let signing_input = format!("{timestamp_header}.{body}");
    let mut mac = Hmac::<Sha256>::new_from_slice(WEBHOOK_SECRET.as_bytes()).unwrap();
    mac.update(signing_input.as_bytes());
    let computed = hex::encode(mac.finalize().into_bytes());
    computed == expected
}`;
    case "java":
      return `import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;

public class WebhookVerifier {
    private static final String WEBHOOK_SECRET = ${javaLiteral(shown)};

    public static boolean verify(byte[] rawBody, String signatureHeader, String timestampHeader)
            throws Exception {
        if (!signatureHeader.startsWith("sha256=")) {
            return false;
        }
        String expected = signatureHeader.substring("sha256=".length());
        String signingInput = timestampHeader + "." + new String(rawBody, StandardCharsets.UTF_8);
        Mac mac = Mac.getInstance("HmacSHA256");
        mac.init(new SecretKeySpec(WEBHOOK_SECRET.getBytes(StandardCharsets.UTF_8), "HmacSHA256"));
        byte[] digest = mac.doFinal(signingInput.getBytes(StandardCharsets.UTF_8));
        String computed = bytesToHex(digest);
        return java.security.MessageDigest.isEqual(computed.getBytes(), expected.getBytes());
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}`;
  }
}

export function webhookVerificationSnippets(secret: string, visible: boolean) {
  return Object.fromEntries(
    WEBHOOK_SNIPPET_LANGS.map(({ id }) => [id, webhookVerificationSnippet(id, secret, visible)]),
  ) as Record<WebhookSnippetLang, string>;
}
