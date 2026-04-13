import Link from "next/link";

export default function NotFound() {
  return (
    <main style={{ maxWidth: 600, margin: "0 auto", padding: "4rem 1rem" }}>
      <h1>404</h1>
      <p style={{ color: "#666", marginTop: "0.5rem" }}>Page not found.</p>
      <Link href="/" style={{ marginTop: "1rem", display: "inline-block" }}>
        Go home
      </Link>
    </main>
  );
}
