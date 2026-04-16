import { createBrowserRouter } from "react-router-dom";
import Achievements from "@/pages/Achievements";
import Catalogue from "@/pages/Catalogue";
import GameDetail from "@/pages/GameDetail";
import Layout from "@/pages/Layout";
import Library from "@/pages/Library";
import Scan from "@/pages/Scan";
import Settings from "@/pages/Settings";
import Sqoba from "@/pages/Sqoba";
import Statistics from "@/pages/Statistics";
import SystemInfo from "@/pages/SystemInfo";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <Layout />,
    children: [
      { index: true, element: <Library /> },
      { path: "catalogue", element: <Catalogue /> },
      { path: "achievements", element: <Achievements /> },
      { path: "game/:id", element: <GameDetail /> },
      { path: "scan", element: <Scan /> },
      { path: "sqoba", element: <Sqoba /> },
      { path: "statistics", element: <Statistics /> },
      { path: "system", element: <SystemInfo /> },
      { path: "settings", element: <Settings /> },
    ],
  },
]);
