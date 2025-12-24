import net from "node:net";
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";
const shouldCollectCoverage =
  process.argv.includes("--coverage") || process.env.COVERAGE === "true";

const isPortFree = (port) =>
  new Promise((resolve, reject) => {
    const socket = new net.Socket();
    const cleanup = () => {
      socket.removeAllListeners();
      socket.destroy();
    };

    socket.setTimeout(200);

    socket.once("connect", () => {
      cleanup();
      resolve(false);
    });

    socket.once("timeout", () => {
      cleanup();
      resolve(true);
    });

    socket.once("error", (error) => {
      cleanup();
      if (error && error.code === "ECONNREFUSED") {
        resolve(true);
        return;
      }
      if (error && error.code === "EPERM") {
        reject(
          new Error(
            "Local network access is blocked (EPERM). Allow local network access for Terminal/Node.",
          ),
        );
        return;
      }
      resolve(false);
    });

    socket.connect(port, "127.0.0.1");
  });

const getAvailablePort = async () => {
  const startPort = Number(process.env.E2E_PORT_START || 5173);
  const maxAttempts = Number(process.env.E2E_PORT_ATTEMPTS || 20);

  for (let i = 0; i < maxAttempts; i += 1) {
    const port = startPort + i;
    // eslint-disable-next-line no-await-in-loop
    try {
      // eslint-disable-next-line no-await-in-loop
      if (await isPortFree(port)) {
        return port;
      }
    } catch (error) {
      throw error;
    }
  }

  throw new Error("Unable to find a free port for E2E tests.");
};

const waitForServer = async (url, timeoutMs = 30000) => {
  const start = Date.now();

  while (Date.now() - start < timeoutMs) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
    } catch (error) {
      // Server is not ready yet.
    }

    await new Promise((resolve) => setTimeout(resolve, 250));
  }

  throw new Error(`Timed out waiting for ${url}`);
};

const runCoverageReport = () =>
  new Promise((resolve) => {
    const coverageDir = path.join(process.cwd(), ".nyc_output");
    if (
      !shouldCollectCoverage ||
      !fs.existsSync(coverageDir) ||
      fs.readdirSync(coverageDir).length === 0
    ) {
      resolve();
      return;
    }

    const reportProcess = spawn(
      npmCommand,
      [
        "exec",
        "--",
        "nyc",
        "report",
        "--reporter=text-summary",
        "--reporter=lcov",
      ],
      {
        stdio: "inherit",
        env: {
          ...process.env,
        },
      },
    );

    reportProcess.on("exit", () => resolve());
  });

const run = async () => {
  const port = process.env.E2E_PORT
    ? Number(process.env.E2E_PORT)
    : await getAvailablePort();
  const baseUrl = `http://localhost:${port}`;
  const devServerEnv = {
    ...process.env,
    PORT: `${port}`,
    ...(shouldCollectCoverage
      ? { VITE_COVERAGE: "true", CYPRESS_COVERAGE: "true" }
      : {}),
  };

  const devServer = spawn(
    npmCommand,
    ["run", "dev", "--", "--port", `${port}`, "--strictPort"],
    {
      stdio: "inherit",
      env: devServerEnv,
    },
  );

  let cypressProcess = null;
  let finalized = false;

  const cleanup = async (exitCode) => {
    if (finalized) {
      return;
    }
    finalized = true;
    if (devServer && !devServer.killed) {
      devServer.kill("SIGTERM");
    }
    await runCoverageReport();
    if (typeof exitCode === "number") {
      process.exit(exitCode);
    }
  };

  process.on("SIGINT", () => {
    cleanup(130);
  });
  process.on("SIGTERM", () => {
    cleanup(143);
  });

  devServer.on("exit", (code) => {
    if (cypressProcess && !cypressProcess.killed) {
      cypressProcess.kill("SIGTERM");
    }
    cleanup(code ?? 1);
  });

  await waitForServer(baseUrl);

  cypressProcess = spawn(npmCommand, ["run", "cy:run"], {
    stdio: "inherit",
    env: {
      ...process.env,
      BASE_URL: baseUrl,
      ...(shouldCollectCoverage ? { CYPRESS_COVERAGE: "true" } : {}),
    },
  });

  cypressProcess.on("exit", (code) => {
    cleanup(code ?? 1);
  });
};

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
