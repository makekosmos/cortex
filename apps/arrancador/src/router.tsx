import { lazy, Suspense, type ReactNode } from "react";
import { createBrowserRouter } from "react-router-dom";
import Layout from "@/pages/Layout";

const Library = lazy(() => import("@/pages/Library"));
const Catalogue = lazy(() => import("@/pages/Catalogue"));
const Achievements = lazy(() => import("@/pages/Achievements"));
const GameDetail = lazy(() => import("@/pages/GameDetail"));
const Scan = lazy(() => import("@/pages/Scan"));
const Sqoba = lazy(() => import("@/pages/Sqoba"));
const Statistics = lazy(() => import("@/pages/Statistics"));
const SystemInfo = lazy(() => import("@/pages/SystemInfo"));
const Settings = lazy(() => import("@/pages/Settings"));

function withRouteSuspense(node: ReactNode) {
  return (
    <Suspense
      fallback={
        <div
          data-testid="route-fallback"
          className="min-h-[320px] rounded-2xl border border-border/60 bg-card/50"
        />
      }
    >
      {node}
    </Suspense>
  );
}

export const router = createBrowserRouter([
  {
    path: "/",
    element: <Layout />,
    children: [
      { index: true, element: withRouteSuspense(<Library />) },
      { path: "catalogue", element: withRouteSuspense(<Catalogue />) },
      { path: "achievements", element: withRouteSuspense(<Achievements />) },
      { path: "game/:id", element: withRouteSuspense(<GameDetail />) },
      { path: "scan", element: withRouteSuspense(<Scan />) },
      { path: "sqoba", element: withRouteSuspense(<Sqoba />) },
      { path: "statistics", element: withRouteSuspense(<Statistics />) },
      { path: "system", element: withRouteSuspense(<SystemInfo />) },
      { path: "settings", element: withRouteSuspense(<Settings />) },
    ],
  },
]);
