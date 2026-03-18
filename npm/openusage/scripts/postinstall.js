#!/usr/bin/env node
"use strict";

const https = require("https");
const http = require("http");
const fs = require("fs");
const path = require("path");
const os = require("os");
const { execSync } = require("child_process");

const PLATFORMS = {
  "linux-x64": "openusage-linux-x64",
  "linux-arm64": "openusage-linux-arm64",
  "darwin-x64": "openusage-darwin-x64",
  "darwin-arm64": "openusage-darwin-arm64",
};

function getPlatformKey() {
  const platform = os.platform();
  const arch = os.arch();
  const archMap = { x64: "x64", arm64: "arm64" };
  const normalizedArch = archMap[arch];

  if (!normalizedArch) {
    throw new Error(
      `Unsupported architecture: ${arch}. openusage supports x64 and arm64.`
    );
  }

  const key = `${platform}-${normalizedArch}`;
  if (!PLATFORMS[key]) {
    throw new Error(
      `Unsupported platform: ${platform} ${arch}. openusage supports Linux and macOS on x64/arm64.`
    );
  }

  return key;
}

function getVersion() {
  const pkg = JSON.parse(
    fs.readFileSync(path.join(__dirname, "..", "package.json"), "utf8")
  );
  return pkg.version;
}

function download(url) {
  return new Promise((resolve, reject) => {
    const get = url.startsWith("https:") ? https.get : http.get;
    get(url, (res) => {
      // Follow redirects (GitHub sends 302 to S3)
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return download(res.headers.location).then(resolve, reject);
      }

      if (res.statusCode !== 200) {
        reject(new Error(`Download failed: HTTP ${res.statusCode} from ${url}`));
        return;
      }

      const chunks = [];
      res.on("data", (chunk) => chunks.push(chunk));
      res.on("end", () => resolve(Buffer.concat(chunks)));
      res.on("error", reject);
    }).on("error", reject);
  });
}

async function main() {
  const key = getPlatformKey();
  const binaryName = PLATFORMS[key];
  const version = getVersion();
  const binDir = path.join(__dirname, "..", "bin");
  const binPath = path.join(binDir, "openusage-bin");

  // Skip if binary already exists (e.g. local development)
  if (fs.existsSync(binPath)) {
    try {
      execSync(`"${binPath}" --version`, { stdio: "ignore" });
      console.log("openusage: binary already exists, skipping download.");
      return;
    } catch {
      // Binary exists but doesn't work — re-download
    }
  }

  const url = `https://github.com/MWZ-Code/openusage-cli/releases/download/v${version}/${binaryName}`;

  console.log(`openusage: downloading ${binaryName} v${version}...`);

  try {
    const data = await download(url);

    fs.mkdirSync(binDir, { recursive: true });
    fs.writeFileSync(binPath, data);
    fs.chmodSync(binPath, 0o755);

    console.log("openusage: installed successfully.");
  } catch (err) {
    console.error(`openusage: failed to download binary from ${url}`);
    console.error(`openusage: ${err.message}`);
    console.error(
      "openusage: you can manually download the binary from https://github.com/MWZ-Code/openusage-cli/releases"
    );
    // Don't fail the install — the wrapper will give a clear error at runtime
  }
}

main();
