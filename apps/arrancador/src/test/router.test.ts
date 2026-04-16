let capturedRoutes: unknown = null;

// Keep this test fast and deterministic: avoid importing heavy page modules.
vi.mock("@/pages/GameDetail", () => ({ default: () => null }));
vi.mock("@/pages/Layout", () => ({ default: () => null }));
vi.mock("@/pages/Library", () => ({ default: () => null }));
vi.mock("@/pages/Scan", () => ({ default: () => null }));
vi.mock("@/pages/Settings", () => ({ default: () => null }));
vi.mock("@/pages/Sqoba", () => ({ default: () => null }));
vi.mock("@/pages/Statistics", () => ({ default: () => null }));
vi.mock("@/pages/SystemInfo", () => ({ default: () => null }));

vi.mock("react-router-dom", () => ({
  createBrowserRouter: (routes: unknown) => {
    capturedRoutes = routes;
    return { routes };
  },
}));

describe("router", () => {
  it("defines the expected route structure", async () => {
    const { router } = await import("@/router");

    expect(router).toBeTruthy();
    expect(Array.isArray((router as { routes?: unknown }).routes)).toBe(true);

    const routes = capturedRoutes as Array<{
      path?: string;
      children?: unknown;
    }>;
    expect(routes[0]?.path).toBe("/");

    const children = routes[0]?.children as Array<{
      path?: string;
      index?: boolean;
    }>;
    expect(children.some((r) => r.index === true)).toBe(true);
    expect(children.some((r) => r.path === "scan")).toBe(true);
    expect(children.some((r) => r.path === "settings")).toBe(true);
  }, 20_000);
});
