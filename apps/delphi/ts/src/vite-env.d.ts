/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_URL?: string;
  readonly VITE_GEMINI_API_KEY?: string;
  readonly VITE_GEMINI_LIVE_DEBUG?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
