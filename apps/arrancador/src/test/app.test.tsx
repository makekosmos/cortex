import { render, screen } from "@testing-library/react";
import App from "@/App";

vi.mock("react-router-dom", () => ({
  RouterProvider: ({ router }: { router: unknown }) => (
    <div data-testid="router">{router ? "ok" : "missing"}</div>
  ),
}));

vi.mock("@/router", () => ({
  router: { id: "test-router" },
}));

describe("App", () => {
  it("renders RouterProvider with router", () => {
    render(<App />);
    expect(screen.getByTestId("router")).toHaveTextContent("ok");
  });
});
