'use strict';

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
const gitTag = process.argv[3];

if (!version || !gitTag) {
  console.error('Missing release version arguments.');
  process.exit(1);
}

const outputPath = path.resolve(__dirname, '..', 'next-release.json');
const payload = {
  version,
  gitTag,
};

fs.writeFileSync(outputPath, `${JSON.stringify(payload, null, 2)}\n`);
