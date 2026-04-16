import { beforeEach, expect, it, vi } from "vitest";

const { renderMock, createRootMock } = vi.hoisted(() => {
  const renderMock = vi.fn();
  const createRootMock = vi.fn(() => ({ render: renderMock }));
  return { renderMock, createRootMock };
});

vi.mock("react-dom/client", () => ({
  default: {
    createRoot: createRootMock,
  },
}));

vi.mock("@/router", () => ({
  router: { __test: true },
}));

vi.mock("@/providers", () => ({
  default: ({ children }: { children: unknown }) => <>{children}</>,
}));

describe("main.tsx", () => {
  beforeEach(() => {
    vi.resetModules();
    document.body.innerHTML = '<div id="root"></div>';
    renderMock.mockClear();
    createRootMock.mockClear();
  });

  it("mounts RouterProvider under AppProviders", async () => {
    await import("@/main");
    expect(createRootMock).toHaveBeenCalledWith(
      document.getElementById("root"),
    );
    expect(renderMock).toHaveBeenCalledTimes(1);
  }, 30_000);

  it("throws when the root element is missing", async () => {
    document.body.innerHTML = "";
    await expect(import("@/main")).rejects.toThrow(
      'Missing required root element with id="root".',
    );
  });
});
