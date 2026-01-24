'use strict';

const fs = require('fs');
const path = require('path');

const REQUIRED_UI_ROUTES = [
  '/',
  '/torrents',
  '/torrents/:id',
  '/settings',
  '/logs',
  '/health',
  '/not-found',
];

const API_METHODS = new Set(['get', 'post', 'put', 'patch', 'delete']);

function loadCoverageFiles(dir, prefix) {
  const covered = new Set();
  const files = [];
  if (!fs.existsSync(dir)) {
    return { covered, files };
  }
  for (const entry of fs.readdirSync(dir)) {
    if (!entry.startsWith(prefix) || !entry.endsWith('.json')) {
      continue;
    }
    const fullPath = path.join(dir, entry);
    const raw = fs.readFileSync(fullPath, 'utf-8');
    const items = JSON.parse(raw);
    files.push(entry);
    for (const item of items) {
      covered.add(item);
    }
  }
  return { covered, files };
}

function requiredApiOperations(openapiPath) {
  const raw = fs.readFileSync(openapiPath, 'utf-8');
  const spec = JSON.parse(raw);
  const required = new Set();
  if (!spec.paths) {
    return required;
  }
  for (const [route, methods] of Object.entries(spec.paths)) {
    if (!methods || typeof methods !== 'object') {
      continue;
    }
    for (const method of Object.keys(methods)) {
      if (API_METHODS.has(method)) {
        required.add(`${method.toUpperCase()} ${route}`);
      }
    }
  }
  return required;
}

function assertCoverage(label, required, covered) {
  const missing = [...required].filter((item) => !covered.has(item));
  if (missing.length === 0) {
    return;
  }
  const details = missing.sort().join('\n');
  throw new Error(`${label} coverage missing ${missing.length} entries:\n${details}`);
}

function run() {
  const root = path.resolve(__dirname, '..', '..');
  const resultsDir = process.env.E2E_COVERAGE_DIR || path.join(root, 'tests', 'test-results');

  const openapiPath = path.join(root, 'docs', 'api', 'openapi.json');
  const requiredApi = requiredApiOperations(openapiPath);
  const apiCoverage = loadCoverageFiles(resultsDir, 'api-coverage-');
  if (apiCoverage.files.length === 0) {
    throw new Error('API coverage files were not produced; check the API fixture setup.');
  }
  assertCoverage('API', requiredApi, apiCoverage.covered);

  const uiCoverage = loadCoverageFiles(resultsDir, 'ui-coverage-');
  if (uiCoverage.files.length === 0) {
    throw new Error('UI coverage files were not produced; check the UI fixture setup.');
  }
  assertCoverage('UI', new Set(REQUIRED_UI_ROUTES), uiCoverage.covered);
}

try {
  run();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
