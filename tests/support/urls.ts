export function buildInsecureTestUrl(hostname: string): string {
  const url = new URL(`https://${hostname}`);
  url.protocol = 'http:';
  return url.toString();
}
