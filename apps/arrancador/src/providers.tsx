import { useEffect } from "react";
import { LanguageProvider } from "@/components/language-provider";
import { ThemeProvider } from "@/components/theme-provider";

export default function AppProviders({
  children,
}: {
  children: React.ReactNode;
}) {
  useEffect(() => {
    // Electron build has no updater wiring yet. Keep the provider side-effect free
    // until the main-process updater contract is implemented.
  }, []);

  return (
    <ThemeProvider defaultTheme="dark" storageKey="arrancador-theme">
      <LanguageProvider>{children}</LanguageProvider>
    </ThemeProvider>
  );
}
