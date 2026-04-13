import Link from "next/link";

export default function Home() {
  return (
    <main style={{ maxWidth: 600, margin: "0 auto", padding: "4rem 1rem" }}>
      <h1>NextAxum</h1>
      <p>Next.js 16 + Axum + Supabase template.</p>
      <nav style={{ display: "flex", gap: "1rem", marginTop: "2rem" }}>
        <Link href="/login">Login</Link>
        <Link href="/dashboard">Dashboard</Link>
      </nav>
    </main>
  );
}
