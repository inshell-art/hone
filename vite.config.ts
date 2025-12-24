import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import istanbul from "vite-plugin-istanbul";

export default () => {
  const shouldCollectCoverage = process.env.VITE_COVERAGE === "true";

  return defineConfig({
    plugins: [
      react(),
      shouldCollectCoverage
        ? istanbul({
            include: "src/**",
            exclude: ["node_modules", "cypress/**"],
            extension: [".js", ".ts", ".tsx"],
            cypress: true,
            requireEnv: true,
          })
        : null,
    ].filter(Boolean),
    build: {
      outDir: "dist",
    },
  });
};
