import { render, screen } from "@testing-library/react";

test("smoke: renders in jsdom", () => {
  render(<div>Smoke</div>);
  expect(screen.getByText("Smoke")).toBeInTheDocument();
});

