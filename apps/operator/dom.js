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

export const numeric = (value) =>
  value === null || value === undefined ? "—" : Number(value).toLocaleString("en-US");
