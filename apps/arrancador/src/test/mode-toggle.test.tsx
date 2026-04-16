import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModeToggle } from "@/components/mode-toggle";

const { setThemeMock } = vi.hoisted(() => ({
  setThemeMock: vi.fn(),
}));

vi.mock("@/components/theme-provider", () => ({
  useTheme: () => ({ theme: "dark", setTheme: setThemeMock }),
}));

describe("ModeToggle", () => {
  beforeEach(() => {
    setThemeMock.mockClear();
  });

  it("opens menu and calls setTheme for each option", async () => {
    render(<ModeToggle />);

    await userEvent.click(screen.getByRole("button", { name: "Toggle theme" }));

    await userEvent.click(screen.getByRole("menuitem", { name: "Light" }));
    expect(setThemeMock).toHaveBeenCalledWith("light");

    await userEvent.click(screen.getByRole("button", { name: "Toggle theme" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Dark" }));
    expect(setThemeMock).toHaveBeenCalledWith("dark");

    await userEvent.click(screen.getByRole("button", { name: "Toggle theme" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "System" }));
    expect(setThemeMock).toHaveBeenCalledWith("system");
  });
});
