import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { ThemeProvider } from './features/themeProvider';
import './global.css';
import { BrowserRouter, MemoryRouter } from 'react-router-dom';
import { isElectronRuntime } from '@/services/runtime/platform';

const Router = isElectronRuntime() ? MemoryRouter : BrowserRouter;

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider defaultTheme="system" storageKey="vite-ui-theme">
      <Router>
        <div className="flex h-dvh w-dvw flex-col overflow-hidden">
          <App />
        </div>
      </Router>
    </ThemeProvider>
  </React.StrictMode>,
);
