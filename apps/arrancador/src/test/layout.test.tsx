import { render, screen } from "@testing-library/react";
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
  default: () => <button type="button">Search</button>,
}));
vi.mock("@/components/Sidebar", () => ({
  Sidebar: () => <div data-testid="sidebar">Sidebar</div>,
}));

function getSidebarWrapper() {
  const sidebar = screen.getByTestId("sidebar");
  const wrapper = sidebar.parentElement;
  if (!wrapper) throw new Error("missing wrapper");
  return wrapper;
}

describe("Layout", () => {
  it("toggles mobile menu and closes it on route change", async () => {
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

    expect(getSidebarWrapper().className).toContain("-translate-x-full");

    const toggleButton = screen.getByRole("button", {
      name: /^Открыть меню$/i,
    });
    await userEvent.click(toggleButton);
    expect(getSidebarWrapper().className).toContain("translate-x-0");

    const closeButtons = screen.getAllByRole("button", {
      name: /^Закрыть меню$/i,
    });
    const backdrop = closeButtons[1] ?? closeButtons[0];
    await userEvent.click(backdrop);
    expect(getSidebarWrapper().className).toContain("-translate-x-full");

    await userEvent.click(screen.getByRole("link", { name: "Go settings" }));
    expect(await screen.findByText("Settings page")).toBeInTheDocument();
    expect(getSidebarWrapper().className).toContain("-translate-x-full");
  });
});
