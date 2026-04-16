import { defineComponent, h, type VNode } from "vue";

function icon(paths: VNode[], viewBox = "0 0 20 20") {
  return defineComponent({
    name: "EdenSidebarIcon",
    setup() {
      return () =>
        h(
          "svg",
          {
            fill: "none",
            stroke: "currentColor",
            "stroke-width": "1.8",
            "stroke-linecap": "round",
            "stroke-linejoin": "round",
            viewBox,
            width: "18",
            height: "18",
            "aria-hidden": "true",
          },
          paths,
        );
    },
  });
}

export const PlusIcon = icon([h("path", { d: "M10 4.5v11M4.5 10h11" })]);

export const SearchIcon = icon([
  h("circle", { cx: "9", cy: "9", r: "4.5" }),
  h("path", { d: "m12.5 12.5 3 3" }),
]);

export const SettingsIcon = icon([
  h("circle", { cx: "10", cy: "10", r: "2.5" }),
  h("path", {
    d: "M10 4.5v1.2M10 14.3v1.2M15.5 10h-1.2M5.7 10H4.5M13.89 6.11l-.85.85M6.96 13.04l-.85.85M13.89 13.89l-.85-.85M6.96 6.96l-.85-.85",
  }),
]);

export const BookIcon = icon([
  h("path", { d: "M6 4.5h7a1.5 1.5 0 0 1 1.5 1.5v8.5H7.5A1.5 1.5 0 0 0 6 16" }),
  h("path", { d: "M6 4.5v11.5" }),
  h("path", { d: "M6 6h7" }),
]);

export const TrashIcon = icon([
  h("path", { d: "M5.5 6.5h9" }),
  h("path", { d: "M8 6.5V5.4a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1.1" }),
  h("path", { d: "M7 6.5v8a1 1 0 0 0 1 1h4a1 1 0 0 0 1-1v-8" }),
]);

export const StorageIcon = icon([
  h("ellipse", { cx: "10", cy: "5.5", rx: "5", ry: "2.5" }),
  h("path", { d: "M5 5.5v9c0 1.38 2.24 2.5 5 2.5s5-1.12 5-2.5v-9" }),
  h("path", { d: "M5 10c0 1.38 2.24 2.5 5 2.5s5-1.12 5-2.5" }),
]);

export const LinkIcon = icon([
  h("path", { d: "M8 12.5 6.5 14a2.5 2.5 0 1 1-3.54-3.54L4.5 9" }),
  h("path", { d: "M12 7.5 13.5 6A2.5 2.5 0 1 1 17.04 9.54L15.5 11" }),
  h("path", { d: "M7 13 13 7" }),
]);

export const ShapesIcon = icon([
  h("rect", { x: "3.5", y: "3.5", width: "5", height: "5", rx: "1" }),
  h("circle", { cx: "14", cy: "6", r: "2.5" }),
  h("path", { d: "m10.5 14.5 3.5-5 3.5 5Z" }),
]);

export const GlobeIcon = icon([
  h("circle", { cx: "10", cy: "10", r: "6" }),
  h("path", { d: "M4.7 7.5h10.6M4.7 12.5h10.6M10 4c1.8 1.8 2.8 3.8 2.8 6s-1 4.2-2.8 6" }),
  h("path", { d: "M10 4c-1.8 1.8-2.8 3.8-2.8 6s1 4.2 2.8 6" }),
]);

export const BackIcon = icon([h("path", { d: "M12 4L6 10L12 16" })]);
