"use client";

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <main style={{ maxWidth: 600, margin: "0 auto", padding: "4rem 1rem" }}>
      <h1>Something went wrong</h1>
      <p style={{ color: "#666", marginTop: "0.5rem" }}>{error.message}</p>
      <button onClick={reset} style={{ marginTop: "1rem" }}>
        Try again
      </button>
    </main>
  );
}
