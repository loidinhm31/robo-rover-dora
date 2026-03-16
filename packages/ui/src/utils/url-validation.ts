/**
 * Returns an error message if connecting to `url` from the current page would
 * be blocked by the browser's mixed content policy, or null if the connection
 * is safe.
 *
 * localhost/127.0.0.1 URLs are always allowed (browsers exempt them).
 */
export function detectMixedContent(url: string): string | null {
  if (typeof window === "undefined") return null;
  if (window.location.protocol !== "https:") return null;

  const isInsecure = url.startsWith("ws://") || url.startsWith("http://");
  if (!isInsecure) return null;

  // Browsers exempt loopback addresses from mixed content blocking
  const isLoopback =
    url.includes("localhost") || url.includes("127.0.0.1") || url.includes("[::1]");
  if (isLoopback) return null;

  return `Mixed content blocked: ${url.split("//")[0]}// target on HTTPS page. Use wss:// or https://.`;
}

/**
 * Upgrades an insecure URL to its secure equivalent.
 * ws:// → wss://, http:// → https://
 */
export function suggestSecureUrl(url: string): string {
  return url.replace(/^ws:\/\//, "wss://").replace(/^http:\/\//, "https://");
}
