import fs from 'fs';
import path from 'path';

export const REQUIRED_UI_ROUTES = [
  '/',
  '/torrents',
  '/torrents/:id',
  '/settings',
  '/logs',
  '/health',
  '/not-found',
];

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

export function setUiCoveragePath(filePath: string): void {
  coveragePath = filePath;
  loaded = false;
  cache = new Set<string>();
}

function normalizeRoute(route: string): string {
  const normalized = route.startsWith('/') ? route : `/${route}`;
  const cleaned = normalized.split('?')[0].split('#')[0];
  const trimmed =
    cleaned.endsWith('/') && cleaned.length > 1 ? cleaned.slice(0, -1) : cleaned;
  if (trimmed === '/' || trimmed === '') {
    return '/';
  }
  if (trimmed.startsWith('/torrents/')) {
    return '/torrents/:id';
  }
  if (REQUIRED_UI_ROUTES.includes(trimmed)) {
    return trimmed;
  }
  return '/not-found';
}

export function recordUiRoute(route: string): void {
  if (!coveragePath) {
    return;
  }
  loadCoverage();
  cache.add(normalizeRoute(route));
  fs.mkdirSync(path.dirname(coveragePath), { recursive: true });
  fs.writeFileSync(coveragePath, JSON.stringify([...cache].sort(), null, 2));
}
