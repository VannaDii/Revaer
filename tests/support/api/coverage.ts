import fs from 'fs';
import path from 'path';

let coveragePath: string | null = null;
let loaded = false;
let cache = new Set<string>();

function loadCoverage(): void {
  if (loaded || !coveragePath) {
    return;
  }
  if (fs.existsSync(coveragePath)) {
    const raw = fs.readFileSync(coveragePath, 'utf-8');
    const items = JSON.parse(raw) as string[];
    cache = new Set(items);
  }
  loaded = true;
}

export function setApiCoveragePath(filePath: string): void {
  coveragePath = filePath;
  loaded = false;
  cache = new Set<string>();
}

export function recordApiCoverage(method: string, route: string): void {
  if (!coveragePath) {
    return;
  }
  loadCoverage();
  cache.add(`${method.toUpperCase()} ${route}`);
  fs.mkdirSync(path.dirname(coveragePath), { recursive: true });
  const payload = JSON.stringify([...cache].sort(), null, 2);
  fs.writeFileSync(coveragePath, payload);
}
