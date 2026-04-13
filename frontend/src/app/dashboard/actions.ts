"use server";

import { z } from "zod";
import { createItem, deleteItem } from "@/lib/api/items";
import { createClient } from "@/lib/supabase/server";
import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";

const addItemSchema = z.object({
  title: z.string().trim().min(1, "Title is required").max(255),
});

const removeItemSchema = z.object({
  id: z.string().uuid("Invalid item ID"),
});

export async function addItem(title: string) {
  const parsed = addItemSchema.parse({ title });
  await createItem({ title: parsed.title });
  revalidatePath("/dashboard");
}

export async function removeItem(id: string) {
  const parsed = removeItemSchema.parse({ id });
  await deleteItem(parsed.id);
  revalidatePath("/dashboard");
}

export async function logout() {
  const supabase = await createClient();
  await supabase.auth.signOut();
  redirect("/login");
}
