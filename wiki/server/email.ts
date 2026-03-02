/**
 * Server-side email verification for atproto users.
 *
 * Security context: The `auth.users.email` column is used as an identity
 * key for the wiki's invite/membership system (`members` table matches
 * on email). Allowing users to self-set their email without verification
 * would let them claim other users' pending invites. This module ensures
 * emails are verified before being written to the database.
 *
 * Flow:
 *  1. Authenticated user submits desired email via POST /email/request-verification
 *  2. Server creates a signed JWT containing { sub: userId, email, aud: "email-verify" }
 *  3. Server sends a verification email with a link containing the token
 *  4. User clicks the link → GET /email/verify?token=<jwt>
 *  5. Server verifies signature + expiry, writes email to auth.users via admin API
 *  6. Server responds with a redirect or confirmation page
 *
 * Token approach: Stateless signed JWTs (using jose, already a dependency).
 * No database table needed — the token is self-contained and short-lived.
 *
 * Environment variables:
 *   SMTP_HOST     — SMTP server hostname (default: "localhost")
 *   SMTP_PORT     — SMTP server port (default: 587)
 *   SMTP_USER     — SMTP username (optional, skip auth if unset)
 *   SMTP_PASS     — SMTP password (optional, skip auth if unset)
 *   SMTP_FROM     — Sender address (default: "noreply@overby.me")
 *   SMTP_SECURE   — Use TLS (default: "false"; set "true" for port 465)
 *   EMAIL_SECRET  — Secret for signing verification tokens
 *                   (falls back to HASURA_ADMIN_SECRET if unset)
 *   PUBLIC_URL    — Public base URL of the webhook server
 *                   (e.g. "https://wiki-auth.overby.me")
 *   WIKI_URL      — Public URL of the wiki frontend for redirects
 *                   (default: "https://radikal.wiki")
 */

import * as jose from "jose";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

const HASURA_ENDPOINT = Deno.env.get("HASURA_ENDPOINT");
const HASURA_ADMIN_SECRET = Deno.env.get("HASURA_ADMIN_SECRET");

const SMTP_HOST = Deno.env.get("SMTP_HOST") ?? "localhost";
const SMTP_PORT = Number(Deno.env.get("SMTP_PORT") ?? "587");
const SMTP_USER = Deno.env.get("SMTP_USER");
const SMTP_PASS = Deno.env.get("SMTP_PASS");
const SMTP_FROM = Deno.env.get("SMTP_FROM") ?? "noreply@overby.me";
const SMTP_SECURE = Deno.env.get("SMTP_SECURE") === "true";

const PUBLIC_URL =
	Deno.env.get("PUBLIC_URL") ?? Deno.env.get("WEBHOOK_URL") ?? "";
const WIKI_URL = Deno.env.get("WIKI_URL") ?? "https://radikal.wiki";

/** Token lifetime: 1 hour */
const TOKEN_EXPIRY = "1h";

/**
 * Derive the signing key from the configured secret.
 * Uses EMAIL_SECRET if set, otherwise falls back to HASURA_ADMIN_SECRET.
 */
function getSigningSecret(): Uint8Array {
	const raw = Deno.env.get("EMAIL_SECRET") ?? HASURA_ADMIN_SECRET;
	if (!raw) {
		throw new Error(
			"EMAIL_SECRET or HASURA_ADMIN_SECRET must be set for email verification",
		);
	}
	return new TextEncoder().encode(raw);
}

// ---------------------------------------------------------------------------
// Token creation & verification
// ---------------------------------------------------------------------------

/**
 * Create a signed verification token embedding the user ID and email.
 */
export async function createVerificationToken(
	userId: string,
	email: string,
): Promise<string> {
	const secret = getSigningSecret();

	return await new jose.SignJWT({ email })
		.setProtectedHeader({ alg: "HS256" })
		.setSubject(userId)
		.setAudience("email-verify")
		.setIssuedAt()
		.setExpirationTime(TOKEN_EXPIRY)
		.sign(secret);
}

export interface VerifiedToken {
	userId: string;
	email: string;
}

/**
 * Verify a verification token and extract the payload.
 * Returns null if the token is invalid or expired.
 */
export async function verifyToken(
	token: string,
): Promise<VerifiedToken | null> {
	try {
		const secret = getSigningSecret();
		const { payload } = await jose.jwtVerify(token, secret, {
			audience: "email-verify",
		});

		const userId = payload.sub;
		const email = payload.email as string | undefined;

		if (!userId || !email) {
			console.warn("Verification token missing sub or email claim");
			return null;
		}

		return { userId, email };
	} catch (err) {
		console.warn(
			"Verification token invalid:",
			err instanceof Error ? err.message : err,
		);
		return null;
	}
}

// ---------------------------------------------------------------------------
// Email sending via SMTP (raw socket / STARTTLS)
// ---------------------------------------------------------------------------

/**
 * Send an email using a raw SMTP connection.
 *
 * This is a minimal SMTP client that supports:
 * - Plain SMTP (no encryption)
 * - STARTTLS upgrade
 * - Direct TLS (SMTPS, port 465)
 * - AUTH LOGIN (when SMTP_USER/SMTP_PASS are set)
 *
 * For a small deployment on NixOS with a local MTA (Postfix, etc.),
 * this avoids pulling in a heavy npm dependency like nodemailer.
 */
async function sendEmail(
	to: string,
	subject: string,
	bodyText: string,
	bodyHtml: string,
): Promise<void> {
	const boundary = `----=_Part_${crypto.randomUUID().replace(/-/g, "")}`;

	const message = [
		`From: RadikalWiki <${SMTP_FROM}>`,
		`To: ${to}`,
		`Subject: ${subject}`,
		`MIME-Version: 1.0`,
		`Content-Type: multipart/alternative; boundary="${boundary}"`,
		`Date: ${new Date().toUTCString()}`,
		`Message-ID: <${crypto.randomUUID()}@${SMTP_FROM.split("@")[1] ?? "overby.me"}>`,
		``,
		`--${boundary}`,
		`Content-Type: text/plain; charset=UTF-8`,
		`Content-Transfer-Encoding: 8bit`,
		``,
		bodyText,
		``,
		`--${boundary}`,
		`Content-Type: text/html; charset=UTF-8`,
		`Content-Transfer-Encoding: 8bit`,
		``,
		bodyHtml,
		``,
		`--${boundary}--`,
		``,
	].join("\r\n");

	let conn: Deno.TcpConn | Deno.TlsConn;

	if (SMTP_SECURE) {
		// Direct TLS (port 465 typically)
		conn = await Deno.connectTls({ hostname: SMTP_HOST, port: SMTP_PORT });
	} else {
		conn = await Deno.connect({ hostname: SMTP_HOST, port: SMTP_PORT });
	}

	const encoder = new TextEncoder();
	const decoder = new TextDecoder();

	async function readResponse(): Promise<string> {
		const buf = new Uint8Array(4096);
		let result = "";
		// Read until we get a complete response (line ending with \r\n where
		// the 4th character is a space, not a dash — indicating final line)
		while (true) {
			const n = await conn.read(buf);
			if (n === null) break;
			result += decoder.decode(buf.subarray(0, n));
			// Check if we have a final response line (code + space)
			const lines = result.split("\r\n");
			const lastNonEmpty = lines.filter((l) => l.length > 0).pop();
			if (lastNonEmpty && lastNonEmpty.length >= 4 && lastNonEmpty[3] === " ") {
				break;
			}
		}
		return result;
	}

	async function sendCommand(cmd: string): Promise<string> {
		await conn.write(encoder.encode(cmd + "\r\n"));
		return await readResponse();
	}

	function expectCode(
		response: string,
		expected: number,
		context: string,
	): void {
		const code = parseInt(response.substring(0, 3), 10);
		if (code !== expected) {
			throw new Error(
				`SMTP ${context}: expected ${expected}, got: ${response.trim()}`,
			);
		}
	}

	try {
		// Read server greeting
		const greeting = await readResponse();
		expectCode(greeting, 220, "greeting");

		// EHLO
		const ehlo = await sendCommand(
			`EHLO ${SMTP_FROM.split("@")[1] ?? "localhost"}`,
		);
		expectCode(ehlo, 250, "EHLO");

		// STARTTLS if not already secure
		if (!SMTP_SECURE && ehlo.includes("STARTTLS")) {
			const starttls = await sendCommand("STARTTLS");
			expectCode(starttls, 220, "STARTTLS");

			// Upgrade connection to TLS
			conn = await Deno.startTls(conn as Deno.TcpConn, {
				hostname: SMTP_HOST,
			});

			// Re-EHLO after STARTTLS
			const ehlo2 = await sendCommand(
				`EHLO ${SMTP_FROM.split("@")[1] ?? "localhost"}`,
			);
			expectCode(ehlo2, 250, "EHLO after STARTTLS");
		}

		// AUTH LOGIN if credentials are provided
		if (SMTP_USER && SMTP_PASS) {
			const authResponse = await sendCommand("AUTH LOGIN");
			expectCode(authResponse, 334, "AUTH LOGIN");

			const userResponse = await sendCommand(btoa(SMTP_USER));
			expectCode(userResponse, 334, "AUTH username");

			const passResponse = await sendCommand(btoa(SMTP_PASS));
			expectCode(passResponse, 235, "AUTH password");
		}

		// MAIL FROM
		const mailFrom = await sendCommand(`MAIL FROM:<${SMTP_FROM}>`);
		expectCode(mailFrom, 250, "MAIL FROM");

		// RCPT TO
		const rcptTo = await sendCommand(`RCPT TO:<${to}>`);
		expectCode(rcptTo, 250, "RCPT TO");

		// DATA
		const dataResponse = await sendCommand("DATA");
		expectCode(dataResponse, 354, "DATA");

		// Send message body (dot-stuffing: lines starting with . get an extra .)
		const stuffed = message.replace(/\r\n\./g, "\r\n..");
		await conn.write(encoder.encode(stuffed + "\r\n.\r\n"));
		const doneResponse = await readResponse();
		expectCode(doneResponse, 250, "message accepted");

		// QUIT
		await sendCommand("QUIT");
	} finally {
		try {
			conn.close();
		} catch {
			// Connection may already be closed
		}
	}
}

// ---------------------------------------------------------------------------
// Hasura admin helper (same pattern as users.ts)
// ---------------------------------------------------------------------------

async function hasuraAdmin<T = unknown>(
	query: string,
	variables: Record<string, unknown> = {},
): Promise<T> {
	if (!HASURA_ENDPOINT || !HASURA_ADMIN_SECRET) {
		throw new Error("HASURA_ENDPOINT and HASURA_ADMIN_SECRET must be set");
	}

	const response = await fetch(HASURA_ENDPOINT, {
		method: "POST",
		headers: {
			"Content-Type": "application/json",
			"x-hasura-admin-secret": HASURA_ADMIN_SECRET,
		},
		body: JSON.stringify({ query, variables }),
	});

	if (!response.ok) {
		const text = await response.text();
		throw new Error(
			`Hasura admin request failed (${response.status}): ${text}`,
		);
	}

	const json = (await response.json()) as {
		data?: T;
		errors?: { message: string }[];
	};

	if (json.errors?.length) {
		throw new Error(
			`Hasura GraphQL errors: ${json.errors.map((e) => e.message).join(", ")}`,
		);
	}

	return json.data as T;
}

// ---------------------------------------------------------------------------
// Database operations
// ---------------------------------------------------------------------------

/**
 * Check whether an email address is already in use by another user.
 */
async function isEmailTaken(
	email: string,
	excludeUserId: string,
): Promise<boolean> {
	const data = await hasuraAdmin<{
		users: { id: string }[];
	}>(
		`query CheckEmail($email: citext!, $excludeId: uuid!) {
			users(where: {
				email: { _eq: $email }
				id: { _neq: $excludeId }
			}) {
				id
			}
		}`,
		{ email, excludeId: excludeUserId },
	);

	return data.users.length > 0;
}

/**
 * Write a verified email to auth.users using the admin secret.
 * This is the ONLY path through which a user's email gets set —
 * there is no Hasura permission for users to update their own email.
 */
async function setUserEmail(userId: string, email: string): Promise<void> {
	await hasuraAdmin(
		`mutation SetVerifiedEmail($userId: uuid!, $email: citext!) {
			updateUser(pk_columns: { id: $userId }, _set: { email: $email }) {
				id
			}
		}`,
		{ userId, email },
	);
}

// ---------------------------------------------------------------------------
// Request handlers
// ---------------------------------------------------------------------------

/**
 * POST /email/request-verification
 *
 * Expects: authenticated request (validated by extracting userId from auth
 * headers the same way /validate does) with JSON body { email: string }.
 *
 * Sends a verification email and returns 200 { ok: true }.
 */
export async function handleRequestVerification(
	userId: string,
	body: { email?: string },
): Promise<Response> {
	const email = body.email?.trim().toLowerCase();

	if (!email) {
		return jsonResponse(400, { error: "email is required" });
	}

	// Basic email validation
	if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
		return jsonResponse(400, { error: "invalid email format" });
	}

	if (!PUBLIC_URL) {
		console.error("PUBLIC_URL not set — cannot build verification link");
		return jsonResponse(500, { error: "server configuration error" });
	}

	// Check if the email is already taken by another user
	try {
		if (await isEmailTaken(email, userId)) {
			return jsonResponse(409, {
				error: "this email address is already in use by another account",
			});
		}
	} catch (err) {
		console.error("Failed to check email uniqueness:", err);
		return jsonResponse(500, { error: "failed to validate email" });
	}

	// Create signed verification token
	const token = await createVerificationToken(userId, email);
	const verifyUrl = `${PUBLIC_URL}/email/verify?token=${encodeURIComponent(token)}`;

	// Build email content
	const subject = "Verify your email address — RadikalWiki";

	const bodyText = [
		"Hi,",
		"",
		"You requested to link this email address to your RadikalWiki account.",
		"",
		"Click the link below to verify:",
		verifyUrl,
		"",
		"The link expires in 1 hour.",
		"",
		"If you did not request this, you can ignore this email.",
		"",
		"— RadikalWiki",
	].join("\n");

	const bodyHtml = `<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
  <h2 style="color: #333;">Verify your email address</h2>
  <p>You requested to link this email address to your RadikalWiki account.</p>
  <p>Click the button below to verify:</p>
  <p style="margin: 24px 0;">
    <a href="${escapeHtml(verifyUrl)}"
       style="background-color: #7b1fa2; color: white; padding: 12px 24px;
              text-decoration: none; border-radius: 4px; display: inline-block;">
      Verify email address
    </a>
  </p>
  <p style="color: #666; font-size: 14px;">The link expires in 1 hour.</p>
  <p style="color: #666; font-size: 14px;">
    If you did not request this, you can ignore this email.
  </p>
  <hr style="border: none; border-top: 1px solid #eee; margin: 24px 0;">
  <p style="color: #999; font-size: 12px;">— RadikalWiki</p>
</body>
</html>`;

	try {
		await sendEmail(email, subject, bodyText, bodyHtml);
		console.log(`Verification email sent to ${email} for user ${userId}`);
		return jsonResponse(200, { ok: true });
	} catch (err) {
		console.error("Failed to send verification email:", err);
		return jsonResponse(500, {
			error: "failed to send verification email",
		});
	}
}

/**
 * GET /email/verify?token=<jwt>
 *
 * Validates the signed token and writes the email to auth.users.
 * Returns an HTML page with the result and a redirect link to the wiki.
 */
export async function handleVerifyEmail(
	token: string | null,
): Promise<Response> {
	if (!token) {
		return htmlResponse(
			400,
			"Invalid link",
			"The verification link is missing a token.",
		);
	}

	const verified = await verifyToken(token);
	if (!verified) {
		return htmlResponse(
			400,
			"Invalid or expired link",
			"The verification link is invalid or has expired. Please request a new verification from the wiki.",
		);
	}

	const { userId, email } = verified;

	// Check email is not taken (race condition guard)
	try {
		if (await isEmailTaken(email, userId)) {
			return htmlResponse(
				409,
				"Email address already in use",
				"This email address is already linked to another account.",
			);
		}
	} catch (err) {
		console.error("Failed to check email uniqueness during verify:", err);
		return htmlResponse(
			500,
			"Server error",
			"Could not verify the email address. Please try again later.",
		);
	}

	// Write the verified email to the database
	try {
		await setUserEmail(userId, email);
		console.log(`Email verified and set for user ${userId}: ${email}`);
		return htmlResponse(
			200,
			"Email address verified!",
			`Your email address <strong>${escapeHtml(email)}</strong> has been linked to your account.`,
		);
	} catch (err) {
		console.error(`Failed to set email for user ${userId}:`, err);
		return htmlResponse(
			500,
			"Server error",
			"Could not save the email address. Please try again later.",
		);
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function jsonResponse(status: number, body: Record<string, unknown>): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { "Content-Type": "application/json" },
	});
}

function htmlResponse(
	status: number,
	title: string,
	message: string,
): Response {
	const html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${escapeHtml(title)} — RadikalWiki</title>
  <style>
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      max-width: 500px;
      margin: 60px auto;
      padding: 20px;
      text-align: center;
      color: #333;
    }
    .icon { font-size: 48px; margin-bottom: 16px; }
    h1 { font-size: 24px; margin-bottom: 12px; }
    p { color: #666; line-height: 1.5; }
    a.btn {
      display: inline-block;
      margin-top: 24px;
      padding: 12px 24px;
      background-color: #7b1fa2;
      color: white;
      text-decoration: none;
      border-radius: 4px;
    }
    a.btn:hover { background-color: #6a1b9a; }
  </style>
</head>
<body>
  <div class="icon">${status === 200 ? "✅" : "⚠️"}</div>
  <h1>${escapeHtml(title)}</h1>
  <p>${message}</p>
  <a class="btn" href="${escapeHtml(WIKI_URL)}">Go to RadikalWiki</a>
</body>
</html>`;

	return new Response(html, {
		status,
		headers: { "Content-Type": "text/html; charset=utf-8" },
	});
}

function escapeHtml(str: string): string {
	return str
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;")
		.replace(/"/g, "&quot;")
		.replace(/'/g, "&#x27;");
}
