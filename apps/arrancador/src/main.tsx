import "./index.css";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router-dom";
import AppProviders from "@/providers";
import { router } from "@/router";

const root = document.getElementById("root");
if (!root) {
  throw new Error('Missing required root element with id="root".');
}

ReactDOM.createRoot(root).render(
  <AppProviders>
    <RouterProvider router={router} />
  </AppProviders>,
);
