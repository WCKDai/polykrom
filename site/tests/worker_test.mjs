// site/worker.js serves a file stored in pieces by site/parts.py (NAME.parts, NAME.part0, ...)
// as the whole NAME: the round trip through both, on a fake assets binding reading a folder.
//
//     node --test site/tests/worker_test.mjs
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const site = dirname(dirname(fileURLToPath(import.meta.url)));
const worker = (await import(join(site, "worker.js"))).default;

const sha256 = (data) => createHash("sha256").update(data).digest("hex");

// Static assets as Cloudflare serves them: the file at the path, or 404.
function assets(root) {
  return {
    async fetch(request) {
      const path = join(root, new URL(request.url).pathname);
      if (!existsSync(path)) return new Response("not found", { status: 404 });
      const body = readFileSync(path);
      return new Response(request.method === "HEAD" ? null : body, {
        status: 200,
        headers: { "Content-Length": String(body.length) },
      });
    },
  };
}

const ctx = { waitUntil() {} };
const get = (env, path, method = "GET") =>
  worker.fetch(new Request("https://polykrom.app" + path, { method }), env, ctx);

test("a split file is served whole, a missing one is still 404", async () => {
  const root = mkdtempSync(join(tmpdir(), "parts-"));
  try {
    const big = randomBytes(50 * 1024 * 1024 + 12345);
    writeFileSync(join(root, "big.filez"), big);
    writeFileSync(join(root, "small.filez"), "small");
    execFileSync("python3", [join(site, "parts.py"), root]);
    assert.ok(!existsSync(join(root, "big.filez")), "the original is replaced by its pieces");
    assert.ok(existsSync(join(root, "big.filez.part2")) && !existsSync(join(root, "big.filez.part3")));
    assert.ok(existsSync(join(root, "small.filez")), "a small file is left alone");
    assert.deepEqual(JSON.parse(readFileSync(join(root, "big.filez.parts"), "utf8")), { size: big.length, parts: 3 });

    const env = { ASSETS: assets(root) };
    const joined = await get(env, "/big.filez");
    assert.equal(joined.status, 200);
    assert.equal(joined.headers.get("Content-Length"), String(big.length));
    assert.equal(sha256(Buffer.from(await joined.arrayBuffer())), sha256(big));

    const head = await get(env, "/big.filez", "HEAD");
    assert.equal(head.status, 200);
    assert.equal(head.headers.get("Content-Length"), String(big.length));
    assert.equal(head.body, null);

    assert.equal(await (await get(env, "/small.filez")).text(), "small");
    assert.equal((await get(env, "/none.filez")).status, 404);

    // A lost piece makes the body fail short of Content-Length rather than serve a wrong file.
    rmSync(join(root, "big.filez.part1"));
    const broken = await get(env, "/big.filez");
    assert.equal(broken.status, 200);
    await assert.rejects(broken.arrayBuffer());
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
