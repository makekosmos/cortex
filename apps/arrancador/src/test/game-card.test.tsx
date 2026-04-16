import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { GameCard } from "@/components/GameCard";
import { createTestGame } from "@/types";

describe("GameCard", () => {
  it("renders linked cards with either a cover image or a placeholder shell", () => {
    const withCover = createTestGame({
      id: "g1",
      name: "Green",
      background_image: "https://example.test/green.jpg",
    });
    const withoutCover = createTestGame({
      id: "g2",
      name: "Yellow",
      background_image: null,
      cover_image: null,
    });

    render(
      <MemoryRouter>
        <div>
          <GameCard game={withCover} />
          <GameCard game={withoutCover} />
        </div>
      </MemoryRouter>,
    );

    const linkedCard = screen.getByRole("link", { name: "Green" });
    expect(linkedCard).toHaveAttribute("href", "/game/g1");

    const image = screen.getByRole("img", { name: "Green" });
    expect(image).toHaveAttribute("loading", "lazy");
    expect(image).toHaveAttribute("decoding", "async");

    const placeholderCard = screen.getByRole("link", { name: "Yellow" });
    expect(placeholderCard.querySelector("img")).toBeNull();
  });
});
