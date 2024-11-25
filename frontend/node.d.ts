interface ImportMeta {
  readonly env: {
    DEV: boolean;
    MODE: "dev" | "ic";
    VITE_SATSLINKER_CANISTER_ID: string;
    VITE_SATSLINK_TOKEN_CANISTER_ID: string;
    VITE_II_CANISTER_ID: string;
    VITE_ROOT_KEY: string;
    VITE_IC_HOST: string;
  };
}
