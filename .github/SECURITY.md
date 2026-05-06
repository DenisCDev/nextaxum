# Security Policy

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security findings.

Email the maintainer at `bedarodrigues83@gmail.com` (or open a private
[security advisory](../../security/advisories/new)). We will respond within
5 business days, agree on a disclosure timeline, and credit you in the
patch notes if you wish.

## Scope

In-scope:
- Bugs in this template's source (`backend/src/`, `frontend/src/`,
  migrations, CI workflows, Dockerfiles).
- Misconfiguration that ships by default — env defaults, headers, RLS,
  rate limits.

Out of scope:
- Issues in upstream dependencies — file those with the upstream project
  and link us if a workaround belongs in this repo.
- Issues that require a malicious operator (someone with `WEBHOOK_SECRET`,
  the service role key, or DB superuser) — those are a configuration
  question, not a bug.

## Hardening checklist already in place

- HSTS, X-Frame-Options DENY, X-Content-Type-Options nosniff, restrictive
  Permissions-Policy, Referrer-Policy strict-origin-when-cross-origin.
- Per-IP rate limit (tower-governor) on the Axum API.
- RLS enabled and FORCED on `items` and `profiles`; auth.users FK with
  cascade.
- Asymmetric JWT verification (RS256/ES256/EdDSA) via JWKS cache, with
  HS256 fallback for legacy projects.
- HMAC-SHA256 + constant-time compare on the webhook receiver.
- Daily `cargo audit` workflow against the RustSec advisory DB.
- Optimistic auth checks at the proxy (`proxy.ts`) but the real boundary
  is the DAL (`lib/dal.ts`) — every Server Action / Route Handler calls
  `verifySession()` before touching data.
