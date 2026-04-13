import "server-only";
import { cache } from "react";
import { api } from "./client";

export interface Item {
  id: string;
  user_id: string;
  title: string;
  description: string | null;
  created_at: string;
  updated_at: string;
}

export const getItems = cache(async (): Promise<Item[]> => {
  return api<Item[]>("/api/items");
});

export const getItem = cache(async (id: string): Promise<Item> => {
  return api<Item>(`/api/items/${id}`);
});

export async function createItem(data: {
  title: string;
  description?: string;
}): Promise<Item> {
  return api<Item>("/api/items", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateItem(
  id: string,
  data: { title?: string; description?: string }
): Promise<Item> {
  return api<Item>(`/api/items/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteItem(id: string): Promise<void> {
  return api<void>(`/api/items/${id}`, { method: "DELETE" });
}
