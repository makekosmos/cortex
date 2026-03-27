import { Route, Routes } from 'react-router-dom';
import AllTaskPage from '@/pages/AllTaskPage';
import CompletedTasksPage from '@/pages/CompletedTasksPage';
import SettingsPage from '@/pages/SettingsPage';
import UpcomingPage from '@/pages/UpcomingPage';

export default function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<AllTaskPage />} />
      <Route path="/completed" element={<CompletedTasksPage />} />
      <Route path="/upcoming" element={<UpcomingPage />} />
      <Route path="/settings" element={<SettingsPage />} />
    </Routes>
  );
}
