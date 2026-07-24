'use strict';

// Builds and tears down a disposable git fixture for the e2e audit:
//   - a bare "origin.git" repository
//   - a working clone ("work") checked out from origin over file://
//   - 3+ commits on main, already pushed to origin
//   - one dirty (modified-but-unstaged) tracked file
//   - one untracked file
//   - the working clone registered with the running zync-server as a repository
//
// This file intentionally only shells out to the system `git` binary and the
// zync-server HTTP API - it does not depend on any other file in this repo,
// so it can be exercised standalone (see README.md) even when the UI dev
// server is not running.

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const API_BASE = process.env.E2E_API_BASE || 'http://127.0.0.1:58271';

function git(args, cwd) {
  try {
    return execFileSync('git', args, {
      cwd,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });
  } catch (error) {
    const stderr = error.stderr ? error.stderr.toString() : '';
    throw new Error(`git ${args.join(' ')} (cwd=${cwd}) failed: ${error.message}\n${stderr}`);
  }
}

function writeFile(filePath, contents) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, contents);
}

/**
 * @param {string} baseDir an empty (or non-existent) directory to build the fixture in
 * @returns {Promise<{repoId: string, repoName: string, workPath: string, originPath: string}>}
 */
// Remove fixtures leaked by aborted runs (killed before cleanup() could run).
async function removeStaleFixtures() {
  try {
    const response = await fetch(`${API_BASE}/repositories`);
    if (!response.ok) return;
    const repos = await response.json();
    for (const repo of repos) {
      if (typeof repo.name === 'string' && repo.name.startsWith('zync-e2e-')) {
        await fetch(`${API_BASE}/repositories/${repo.id}`, { method: 'DELETE' }).catch(() => {});
        console.log(`Removed stale fixture repository: ${repo.name}`);
      }
    }
  } catch {
    // Registry unreachable here fails loudly later in buildFixture anyway.
  }
}

async function buildFixture(baseDir) {
  await removeStaleFixtures();
  fs.mkdirSync(baseDir, { recursive: true });

  const originPath = path.join(baseDir, 'origin.git');
  const workPath = path.join(baseDir, 'work');
  const suffix = `${Date.now()}-${Math.floor(Math.random() * 1e6)}`;
  const repoName = `zync-e2e-${suffix}`;

  // Bare origin.
  git(['init', '--bare', '--initial-branch=main', originPath], baseDir);
  const originUrl = `file://${originPath}`;

  // Working clone via file:// URL.
  git(['clone', originUrl, workPath], baseDir);
  git(['config', 'user.name', 'Zync E2E'], workPath);
  git(['config', 'user.email', 'zync-e2e@example.com'], workPath);
  git(['config', 'commit.gpgsign', 'false'], workPath);

  // Commit 1.
  writeFile(path.join(workPath, 'README.md'), '# Zync E2E fixture\n\nDisposable repository used by tests/e2e.\n');
  git(['add', 'README.md'], workPath);
  git(['commit', '-m', 'Initial commit'], workPath);

  // Commit 2.
  writeFile(path.join(workPath, 'src', 'app.txt'), 'line one\n');
  git(['add', 'src/app.txt'], workPath);
  git(['commit', '-m', 'Add app.txt'], workPath);

  // Commit 3.
  fs.appendFileSync(path.join(workPath, 'src', 'app.txt'), 'line two\n');
  git(['add', 'src/app.txt'], workPath);
  git(['commit', '-m', 'Extend app.txt'], workPath);

  // Push main to origin so Fetch/Pull/Push flows have something real to do.
  git(['push', '-u', 'origin', 'main'], workPath);

  // One dirty (modified, unstaged) tracked file.
  fs.appendFileSync(path.join(workPath, 'src', 'app.txt'), 'line three (uncommitted)\n');

  // One untracked file.
  writeFile(path.join(workPath, 'notes.txt'), 'scratch notes\n');

  // Register the working clone with zync-server.
  const response = await fetch(`${API_BASE}/repositories`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ name: repoName, path: workPath }),
  });
  if (!response.ok) {
    const body = await response.text().catch(() => '');
    throw new Error(
      `POST ${API_BASE}/repositories failed: ${response.status} ${response.statusText}\n${body}`,
    );
  }
  const payload = await response.json();
  const repoId = payload && payload.repository && payload.repository.id;
  if (!repoId) {
    throw new Error(`unexpected response registering repository: ${JSON.stringify(payload)}`);
  }

  return {
    repoId,
    repoName: payload.repository.name || repoName,
    workPath,
    originPath,
  };
}

/**
 * @param {{repoId: string}} fixture the object returned by buildFixture
 */
async function cleanup(fixture) {
  if (!fixture || !fixture.repoId) {
    return;
  }
  const response = await fetch(`${API_BASE}/repositories/${fixture.repoId}`, {
    method: 'DELETE',
  });
  if (response.status !== 204) {
    const body = await response.text().catch(() => '');
    throw new Error(
      `DELETE /repositories/${fixture.repoId} expected 204, got ${response.status}\n${body}`,
    );
  }
}

module.exports = { buildFixture, cleanup, API_BASE };
