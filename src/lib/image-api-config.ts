/** The official Images API always requires a key. Compatible servers can opt out. */
export function isOpenAiImageApi(url: string): boolean {
  try { return new URL(url).hostname === 'api.openai.com'; }
  catch { return false; }
}
