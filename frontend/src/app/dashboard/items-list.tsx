"use client";

import type { Item } from "@/lib/api/items";
import { useEffect, useState, type FormEvent } from "react";
import { addItem, removeItem } from "./actions";
import { createClient } from "@/lib/supabase/browser";

/**
 * `initialItems` is the SSR snapshot. Once mounted, we subscribe to the
 * `items` table on Supabase Realtime — INSERT/UPDATE/DELETE filtered to the
 * caller's user_id reconcile the local state without a refetch. RLS on
 * `items` still applies on the wire, so other users' rows never reach us.
 */
export function ItemsList({
  initialItems,
  userId,
}: {
  initialItems: Item[];
  userId: string;
}) {
  const [items, setItems] = useState<Item[]>(initialItems);
  const [title, setTitle] = useState("");
  const [pending, setPending] = useState(false);

  useEffect(() => {
    const supabase = createClient();
    const channel = supabase
      .channel(`items:${userId}`)
      .on(
        "postgres_changes",
        {
          event: "*",
          schema: "public",
          table: "items",
          filter: `user_id=eq.${userId}`,
        },
        (payload) => {
          if (payload.eventType === "INSERT") {
            const row = payload.new as Item;
            setItems((prev) =>
              prev.some((i) => i.id === row.id) ? prev : [row, ...prev],
            );
          } else if (payload.eventType === "UPDATE") {
            const row = payload.new as Item;
            setItems((prev) => prev.map((i) => (i.id === row.id ? row : i)));
          } else if (payload.eventType === "DELETE") {
            const row = payload.old as Item;
            setItems((prev) => prev.filter((i) => i.id !== row.id));
          }
        },
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [userId]);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!title.trim()) return;
    setPending(true);
    await addItem(title.trim());
    setTitle("");
    setPending(false);
  }

  async function handleDelete(id: string) {
    await removeItem(id);
  }

  return (
    <section>
      <form
        onSubmit={handleSubmit}
        style={{ display: "flex", gap: "0.5rem", margin: "1rem 0" }}
      >
        <input
          type="text"
          placeholder="New item title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          required
          maxLength={255}
          style={{ flex: 1 }}
        />
        <button type="submit" disabled={pending}>
          {pending ? "Adding..." : "Add"}
        </button>
      </form>

      {items.length === 0 ? (
        <p>No items yet.</p>
      ) : (
        <ul style={{ listStyle: "none", padding: 0 }}>
          {items.map((item) => (
            <li
              key={item.id}
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                padding: "0.75rem 0",
                borderBottom: "1px solid #eee",
              }}
            >
              <div>
                <strong>{item.title}</strong>
                {item.description && (
                  <p style={{ margin: "0.25rem 0 0", color: "#666" }}>
                    {item.description}
                  </p>
                )}
              </div>
              <button onClick={() => handleDelete(item.id)}>Delete</button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
