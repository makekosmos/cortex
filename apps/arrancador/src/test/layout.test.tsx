import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Link, MemoryRouter, Route, Routes } from "react-router-dom";
import Layout from "@/pages/Layout";

vi.mock("@/store/GamesContext", () => ({
  GamesProvider: ({ children }: { children: React.ReactNode }) => children,
}));
vi.mock("@/components/ToastProvider", () => ({
  ToastProvider: ({ children }: { children: React.ReactNode }) => children,
}));
vi.mock("@/components/Spotlight", () => ({
  default: ({
    showTrigger = true,
    enableShortcut = true,
  }: {
    showTrigger?: boolean;
    enableShortcut?: boolean;
  }) =>
    showTrigger ? (
      <button
        type="button"
        data-testid="spotlight-trigger"
        data-shortcut-enabled={enableShortcut ? "true" : "false"}
      >
        Search
      </button>
    ) : null,
}));
vi.mock("@/components/Sidebar", () => ({
  SIDEBAR_STORAGE_KEY: "arrancador-sidebar-config",
  sanitizeSidebarConfig: (config?: { width?: number; hidden?: boolean }) => ({
    width:
      typeof config?.width === "number" && Number.isFinite(config.width)
        ? Math.min(320, Math.max(160, config.width))
        : 200,
    hidden: config?.hidden ?? false,
  }),
  Sidebar: ({
    mobile,
    hidden,
    showToggle,
    reserveTopInset,
    enableToggleShortcut,
  }: {
    mobile?: boolean;
    hidden?: boolean;
    showToggle?: boolean;
    reserveTopInset?: boolean;
    enableToggleShortcut?: boolean;
  }) => (
    <div
      data-testid={mobile ? "mobile-sidebar-content" : "desktop-sidebar-content"}
      data-hidden={hidden ? "true" : "false"}
      data-show-toggle={showToggle === false ? "false" : "true"}
      data-reserve-top-inset={reserveTopInset === false ? "false" : "true"}
      data-enable-toggle-shortcut={
        enableToggleShortcut === false ? "false" : "true"
      }
    >
      {mobile ? (
        <Link to="/settings">Mobile settings</Link>
      ) : hidden ? (
        "Desktop sidebar hidden"
      ) : (
        "Desktop sidebar visible"
      )}
    </div>
  ),
}));

describe("Layout", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("opens mobile navigation in a sheet and closes it on route change", async () => {
    render(
      <MemoryRouter initialEntries={["/"]}>
        <Routes>
          <Route path="/" element={<Layout />}>
            <Route index element={<Link to="/settings">Go settings</Link>} />
            <Route path="settings" element={<div>Settings page</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId("app-titlebar")).toBeInTheDocument();
    expect(screen.getByTestId("desktop-titlebar")).toBeInTheDocument();
    expect(screen.getByTestId("desktop-sidebar-content")).toHaveAttribute(
      "data-show-toggle",
      "false",
    );
    expect(screen.getByTestId("desktop-sidebar-content")).toHaveAttribute(
      "data-reserve-top-inset",
      "false",
    );
    expect(
      screen.queryByTestId("mobile-sidebar-content"),
    ).not.toBeInTheDocument();

    await userEvent.click(screen.getByTestId("mobile-menu-toggle"));

    expect(
      await screen.findByTestId("mobile-sidebar-sheet"),
    ).toBeInTheDocument();
    expect(await screen.findByTestId("mobile-sidebar-content")).toHaveAttribute(
      "data-enable-toggle-shortcut",
      "false",
    );

    await userEvent.click(screen.getByRole("link", { name: "Mobile settings" }));
    expect(await screen.findByText("Settings page")).toBeInTheDocument();
    await waitFor(() =>
      expect(
        screen.queryByTestId("mobile-sidebar-content"),
      ).not.toBeInTheDocument(),
    );
  });

  it("keeps sidebar restore in the titlebar only when persisted config starts hidden", async () => {
    localStorage.setItem(
      "arrancador-sidebar-config",
      JSON.stringify({ width: 200, hidden: true }),
    );

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Routes>
          <Route path="/" element={<Layout />}>
            <Route index element={<div>Home</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId("desktop-sidebar-content")).toHaveAttribute(
      "data-hidden",
      "true",
    );
    expect(
      screen.queryByTestId("desktop-sidebar-restore"),
    ).not.toBeInTheDocument();
    expect(
      within(screen.getByTestId("desktop-titlebar")).queryByTestId(
        "spotlight-trigger",
      ),
    ).not.toBeInTheDocument();

    await userEvent.click(screen.getByTestId("desktop-sidebar-toggle"));

    expect(screen.getByTestId("desktop-sidebar-content")).toHaveAttribute(
      "data-hidden",
      "false",
    );
    expect(
      screen.queryByTestId("desktop-sidebar-restore"),
    ).not.toBeInTheDocument();
  });
});
