import { defineConfig } from "cypress";
import codeCoverageTask from "@cypress/code-coverage/task.js";

const baseUrl = process.env.BASE_URL || "http://localhost:5173";

export default defineConfig({
  e2e: {
    baseUrl,
    specPattern: "cypress/e2e/**/*.spec.ts",
    setupNodeEvents(on, config) {
      codeCoverageTask(on, config);
      return config;
    },
  },
  //config env to run:
  // 1, pre-commit to check lint, prettier and type-check as "quality check"
  // * the quality-check on local machine
  // 2, pre-push, built and run e2e to confirm the app is working where is the emu env as "deployment workflow";
  // * the deployment-workflow on emu locally
  // 3, after push, run quality-check in github action
  // * the quality-check on github action machine to avoid "it works on my machine" problem
  // 4, if the step before works, run deployment-workflow against to firebase staging env
  // * the deployment-workflow on github action machine and against to firebase staging env
  // 5, if the step before works, run deployment-workflow against to firebase prod env
  // * the deployment-workflow on github action machine and against to firebase prod env
  component: {
    devServer: {
      framework: "react",
      bundler: "vite",
    },
  },
});
