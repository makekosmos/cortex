import { render, screen } from "@testing-library/react";
import AppProviders from "@/providers";

describe("AppProviders", () => {
  it("renders children without updater side effects", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(
      <AppProviders>
        <div>Child</div>
      </AppProviders>,
    );

    expect(screen.getByText("Child")).toBeInTheDocument();
    expect(spy).not.toHaveBeenCalled();

    spy.mockRestore();
  });
});
