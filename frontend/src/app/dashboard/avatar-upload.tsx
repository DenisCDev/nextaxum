"use client";

import { useActionState } from "react";
import { uploadAvatar, type UploadAvatarState } from "./avatar-actions";

const initial: UploadAvatarState = {};

export function AvatarUpload() {
  const [state, action, pending] = useActionState(uploadAvatar, initial);

  return (
    <form action={action} style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
      <input
        type="file"
        name="avatar"
        accept="image/png,image/jpeg,image/webp"
        required
      />
      <button type="submit" disabled={pending}>
        {pending ? "Uploading…" : "Upload avatar"}
      </button>
      {state.error && <span style={{ color: "red" }}>{state.error}</span>}
      {state.ok && <span style={{ color: "green" }}>Uploaded ✓</span>}
    </form>
  );
}
