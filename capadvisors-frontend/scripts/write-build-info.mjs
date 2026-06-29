import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function getGitCommit() {
  try {
    return execSync('git rev-parse HEAD').toString().trim();
  } catch (e) {
    return 'unknown';
  }
}

function getGitBranch() {
  try {
    return execSync('git rev-parse --abbrev-ref HEAD').toString().trim();
  } catch (e) {
    return 'unknown';
  }
}

const buildInfo = {
  commitSha: process.env.VERCEL_GIT_COMMIT_SHA || getGitCommit(),
  commitRef: process.env.VERCEL_GIT_COMMIT_REF || getGitBranch(),
  env: process.env.VERCEL_ENV || 'local',
  deploymentId: process.env.VERCEL_DEPLOYMENT_ID || 'local-dev',
  timestamp: new Date().toISOString()
};

const outputDir = path.join(__dirname, '../public');
if (!fs.existsSync(outputDir)) {
  fs.mkdirSync(outputDir, { recursive: true });
}

fs.writeFileSync(
  path.join(outputDir, 'build-info.json'),
  JSON.stringify(buildInfo, null, 2)
);

console.log('Build info written successfully:', buildInfo);
