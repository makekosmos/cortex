import { createElement, type ElementType, type ReactNode } from "react";

import { gamePosterCardClasses } from "../patterns";

const eyebrowStyle = {
  color: "#ffffff",
} as const;

const titleStyle = {
  color: "#ffffff",
  textShadow: "0 1px 2px rgba(0,0,0,0.38)",
} as const;

export interface GamePosterCardProps {
  to: string;
  title: string;
  eyebrow?: string | null;
  coverSrc?: string | null;
  placeholder?: ReactNode;
  className?: string;
  LinkComponent?: ElementType;
}

function joinClasses(...classes: Array<string | undefined>) {
  return classes.filter(Boolean).join(" ");
}

export function GamePosterCard({
  to,
  title,
  eyebrow,
  coverSrc,
  placeholder,
  className,
  LinkComponent,
}: GamePosterCardProps) {
  const rootClassName = joinClasses(gamePosterCardClasses.root, className);
  const mediaContent = createElement(
    "div",
    { className: gamePosterCardClasses.media },
    coverSrc
      ? createElement("img", {
          src: coverSrc,
          alt: title,
          className: gamePosterCardClasses.image,
          loading: "lazy",
          decoding: "async",
        })
      : createElement(
          "div",
          { className: gamePosterCardClasses.placeholder },
          placeholder,
        ),
    createElement("div", {
      className: gamePosterCardClasses.scrim,
      "aria-hidden": "true",
    }),
    createElement("div", {
      className: gamePosterCardClasses.overlay,
      "aria-hidden": "true",
    }),
  );
  const content = [
    mediaContent,
    createElement(
      "div",
      { className: gamePosterCardClasses.content },
      eyebrow
        ? createElement(
            "div",
            { className: gamePosterCardClasses.eyebrow, style: eyebrowStyle },
            eyebrow,
          )
        : null,
      createElement(
        "div",
        { className: gamePosterCardClasses.title, style: titleStyle },
        title,
      ),
    ),
  ];

  if (LinkComponent) {
    return createElement(
      LinkComponent,
      {
        to,
        "aria-label": title,
        title,
        className: rootClassName,
      },
      ...content,
    );
  }

  return createElement(
    "a",
    {
      href: to,
      "aria-label": title,
      title,
      className: rootClassName,
    },
    ...content,
  );
}
