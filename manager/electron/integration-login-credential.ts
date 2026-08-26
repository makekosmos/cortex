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

export function encodeGreatFrontendCredential(
  input: CookieInput[],
): string | null {
  const token = input.find(
    ({ name, value }) =>
      name === "supabase-auth-token" &&
      value.length > 0 &&
      value.length <= 1_280 &&
      !value.includes(";") &&
      !value.includes("\r") &&
      !value.includes("\n"),
  )?.value;
  return token ?? null;
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
