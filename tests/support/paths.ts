import fs from 'fs';
import path from 'path';

export function repoRoot(): string {
  let current = path.resolve(__dirname, '..');
  for (let i = 0; i < 8; i += 1) {
    if (fs.existsSync(path.join(current, 'AGENT.md'))) {
      return current;
    }
    current = path.resolve(current, '..');
  }
  return path.resolve(__dirname, '..', '..');
}

export function resolveFsRoot(): string {
  const fsRoot = process.env.E2E_FS_ROOT ?? '.';
  if (path.isAbsolute(fsRoot)) {
    return fsRoot;
  }
  return path.resolve(repoRoot(), fsRoot);
}
