"use client";

import { useActionState } from "react";
import { login, type LoginState } from "./actions";

const initialState: LoginState = {};

export default function LoginPage() {
  const [state, action, pending] = useActionState(login, initialState);

  return (
    <main style={{ maxWidth: 400, margin: "0 auto", padding: "4rem 1rem" }}>
      <h1>Login</h1>
      <form action={action} style={{ display: "grid", gap: "1rem" }}>
        <div>
          <input
            type="email"
            name="email"
            placeholder="Email"
            required
            autoComplete="email"
            style={{ width: "100%" }}
          />
          {state.fieldErrors?.email && (
            <p style={{ color: "red", margin: "0.25rem 0 0" }}>
              {state.fieldErrors.email[0]}
            </p>
          )}
        </div>
        <div>
          <input
            type="password"
            name="password"
            placeholder="Password"
            required
            minLength={8}
            autoComplete="current-password"
            style={{ width: "100%" }}
          />
          {state.fieldErrors?.password && (
            <p style={{ color: "red", margin: "0.25rem 0 0" }}>
              {state.fieldErrors.password[0]}
            </p>
          )}
        </div>
        {state.error && <p style={{ color: "red" }}>{state.error}</p>}
        <button type="submit" disabled={pending}>
          {pending ? "Signing in..." : "Sign in"}
        </button>
      </form>
    </main>
  );
}
