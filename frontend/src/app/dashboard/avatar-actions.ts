"use server";

import { revalidatePath } from "next/cache";
import { z } from "zod";
import { verifySession } from "@/lib/dal";
import { createClient } from "@/lib/supabase/server";

const ALLOWED_MIME = new Set(["image/png", "image/jpeg", "image/webp"]);
const MAX_BYTES = 1_048_576;

const InputSchema = z.object({
  filename: z.string().min(1).max(120),
  mime: z.string().refine((v) => ALLOWED_MIME.has(v), "unsupported mime"),
  size: z
    .number()
    .int()
    .positive()
    .max(MAX_BYTES, "file too large (max 1 MB)"),
});

export type UploadAvatarState = { error?: string; ok?: boolean };

export async function uploadAvatar(
  _prev: UploadAvatarState | undefined,
  formData: FormData,
): Promise<UploadAvatarState> {
  const file = formData.get("avatar");
  if (!(file instanceof File)) {
    return { error: "No file provided" };
  }

  const parsed = InputSchema.safeParse({
    filename: file.name,
    mime: file.type,
    size: file.size,
  });
  if (!parsed.success) {
    return { error: parsed.error.issues[0]?.message ?? "invalid file" };
  }

  const { user } = await verifySession();
  const supabase = await createClient();

  const ext = parsed.data.filename.split(".").pop()?.toLowerCase() ?? "bin";
  const path = `${user.id}/avatar.${ext}`;

  const { error: upErr } = await supabase.storage
    .from("avatars")
    .upload(path, file, { contentType: parsed.data.mime, upsert: true });
  if (upErr) {
    return { error: `upload failed: ${upErr.message}` };
  }

  // Mirror the path on profiles.avatar_url. We store the storage path, not a
  // signed URL — pages that render the avatar generate a fresh signed URL
  // server-side so links never get stale or leak.
  const { error: updErr } = await supabase
    .from("profiles")
    .update({ avatar_url: path })
    .eq("id", user.id);
  if (updErr) {
    return { error: `profile update failed: ${updErr.message}` };
  }

  revalidatePath("/dashboard");
  return { ok: true };
}
