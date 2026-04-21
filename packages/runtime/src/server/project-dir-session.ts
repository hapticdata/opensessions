export function buildDirSessionMap(
  sessions: Iterable<{ dir: string; name: string }>,
): Map<string, string[]> {
  const map = new Map<string, string[]>();

  for (const { dir, name } of sessions) {
    if (!dir) continue;
    const existing = map.get(dir);
    if (existing) {
      if (!existing.includes(name)) existing.push(name);
      continue;
    }
    map.set(dir, [name]);
  }

  return map;
}

function uniqueMatch(matches: Set<string>): string | null {
  return matches.size === 1 ? [...matches][0]! : null;
}

export function resolveSessionForProjectDir(
  projectDir: string,
  dirSessionMap: Map<string, string[]>,
): string | null {
  const exactMatches = new Set<string>(dirSessionMap.get(projectDir) ?? []);
  const exact = uniqueMatch(exactMatches);
  if (exact || exactMatches.size > 1) return exact;

  const relatedMatches = new Set<string>();
  for (const [dir, sessions] of dirSessionMap) {
    if (!projectDir.startsWith(dir + "/") && !dir.startsWith(projectDir + "/")) continue;
    for (const session of sessions) relatedMatches.add(session);
  }
  const related = uniqueMatch(relatedMatches);
  if (related || relatedMatches.size > 1) return related;

  if (!projectDir.startsWith("__encoded__:")) return null;

  const encoded = projectDir.slice("__encoded__:".length);
  const encodedMatches = new Set<string>();
  for (const [dir, sessions] of dirSessionMap) {
    if (dir.replace(/[/._]/g, "-") !== encoded) continue;
    for (const session of sessions) encodedMatches.add(session);
  }

  return uniqueMatch(encodedMatches);
}
