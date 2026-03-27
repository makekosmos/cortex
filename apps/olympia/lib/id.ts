// Crypto-free UUID replacement for React Native
export function generateId(): string {
  const t = Date.now().toString(36);
  const r1 = Math.random().toString(36).substring(2, 10);
  const r2 = Math.random().toString(36).substring(2, 10);
  return `${t}-${r1}-${r2}`;
}
