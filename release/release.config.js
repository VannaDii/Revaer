'use strict';

const path = require('path');

const root = path.resolve(__dirname, '..');
const assets = [
  {
    path: path.join(root, 'dist', 'revaer-app'),
    label: 'revaer-app',
  },
  {
    path: path.join(root, 'dist', 'revaer-app.sha256'),
    label: 'revaer-app.sha256',
  },
  {
    path: path.join(root, 'dist', 'openapi.json'),
    label: 'openapi.json',
  },
  path.join(root, 'dist', 'helm', '*.tgz'),
  path.join(root, 'dist', 'helm', '*.tgz.prov'),
  path.join(root, 'dist', 'helm', 'revaer-helm-public.asc'),
  path.join(root, 'dist', 'helm', 'revaer-helm-public.gpg'),
  path.join(root, 'dist', 'helm', 'artifacthub-repo.yml'),
];

module.exports = {
  branches: [{ name: 'main', prerelease: 'dev' }, 'gh-pages'],
  tagFormat: 'v${version}',
  plugins: [
    [
      '@semantic-release/commit-analyzer',
      {
        preset: 'conventionalcommits',
        releaseRules: [
          { type: 'build', release: 'patch' },
          { type: 'chore', release: 'patch' },
          { type: 'ci', release: 'patch' },
          { type: 'docs', release: 'patch' },
          { type: 'refactor', release: 'patch' },
          { type: 'style', release: 'patch' },
          { type: 'test', release: 'patch' },
          { type: 'revert', release: 'patch' },
        ],
      },
    ],
    ['@semantic-release/release-notes-generator', { preset: 'conventionalcommits' }],
    [
      '@semantic-release/exec',
      {
        prepareCmd:
          'node release/scripts/write-release-info.js "${nextRelease.version}" "${nextRelease.gitTag}" && if [ "$REVAER_ENABLE_HELM_RELEASE_ASSETS" = "1" ]; then bash release/scripts/helm-package.sh "${nextRelease.version}" "${nextRelease.gitTag}"; fi',
      },
    ],
    [
      '@semantic-release/github',
      {
        assets,
        successComment: false,
        releasedLabels: false,
      },
    ],
  ],
};
