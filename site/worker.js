// cad.polykrom.app. Static files are served as they are, except the app's wasm module: it is
// larger than Cloudflare's 25 MiB limit for one file, so it is stored gzip-compressed as
// NAME.wasm.gz and served here as NAME.wasm with Content-Encoding: gzip, which browsers decode.
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname.startsWith("/app/") && url.pathname.endsWith(".wasm")) {
      const gz = await env.ASSETS.fetch(new Request(new URL(url.pathname + ".gz", url), request));
      if (gz.ok) {
        return new Response(gz.body, {
          status: 200,
          headers: {
            "Content-Type": "application/wasm",
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
