import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";

import { AppTitlebar } from "@/components/AppTitlebar";

vi.mock("@/lib/window-chrome", () => ({
  closeWindow: vi.fn(),
  getFallbackWindowChromePlatform: () => "mac",
  minimizeWindow: vi.fn(),
  toggleMaximizeWindow: vi.fn(),
}));

describe("components/AppTitlebar", () => {
  it("toggles the sidebar and routes history controls through callbacks without rendering spotlight", async () => {
    const onToggleSidebar = vi.fn();
    const onBack = vi.fn();
    const onForward = vi.fn();

    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <AppTitlebar
          sidebarHidden={false}
          onToggleSidebar={onToggleSidebar}
          canGoBack
          canGoForward
          onBack={onBack}
          onForward={onForward}
        />
      </MemoryRouter>,
    );

    expect(screen.getByTestId("desktop-sidebar-toggle")).toHaveAccessibleName(
      "\u0421\u043a\u0440\u044b\u0442\u044c \u0431\u043e\u043a\u043e\u0432\u0443\u044e \u043f\u0430\u043d\u0435\u043b\u044c",
    );
    expect(
      screen.queryByRole("button", { name: "Search" }),
    ).not.toBeInTheDocument();

    await userEvent.click(screen.getByTestId("desktop-sidebar-toggle"));
    await userEvent.click(screen.getByTestId("titlebar-history-back"));
    await userEvent.click(screen.getByTestId("titlebar-history-forward"));

    expect(onToggleSidebar).toHaveBeenCalledTimes(1);
    expect(onBack).toHaveBeenCalledTimes(1);
    expect(onForward).toHaveBeenCalledTimes(1);
  });

  it("disables history controls when navigation is unavailable", async () => {
    const onBack = vi.fn();
    const onForward = vi.fn();

    render(
      <MemoryRouter initialEntries={["/"]}>
        <AppTitlebar
          sidebarHidden
          onToggleSidebar={() => {}}
          canGoBack={false}
          canGoForward={false}
          onBack={onBack}
          onForward={onForward}
        />
      </MemoryRouter>,
    );

    const backButton = screen.getByTestId("titlebar-history-back");
    const forwardButton = screen.getByTestId("titlebar-history-forward");

    expect(backButton).toBeDisabled();
    expect(forwardButton).toBeDisabled();

    await userEvent.click(backButton);
    await userEvent.click(forwardButton);

    expect(onBack).not.toHaveBeenCalled();
    expect(onForward).not.toHaveBeenCalled();
  });
});
