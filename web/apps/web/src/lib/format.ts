import { md5 } from "js-md5"

export { formatCommitTime, shortId } from "./helpers"

// Gravatar needs an md5 of the email — the Web Crypto API doesn't provide md5,
// so we use js-md5. "mp" keeps unknown authors neutral (see helpers.rs).
export function gravatarSrc(email: string, size: number): string | null {
  const normalized = email.trim().toLowerCase()
  if (normalized === "") return null
  return `https://www.gravatar.com/avatar/${md5(normalized)}?s=${size}&d=mp`
}
