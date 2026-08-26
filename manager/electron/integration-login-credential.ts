type CredentialInput = string | null;
type CookieInput = { name: string; value: string; httpOnly?: boolean };

export function encodeLeetCodeCredential(
  leetcodeSession: CredentialInput,
  csrfToken: CredentialInput,
): string | null {
  if (
    !leetcodeSession ||
    !csrfToken ||
    leetcodeSession.length > 1024 ||
    csrfToken.length > 1024
  )
    return null;
  const credential = JSON.stringify({ session: leetcodeSession, csrfToken });
  return credential.length <= 2048 ? credential : null;
}

export function encodeCookieCredential(input: CookieInput[]): string | null {
  const cookies = input
    .filter(
      ({ name, value, httpOnly }) =>
        value.length > 0 &&
        value.length <= 8192 &&
        !name.includes(";") &&
        !value.includes(";") &&
        (httpOnly || /auth|session|token/i.test(name)),
    )
    .slice(0, 32)
    .map(({ name, value }) => ({ name, value }));
  if (cookies.length === 0) return null;
  const credential = JSON.stringify({ cookies });
  return credential.length <= 16_384 ? credential : null;
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
    closeWindow();
    return await flow.persistCredential(credential);
  } finally {
    closeWindow();
  }
}
