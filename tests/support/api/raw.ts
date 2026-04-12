import { recordApiCoverage } from './coverage';

type RawRequestOptions = {
  baseUrl: string;
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
  route: string;
  path?: Record<string, string>;
  query?: Record<string, string | undefined>;
  headers?: Record<string, string>;
};

function buildRoutePath(route: string, path: Record<string, string> = {}): string {
  return route.replace(/\{([^}]+)\}/g, (_match, key: string) => {
    const value = path[key];
    if (value === undefined) {
      throw new Error(`Missing path parameter: ${key}`);
    }
    return encodeURIComponent(value);
  });
}

export async function apiFetchRaw(options: RawRequestOptions): Promise<Response> {
  recordApiCoverage(options.method, options.route);

  const url = new URL(buildRoutePath(options.route, options.path), options.baseUrl);
  for (const [key, value] of Object.entries(options.query ?? {})) {
    if (value !== undefined) {
      url.searchParams.set(key, value);
    }
  }

  return fetch(url, {
    method: options.method,
    headers: options.headers,
  });
}
