"use client";

import type { Item } from "@/lib/api/items";
import { useState, type FormEvent } from "react";
import { addItem, removeItem } from "./actions";

export function ItemsList({ initialItems }: { initialItems: Item[] }) {
  const [title, setTitle] = useState("");
  const [pending, setPending] = useState(false);

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

      {initialItems.length === 0 ? (
        <p>No items yet.</p>
      ) : (
        <ul style={{ listStyle: "none", padding: 0 }}>
          {initialItems.map((item) => (
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
