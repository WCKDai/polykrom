// polykrom.app (and cad.polykrom.app; www.polykrom.app redirects to polykrom.app). Static files
// are served as they are, with two exceptions for Cloudflare's 25 MiB limit on one file:
// - two kinds are stored gzip-compressed as NAME.gz and served here as NAME with
//   Content-Encoding: gzip, which browsers decode: the app's wasm module and a showcase's mesh
//   pack (binary, which Cloudflare does not compress on the way);
// - the Flatpak repository's largest OSTree objects (the app's binaries, already compressed) are
//   stored in pieces as NAME.part0, NAME.part1, ... with a NAME.parts manifest (site/parts.py),
//   and served here as NAME, the pieces joined in order. `flatpak install` and `ostree pull` fetch
//   each object with one plain GET (no byte ranges), so the join is all they need.
export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (url.hostname === "www.polykrom.app") {
      url.hostname = "polykrom.app";
      return Response.redirect(url.toString(), 301);
    }
    const wasm = url.pathname.startsWith("/app/") && url.pathname.endsWith(".wasm");
    const pack = url.pathname.endsWith("/meshes.bin");
    if (wasm || pack) {
      const gz = await env.ASSETS.fetch(new Request(new URL(url.pathname + ".gz", url), request));
      if (gz.ok) {
        return new Response(gz.body, {
          status: 200,
          headers: {
            "Content-Type": wasm ? "application/wasm" : "application/octet-stream",
            "Content-Encoding": "gzip",
            "Cache-Control": "public, max-age=31536000, immutable",
          },
          encodeBody: "manual",
        });
      }
    }
    const response = await env.ASSETS.fetch(request);
    if (response.status === 404 && (request.method === "GET" || request.method === "HEAD")) {
      const joined = await parts(url, request.method, env, ctx);
      if (joined) return joined;
    }
    return response;
  },
};

// The file at `url` stored in pieces (NAME.parts, NAME.part0, ...), served whole; null when it is
// not stored that way.
async function parts(url, method, env, ctx) {
  const manifest = await env.ASSETS.fetch(new Request(new URL(url.pathname + ".parts", url)));
  if (!manifest.ok) return null;
  const { size, parts: count } = await manifest.json();
  const headers = {
    "Content-Type": "application/octet-stream",
    "Content-Length": String(size),
    // Only OSTree objects are split, and they are named by their content hash.
    "Cache-Control": "public, max-age=31536000, immutable",
  };
  if (method === "HEAD") return new Response(null, { status: 200, headers });
  // Workers send Content-Length for a streamed body only through a FixedLengthStream, which also
  // refuses to end short; Node (site/tests/worker_test.mjs) has only TransformStream.
  const { readable, writable } =
    typeof FixedLengthStream === "function" ? new FixedLengthStream(size) : new TransformStream();
  const pump = async () => {
    try {
      for (let i = 0; i < count; i++) {
        const part = await env.ASSETS.fetch(new Request(new URL(`${url.pathname}.part${i}`, url)));
        if (!part.ok) throw new Error(`${url.pathname}.part${i}: HTTP ${part.status}`);
        await part.body.pipeTo(writable, { preventClose: true });
      }
      await writable.close();
    } catch (e) {
      // The client sees a short body (Content-Length is the whole file) and fails the download.
      await writable.abort(e);
    }
  };
  ctx.waitUntil(pump());
  return new Response(readable, { status: 200, headers });
}
