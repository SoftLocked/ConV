export type Member = { id: string; order: number };
export const ROOM_LIMIT = 50;
export const LINK_LIMIT = 8; // Six steady links, two slots for safe rewiring.
export function ordered(members: Member[]) {
  return members.slice().sort((a, b) => a.order - b.order || a.id.localeCompare(b.id));
}
export function neighbors(id: string, members: Member[]): Set<string> {
  const ring = ordered(members);
  const index = ring.findIndex(member => member.id === id);
  if (index < 0) return new Set();
  if (ring.length <= 8) return new Set(ring.filter(member => member.id !== id).map(member => member.id));
  return new Set([-4, -2, -1, 1, 2, 4].map(offset => ring[(index + offset + ring.length) % ring.length].id));
}
export function distance(from: string, to: string, members: Member[]): number {
  const ring = ordered(members);
  const a = ring.findIndex(member => member.id === from), b = ring.findIndex(member => member.id === to);
  if (a < 0 || b < 0) return Infinity;
  const difference = Math.abs(a - b);
  return Math.min(difference, ring.length - difference);
}
