export function encodeTrustedCookieCredential(
  values: Record<string, string | undefined>,
  requiredNames: string[],
): string | null {
  if (
    requiredNames.length === 0 ||
    requiredNames.some((name) => !values[name])
  )
    return null;
  const credential = JSON.stringify(
    Object.fromEntries(requiredNames.map((name) => [name, values[name]])),
  );
  return credential.length <= 2048 ? credential : null;
}

interface IntegrationLoginFlow<TWindow, TResult> {
  createWindow(): TWindow;
  loadLogin(window: TWindow): Promise<void>;
  waitForCredential(window: TWindow): Promise<string>;
  closeWindow(window: TWindow): void;
  persistCredential(credential: string): Promise<TResult>;
}

export async function runIntegrationLogin<TWindow, TResult>(
  flow: IntegrationLoginFlow<TWindow, TResult>,
): Promise<TResult> {
  const window = flow.createWindow();
  let closed = false;
  const closeWindow = () => {
    if (closed) return;
    closed = true;
    flow.closeWindow(window);
  };
  try {
    await flow.loadLogin(window);
    const credential = await flow.waitForCredential(window);
    const result = await flow.persistCredential(credential);
    closeWindow();
    return result;
  } finally {
    closeWindow();
  }
}
