import { ApiSession } from './session';

export const API_KEY_HEADER = 'x-revaer-api-key';
export const SETUP_TOKEN_HEADER = 'x-revaer-setup-token';

export function authHeaders(session: ApiSession): Record<string, string> {
  if (!session.apiKey) {
    return {};
  }
  return { [API_KEY_HEADER]: session.apiKey };
}

export function setupHeaders(token: string): Record<string, string> {
  return { [SETUP_TOKEN_HEADER]: token };
}
