import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';

const publicDir = path.resolve('public');
const outputFile = path.join(publicDir, 'build-info.json');

if (!fs.existsSync(publicDir)) {
  fs.mkdirSync(publicDir, { recursive: true });
}

let gitSha = process.env.VERCEL_GIT_COMMIT_SHA;
let branch = process.env.VERCEL_GIT_COMMIT_REF || 'main';

if (!gitSha) {
  try {
    gitSha = execSync('git rev-parse HEAD').toString().trim();
    branch = execSync('git rev-parse --abbrev-ref HEAD').toString().trim();
  } catch {
    gitSha = 'unknown';
  }
}

const buildInfo = {
  gitSha,
  shortSha: gitSha.substring(0, 7),
  branch,
  builtAtUtc: new Date().toISOString(),
  environment: process.env.VERCEL_ENV || 'local',
  deploymentId: process.env.VERCEL_URL || 'local-build',
};

fs.writeFileSync(outputFile, JSON.stringify(buildInfo, null, 2));
console.log(`Build fingerprint written → ${outputFile}`);
