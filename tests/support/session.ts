export type AuthMode = 'api_key' | 'none';

export type ApiSession = {
  authMode: AuthMode;
  apiKey?: string;
};
