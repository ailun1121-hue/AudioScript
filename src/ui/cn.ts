export function cn(...parts: unknown[]): string {
  return parts.filter((p): p is string => typeof p === 'string' && p.length > 0).join(' ')
}
