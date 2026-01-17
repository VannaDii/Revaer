import createClient from 'openapi-fetch';
import type { paths } from './schema';
import { recordApiCoverage } from './coverage';

export type ApiClient = ReturnType<typeof createClient<paths>>;

type ApiClientOptions = {
  baseUrl: string;
  headers?: Record<string, string>;
};

export function createApiClient(options: ApiClientOptions): ApiClient {
  const client = createClient<paths>({
    baseUrl: options.baseUrl,
    headers: options.headers,
  });

  return {
    ...client,
    GET: (...args: Parameters<typeof client.GET>) => {
      recordApiCoverage('GET', String(args[0]));
      return client.GET(...args);
    },
    POST: (...args: Parameters<typeof client.POST>) => {
      recordApiCoverage('POST', String(args[0]));
      return client.POST(...args);
    },
    PUT: (...args: Parameters<typeof client.PUT>) => {
      recordApiCoverage('PUT', String(args[0]));
      return client.PUT(...args);
    },
    PATCH: (...args: Parameters<typeof client.PATCH>) => {
      recordApiCoverage('PATCH', String(args[0]));
      return client.PATCH(...args);
    },
    DELETE: (...args: Parameters<typeof client.DELETE>) => {
      recordApiCoverage('DELETE', String(args[0]));
      return client.DELETE(...args);
    },
  };
}
