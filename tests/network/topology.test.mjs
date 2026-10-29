import { test } from 'node:test';
import assert from 'node:assert/strict';
import { neighbors, distance } from '../../web/src/network/topology.ts';
test('every room size has a symmetric bounded connected graph and greedy routes', () => {
  for (let count = 2; count <= 50; count++) {
    const members = Array.from({ length: count }, (_, i) => ({ id: String(i), order: i }));
    for (const member of members) {
      const links = neighbors(member.id, members);
      assert.equal(links.size, count <= 8 ? count - 1 : 6);
      for (const link of links) assert(neighbors(link, members).has(member.id));
      for (const target of members) if (target.id !== member.id) {
        let current = member.id, hops = 0;
        while (current !== target.id) {
          const choices = [...neighbors(current, members)].sort((a, b) => distance(a, target.id, members) - distance(b, target.id, members));
          assert(distance(choices[0], target.id, members) < distance(current, target.id, members));
          current = choices[0]; assert(++hops <= 16);
        }
      }
    }
    const departed = members.filter((_, i) => i % 4 !== 0);
    for (const member of departed) for (const peer of neighbors(member.id, departed)) assert(neighbors(peer, departed).has(member.id));
  }
});
