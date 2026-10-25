// polykrom.app (and cad.polykrom.app; www.polykrom.app redirects to polykrom.app). Static files
// are served as they are, except two stored gzip-compressed as
// NAME.gz and served here as NAME with Content-Encoding: gzip, which browsers decode: the app's
// wasm module (larger than Cloudflare's 25 MiB limit for one file) and a showcase's mesh pack
// (binary, which Cloudflare does not compress on the way).
export default {
  async fetch(request, env) {
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
    return env.ASSETS.fetch(request);
  },
};
