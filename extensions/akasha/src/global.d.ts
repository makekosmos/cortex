declare global {
  interface Window {
    kepler?: {
      userData?: {
        readJson<T>(name: string): Promise<T | null>;
        writeJson<T>(name: string, value: T): Promise<void>;
        readBinary(name: string): Promise<string | null>;
        writeBinary(name: string, base64: string): Promise<void>;
        deleteFile(name: string): Promise<boolean>;
      };
      window?: {
        close(): Promise<void>;
        minimize(): Promise<void>;
        maximize(): Promise<void>;
      };
    };
  }
}

export {};
