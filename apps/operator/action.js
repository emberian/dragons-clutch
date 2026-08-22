/* The page's one write: a JSON intent, posted to the local daemon.
 *
 * There is still no transaction builder here and no signer, and there is no
 * code path on this page that could become one. What crosses this boundary is
 * a *description* — a knot, a side, a quantity, a limit, or a pacing verb —
 * and the daemon decides what account roles that needs and hands the result to
 * `clutch_sbf_harness::general_transaction`. The browser never learns the wire
 * format, so it cannot drift from it.
 *
 * A string argument is the pacing form the watch-mode screens use; an object
 * is a trade intent. Both are the same POST. */

export const act = async (request) => {
  const body = typeof request === "string" ? { action: request } : request;
  try {
    const reply = await fetch("/api", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body)
    });
    return await reply.json();
  } catch (error) {
    return { ok: false, detail: String(error) };
  }
};
