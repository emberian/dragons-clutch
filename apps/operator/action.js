/* The page's one write: a pacing request.
 *
 * There is no transaction builder here and no signer. This posts a verb to
 * the local daemon and reads the acknowledgement; the daemon refuses any verb
 * that is not about pacing, so this function cannot be turned into an
 * authoring path by adding a string to it. */

export const act = async (action) => {
  try {
    const reply = await fetch("/api", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ action })
    });
    return await reply.json();
  } catch (error) {
    return { ok: false, detail: String(error) };
  }
};
