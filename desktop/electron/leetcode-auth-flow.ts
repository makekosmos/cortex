export interface LeetCodeLoginFlow<TWindow, TResult> {
  clearCookies(): Promise<void>;
  createWindow(): TWindow;
  loadLogin(window: TWindow): Promise<void>;
  waitForCredential(window: TWindow): Promise<string>;
  closeWindow(window: TWindow): void;
  persistCredential(credential: string): Promise<TResult>;
}

export async function runLeetCodeLogin<TWindow, TResult>(
  flow: LeetCodeLoginFlow<TWindow, TResult>,
): Promise<TResult> {
  await flow.clearCookies();
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
