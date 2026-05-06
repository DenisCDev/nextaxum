"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { createClient } from "@/lib/supabase/browser";

type EnrollState =
  | { phase: "idle" }
  | {
      phase: "challenge";
      factorId: string;
      qrCode: string; // SVG markup
      secret: string;
    }
  | { phase: "done" };

/**
 * Two-step TOTP enrollment using `auth.mfa.enroll` -> `verify` directly from
 * the browser. Supabase generates the secret + QR code (SVG) server-side so
 * we never have to trust the client to keep the seed honest.
 */
export function TotpEnrollment() {
  const router = useRouter();
  const [state, setState] = useState<EnrollState>({ phase: "idle" });
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function startEnrollment() {
    setBusy(true);
    setError(null);
    const supabase = createClient();
    const { data, error } = await supabase.auth.mfa.enroll({
      factorType: "totp",
    });
    setBusy(false);
    if (error || !data) {
      setError(error?.message ?? "enrollment failed");
      return;
    }
    setState({
      phase: "challenge",
      factorId: data.id,
      qrCode: data.totp.qr_code,
      secret: data.totp.secret,
    });
  }

  async function confirm() {
    if (state.phase !== "challenge") return;
    setBusy(true);
    setError(null);
    const supabase = createClient();
    const challenge = await supabase.auth.mfa.challenge({
      factorId: state.factorId,
    });
    if (challenge.error || !challenge.data) {
      setBusy(false);
      setError(challenge.error?.message ?? "challenge failed");
      return;
    }
    const verify = await supabase.auth.mfa.verify({
      factorId: state.factorId,
      challengeId: challenge.data.id,
      code,
    });
    setBusy(false);
    if (verify.error) {
      setError(verify.error.message);
      return;
    }
    setState({ phase: "done" });
    router.refresh();
  }

  if (state.phase === "idle") {
    return (
      <div>
        <button type="button" onClick={startEnrollment} disabled={busy}>
          {busy ? "Generating…" : "Begin enrollment"}
        </button>
        {error && <p style={{ color: "red" }}>{error}</p>}
      </div>
    );
  }

  if (state.phase === "challenge") {
    return (
      <div style={{ display: "grid", gap: "1rem" }}>
        <p style={{ margin: 0 }}>
          Scan the QR code with Google Authenticator / 1Password / Authy.
        </p>
        <div
          // Supabase returns the QR as an SVG string. It's static, server-
          // generated content, but if your CSP forbids inline SVG you can
          // swap this for `data:image/png;base64,${data.totp.qr_code}`.
          dangerouslySetInnerHTML={{ __html: state.qrCode }}
          style={{ width: 180, height: 180 }}
        />
        <details>
          <summary>Can't scan? Type this secret instead</summary>
          <code style={{ display: "block", padding: "0.5rem 0", wordBreak: "break-all" }}>
            {state.secret}
          </code>
        </details>
        <input
          type="text"
          inputMode="numeric"
          pattern="[0-9]{6}"
          maxLength={6}
          placeholder="6-digit code"
          value={code}
          onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))}
          required
        />
        <button type="button" onClick={confirm} disabled={busy || code.length !== 6}>
          {busy ? "Verifying…" : "Confirm"}
        </button>
        {error && <p style={{ color: "red" }}>{error}</p>}
      </div>
    );
  }

  return <p style={{ color: "green" }}>TOTP enrolled. Reload to see it listed.</p>;
}
