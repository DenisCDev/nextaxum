import { Suspense } from "react";
import { getItems } from "@/lib/api/items";
import { verifySession } from "@/lib/dal";
import { ItemsList } from "./items-list";
import { logout } from "./actions";

export const dynamic = "force-dynamic";

export default async function DashboardPage() {
  // DAL is the real security layer — not the proxy.
  // verifySession() is cached via React.cache(), so multiple calls
  // in the same render are deduplicated (zero extra cost).
  const { user } = await verifySession();

  return (
    <main style={{ maxWidth: 600, margin: "0 auto", padding: "2rem 1rem" }}>
      <header
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <h1>Dashboard</h1>
        <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
          <span>{user.email}</span>
          <form action={logout}>
            <button type="submit">Logout</button>
          </form>
        </div>
      </header>
      <Suspense fallback={<p style={{ marginTop: "1rem", color: "#666" }}>Loading items...</p>}>
        <ItemsLoader />
      </Suspense>
    </main>
  );
}

async function ItemsLoader() {
  const items = await getItems();
  return <ItemsList initialItems={items} />;
}
