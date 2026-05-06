import { verifySession } from "@/lib/dal";
import { createClient } from "@/lib/supabase/server";
import { TotpEnrollment } from "./totp-enrollment";
import { unenrollFactor } from "./actions";

export const dynamic = "force-dynamic";

export default async function MfaPage() {
  await verifySession();
  const supabase = await createClient();
  const { data, error } = await supabase.auth.mfa.listFactors();

  return (
    <main style={{ maxWidth: 480, margin: "0 auto", padding: "2rem 1rem" }}>
      <h1>Multi-factor auth</h1>

      {error && <p style={{ color: "red" }}>{error.message}</p>}

      <section style={{ marginTop: "1.5rem" }}>
        <h2>Verified factors</h2>
        {!data?.totp?.length ? (
          <p style={{ color: "#666" }}>No TOTP factors yet.</p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0 }}>
            {data.totp.map((factor) => (
              <li
                key={factor.id}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  padding: "0.5rem 0",
                  borderBottom: "1px solid #eee",
                }}
              >
                <span>{factor.friendly_name ?? factor.id}</span>
                <form action={unenrollFactor.bind(null, factor.id)}>
                  <button type="submit">Remove</button>
                </form>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section style={{ marginTop: "2rem" }}>
        <h2>Enroll a TOTP authenticator</h2>
        <TotpEnrollment />
      </section>
    </main>
  );
}
