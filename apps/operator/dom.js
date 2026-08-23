/* Six DOM helpers, which is the whole of this bench's rendering library.
 *
 * Everything is built with createElement and textContent.  Nothing on this
 * page is ever assembled from a string of markup, so a value that arrives
 * from the daemon — a role name, a refusal message, a signature — is text
 * and can only ever be text. */

export const el = (name, className, text) => {
  const node = document.createElement(name);
  if (className) node.className = className;
  if (text !== undefined && text !== null) node.textContent = String(text);
  return node;
};

export const fill = (node, ...children) => {
  node.replaceChildren(...children.filter(Boolean));
  return node;
};

export const row = (className, ...children) => fill(el("div", className), ...children);

/* A term/value pair, the bench's unit of readout.
 *
 * `value` may be a string or an already-built node (a digest, say). A node is
 * appended rather than stringified: coercing one would print
 * "[object Object]" where an identity belongs, which is exactly the kind of
 * quiet wrongness this bench must not have. */
export const field = (term, value, valueClass) => {
  const pair = el("div", "field");
  const cell = el("dd", valueClass || null);
  if (value && typeof value === "object" && "nodeType" in value) {
    cell.append(value);
  } else {
    cell.textContent = value === null || value === undefined ? "—" : String(value);
  }
  pair.append(el("dt", null, term), cell);
  return pair;
};

export const fields = (className, pairs) => {
  const list = el("dl", `fields ${className || ""}`.trim());
  pairs.forEach(([term, value, valueClass]) => list.append(field(term, value, valueClass)));
  return list;
};

/* Long digests and base58 keys, kept readable without being truncated into a
 * different string: the full value is always the element's title. */
export const digest = (value) => {
  const node = el("code", "digest", value ?? "—");
  if (value) node.title = value;
  return node;
};

const CANONICAL_INTEGER = /^(?:0|-?[1-9]\d*)$/;

export const exactInteger = (value) => {
  if (typeof value === "bigint") return value;
  if (typeof value === "string" && CANONICAL_INTEGER.test(value)) return BigInt(value);
  if (typeof value === "number" && Number.isSafeInteger(value)) return BigInt(value);
  return null;
};

export const numeric = (value) => {
  if (value === null || value === undefined) return "—";
  const exact = exactInteger(value);
  if (exact !== null) return exact.toLocaleString("en-US");
  if (typeof value === "number") return "INVALID UNSAFE NUMBER";
  return String(value);
};

/* Exact arithmetic for visual projections. The inputs remain decimal strings
 * through the calculation; only a bounded percentage in 0..100 becomes a
 * Number for CSS/SVG coordinates. */
export const decimalDifference = (left, right) => {
  const one = exactInteger(left);
  const two = exactInteger(right);
  if (one === null || two === null) return null;
  return (one - two).toString();
};

export const decimalPercent = (part, whole) => {
  const numerator = exactInteger(part);
  const denominator = exactInteger(whole);
  if (numerator === null || denominator === null || denominator <= 0n) return 0;
  const bounded = numerator < 0n ? 0n : numerator > denominator ? denominator : numerator;
  return Number((bounded * 10_000n) / denominator) / 100;
};

export const decimalMax = (values, fallback = "0") => {
  let greatest = exactInteger(fallback);
  if (greatest === null) greatest = 0n;
  values.forEach((value) => {
    const candidate = exactInteger(value);
    if (candidate !== null && candidate > greatest) greatest = candidate;
  });
  return greatest.toString();
};

export const decimalCents = (value) => {
  const cents = exactInteger(value);
  if (cents === null || cents < 0n) return "INVALID CENTS";
  const dollars = cents / 100n;
  const remainder = cents % 100n;
  return remainder === 0n
    ? `$${dollars}`
    : `$${dollars}.${remainder.toString().padStart(2, "0")}`;
};
