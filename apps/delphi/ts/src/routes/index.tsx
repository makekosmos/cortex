import { Route, Routes } from 'react-router-dom';
import AllTaskPage from '@/pages/AllTaskPage';
import CompletedTasksPage from '@/pages/CompletedTasksPage';
import SettingsPage from '@/pages/SettingsPage';

export default function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<AllTaskPage />} />
      <Route path="/completed" element={<CompletedTasksPage />} />
      <Route path="/settings" element={<SettingsPage />} />
    </Routes>
  );
}
