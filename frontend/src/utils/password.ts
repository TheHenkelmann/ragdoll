// SPDX-License-Identifier: AGPL-3.0-only

export type PasswordCheck = {
  id: string;
  label: string;
  passed: boolean;
};

export type PasswordEvaluation = {
  checks: PasswordCheck[];
  isValid: boolean;
  score: number;
};

const UPPER = /[A-Z]/;
const LOWER = /[a-z]/;
const DIGIT = /[0-9]/;
const SYMBOL = /[^A-Za-z0-9]/;

export function evaluatePassword(password: string): PasswordEvaluation {
  const checks: PasswordCheck[] = [
    { id: "length", label: "At least 12 characters", passed: password.length >= 12 },
    { id: "upper", label: "Uppercase letter", passed: UPPER.test(password) },
    { id: "lower", label: "Lowercase letter", passed: LOWER.test(password) },
    { id: "digit", label: "Number", passed: DIGIT.test(password) },
    { id: "symbol", label: "Symbol", passed: SYMBOL.test(password) },
  ];
  const passedCount = checks.filter((c) => c.passed).length;
  return {
    checks,
    isValid: checks.every((c) => c.passed),
    score: passedCount / checks.length,
  };
}

const UPPERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWERS = "abcdefghijklmnopqrstuvwxyz";
const DIGITS = "0123456789";
const SYMBOLS = "!@#$%^&*()-_=+[]{}";

function randomChar(pool: string): string {
  const idx = crypto.getRandomValues(new Uint32Array(1))[0]! % pool.length;
  return pool[idx]!;
}

function shuffle(values: string[]): string[] {
  const out = [...values];
  for (let i = out.length - 1; i > 0; i -= 1) {
    const j = crypto.getRandomValues(new Uint32Array(1))[0]! % (i + 1);
    [out[i], out[j]] = [out[j]!, out[i]!];
  }
  return out;
}

export function generatePassword(length = 20): string {
  const required = [
    randomChar(UPPERS),
    randomChar(LOWERS),
    randomChar(DIGITS),
    randomChar(SYMBOLS),
  ];
  const pool = UPPERS + LOWERS + DIGITS + SYMBOLS;
  const rest = Array.from({ length: Math.max(0, length - required.length) }, () => randomChar(pool));
  return shuffle([...required, ...rest]).join("");
}

export function maskSecret(value: string, visibleStart = 12, visibleEnd = 4): string {
  if (value.length <= visibleStart + visibleEnd + 3) {
    return "•".repeat(Math.min(value.length, 8));
  }
  return `${value.slice(0, visibleStart)}…${value.slice(-visibleEnd)}`;
}

/** Obvious fixed mask for one-time API key display. */
export function maskApiKeyToken(): string {
  return "rd_###########";
}
