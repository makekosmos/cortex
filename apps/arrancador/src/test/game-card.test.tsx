import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { MemoryRouter } from "react-router-dom";
import { vi } from "vitest";

import { createTestGame } from "@/types";

const gamePosterCardSpy = vi.fn(
  ({
    to,
    title,
    eyebrow,
    coverSrc,
    placeholder,
  }: {
    to: string;
    title: string;
    eyebrow?: string | null;
    coverSrc?: string | null;
    placeholder?: ReactNode;
  }) => (
    <div
      data-testid="shared-game-poster-card"
      data-to={to}
      data-title={title}
      data-eyebrow={eyebrow ?? ""}
      data-cover-src={coverSrc ?? ""}
    >
      {placeholder}
    </div>
  ),
);

vi.mock("../../../../packages/kepler-visuals/react", () => ({
  GamePosterCard: (props: Parameters<typeof gamePosterCardSpy>[0]) =>
    gamePosterCardSpy(props),
}));

import { GameCard } from "@/components/GameCard";

describe("GameCard", () => {
  it("delegates the horizontal shared card contract to kepler-visuals", () => {
    const withCover = createTestGame({
      id: "g1",
      name: "Green",
      background_image: "https://example.test/green-wide.jpg",
      cover_image: "https://example.test/green-cover.jpg",
      genres: "Action, RPG",
    });
    const withoutCover = createTestGame({
      id: "g2",
      name: "Yellow",
      background_image: null,
      cover_image: null,
      genres: null,
    });

    render(
      <MemoryRouter>
        <div>
          <GameCard game={withCover} />
          <GameCard game={withoutCover} />
        </div>
      </MemoryRouter>,
    );

    expect(gamePosterCardSpy).toHaveBeenCalledTimes(2);

    const renderedCards = screen.getAllByTestId("shared-game-poster-card");
    expect(renderedCards).toHaveLength(2);

    expect(renderedCards[0]).toHaveAttribute("data-to", "/game/g1");
    expect(renderedCards[0]).toHaveAttribute("data-title", "Green");
    expect(renderedCards[0]).toHaveAttribute("data-eyebrow", "Экшен");
    expect(renderedCards[0]).toHaveAttribute(
      "data-cover-src",
      "https://example.test/green-wide.jpg",
    );

    expect(renderedCards[1]).toHaveAttribute("data-to", "/game/g2");
    expect(renderedCards[1]).toHaveAttribute("data-title", "Yellow");
    expect(renderedCards[1]).toHaveAttribute("data-eyebrow", "");
    expect(renderedCards[1]).toHaveAttribute("data-cover-src", "");
    expect(renderedCards[1].querySelector("svg")).not.toBeNull();
  });
});
