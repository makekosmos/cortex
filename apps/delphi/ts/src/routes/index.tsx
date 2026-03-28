import { Route, Routes } from "react-router-dom";
import AllTaskPage from "@/pages/AllTaskPage";
import TodayPage from "@/pages/TodayPage";
import UpcomingPage from "@/pages/UpcomingPage";
import LogbookPage from "@/pages/LogbookPage";
import TrashPage from "@/pages/TrashPage";
import SettingsPage from "@/pages/SettingsPage";
import ProjectPage from "@/pages/ProjectPage";

export default function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<AllTaskPage />} />
      <Route path="/today" element={<TodayPage />} />
      <Route path="/upcoming" element={<UpcomingPage />} />
      <Route path="/logbook" element={<LogbookPage />} />
      <Route path="/trash" element={<TrashPage />} />
      <Route path="/settings" element={<SettingsPage />} />
      <Route path="/project/:id" element={<ProjectPage />} />
    </Routes>
  );
}
