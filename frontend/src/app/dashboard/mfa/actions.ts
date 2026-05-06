"use server";

import { revalidatePath } from "next/cache";
import { createClient } from "@/lib/supabase/server";
import { verifySession } from "@/lib/dal";

export async function unenrollFactor(factorId: string) {
  await verifySession();
  const supabase = await createClient();
  const { error } = await supabase.auth.mfa.unenroll({ factorId });
  if (error) throw error;
  revalidatePath("/dashboard/mfa");
}
