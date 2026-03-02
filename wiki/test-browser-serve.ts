/**
 * test-browser-serve.ts -- Minimal Deno SPA file server for browser testing.
 *
 * Serves files from dist/ with history API fallback: any request that
 * doesn't match a static file gets dist/index.html instead (standard
 * SPA behaviour matching the _redirects rule and rsbuild's
 * historyApiFallback: true).
 *
 * Usage:
 *   deno run --allow-net --allow-read test-browser-serve.ts [--port 4508]
 */

import { parseArgs } from "jsr:@std/cli@1/parse-args";
import { extname } from "jsr:@std/path@1";

const args = parseArgs(Deno.args, {
	string: ["port"],
	default: { port: "4508" },
});

const PORT = Number(args.port);
const DIST = new URL("./dist/", import.meta.url).pathname;

/** MIME types for common static assets. */
const MIME: Record<string, string> = {
	".html": "text/html; charset=utf-8",
	".js": "application/javascript; charset=utf-8",
	".mjs": "application/javascript; charset=utf-8",
	".css": "text/css; charset=utf-8",
	".json": "application/json; charset=utf-8",
	".png": "image/png",
	".jpg": "image/jpeg",
	".jpeg": "image/jpeg",
	".gif": "image/gif",
	".svg": "image/svg+xml",
	".ico": "image/x-icon",
	".avif": "image/avif",
	".webp": "image/webp",
	".wasm": "application/wasm",
	".woff": "font/woff",
	".woff2": "font/woff2",
	".ttf": "font/ttf",
	".map": "application/json",
};

async function serveFile(path: string): Promise<Response> {
	try {
		const data = await Deno.readFile(path);
		const ext = extname(path);
		const contentType = MIME[ext] ?? "application/octet-stream";
		return new Response(data, {
			headers: {
				"content-type": contentType,
				"access-control-allow-origin": "*",
			},
		});
	} catch {
		return new Response("Not Found", { status: 404 });
	}
}

async function handler(req: Request): Promise<Response> {
	const url = new URL(req.url);
	let pathname = decodeURIComponent(url.pathname);

	// Remove trailing slash (except root)
	if (pathname !== "/" && pathname.endsWith("/")) {
		pathname = pathname.slice(0, -1);
	}

	// Try serving the exact file
	const filePath = DIST + pathname.slice(1);
	try {
		const stat = await Deno.stat(filePath);
		if (stat.isFile) {
			return serveFile(filePath);
		}
		// If directory, try index.html inside it
		if (stat.isDirectory) {
			return serveFile(filePath + "/index.html");
		}
	} catch {
		// File doesn't exist -- fall through to SPA fallback
	}

	// SPA fallback: serve index.html for all unmatched routes
	return serveFile(DIST + "index.html");
}

console.log("SPA server listening on http://127.0.0.1:" + PORT);
Deno.serve({ port: PORT, hostname: "127.0.0.1" }, handler);
