#!/usr/bin/env node
'use strict';

// Remote-operation flows for the Zync React web UI (P0.10 in PLAN.md), sibling
// to audit.cjs. Split into its own file because it needs a second, independent
// git clone (to simulate "someone else pushed to origin") alongside the
// fixture's own working clone, and because it deliberately exercises negative
// / error paths (stale force-with-lease, invalid credential host pattern)
// that would be noisy interleaved with audit.cjs's happy-path click-through.
//
// Uses fixture.cjs's buildFixture({ dirty: false }) - a clean tree matching
// origin/main exactly, so fetch/pull/push never get blocked by the dirty
// tracked/untracked files audit.cjs relies on for its Local Changes flows.
//
// Target origin:
//   E2E_BASE_URL  - defaults to http://127.0.0.1:5173 (Vite dev server).
// Target API (fixture.cjs registration/cleanup only):
//   E2E_API_BASE  - defaults to http://127.0.0.1:58271.

const os = require('node:os');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');
const { buildFixture, cleanup, git } = require('./fixture.cjs');

const BASE_URL = process.env.E2E_BASE_URL || 'http://127.0.0.1:5173';

const results = [];

function record(name, ok, detail) {
  const status = ok ? 'PASS' : 'FAIL';
  const line = detail ? `${status} - ${name}: ${detail}` : `${status} - ${name}`;
  console.log(line);
  results.push({ name, ok, detail });
}

async function step(name, fn) {
  try {
    await fn();
    record(name, true);
  } catch (error) {
    record(name, false, error && error.message ? error.message : String(error));
  }
}

function skip(name, reason) {
  console.log(`SKIP - ${name}: ${reason}`);
}

function writeFile(filePath, contents) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, contents);
}

// Commits `message` (touching a fresh file so there is always something to
// commit) in `cwd` and pushes it to origin/main - used to advance the bare
// repo "behind the fixture's back" from a second clone.
function commitAndPush(cwd, message, fileName) {
  writeFile(path.join(cwd, fileName), `${message}\n`);
  git(['add', fileName], cwd);
  git(['commit', '-m', message], cwd);
  git(['push', 'origin', 'main'], cwd);
}

async function main() {
  const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'zync-e2e-remote-'));
  let fixture = null;
  let browser = null;

  try {
    fixture = await buildFixture(tmpRoot, { dirty: false });
    console.log(
      `Fixture ready: repoId=${fixture.repoId} repoName=${fixture.repoName} workPath=${fixture.workPath} originPath=${fixture.originPath}`,
    );

    // A second, independent clone of the same bare origin - stands in for
    // "another machine" pushing to the remote without the fixture's own
    // working clone (and therefore the browser under test) ever fetching it.
    const secondClonePath = path.join(tmpRoot, 'second-clone');
    git(['clone', `file://${fixture.originPath}`, secondClonePath], tmpRoot);
    git(['config', 'user.name', 'Zync E2E Second Clone'], secondClonePath);
    git(['config', 'user.email', 'zync-e2e-second@example.com'], secondClonePath);
    git(['config', 'commit.gpgsign', 'false'], secondClonePath);

    try {
      browser = await chromium.launch({ channel: 'chrome' });
    } catch {
      browser = await chromium.launch();
    }
    const page = await browser.newPage();

    await step('load app', async () => {
      await page.goto(BASE_URL, { waitUntil: 'domcontentloaded', timeout: 30000 });
      await page.getByTestId('commits-tab').waitFor({ state: 'visible', timeout: 30000 });
    });

    // App.tsx auto-opens the first registered repository on initial page load
    // (independent of whichever repo we go on to explicitly select) via a
    // `useEffect` keyed on `!ws.workspace && ws.repositories.length > 0`. If we
    // switch to the fixture repo before that initial `openRepository()` call
    // resolves, its *delayed* resolution can still land afterward and silently
    // overwrite the active workspace / live-sync socket with the default
    // repo's - observed as the footer notice (and the underlying workspace
    // refresh) jumping back to a stray, unrelated repo's state well after we
    // switched away. Waiting for that initial auto-open to fully settle here,
    // before ever touching the fixture tab, avoids racing it. See the P0.10
    // e2e results write-up for the full repro of this real product race.
    await step('let the auto-opened default repository settle first', async () => {
      await page
        .getByTestId('notice')
        .filter({ hasText: /Live sync (connected|reconnected)/i })
        .waitFor({ timeout: 15000 });
    });

    await step('repo tabs: switch to fixture repo tab', async () => {
      const tab = page.locator(`[data-testid="repo-minibar-item"][data-repo-id="${fixture.repoId}"]`);
      await tab.waitFor({ timeout: 15000 });
      await tab.click();
      await page
        .getByTestId('commit-row')
        .filter({ hasText: 'Extend app.txt' })
        .first()
        .waitFor({ timeout: 15000 });
    });

    // Opening a repo kicks off both the data refresh we just waited on above
    // and, once the workspace id resolves, a separate WebSocket connect
    // (useWorkspace.ts's live-sync effect) whose `onopen` handler unconditionally
    // overwrites the footer `notice` with "Live sync connected" - even if a
    // remote op already set it to e.g. "Push complete" in the meantime. Waiting
    // for that first connect here (a real, one-time event per repo switch)
    // avoids racing it later against Fetch/Pull/Push notices below.
    await step('live sync: wait for websocket to connect before remote ops', async () => {
      await page
        .getByTestId('notice')
        .filter({ hasText: /Live sync (connected|reconnected)/i })
        .waitFor({ timeout: 15000 });
    });

    await step('toolbar: Fetch', async () => {
      await page.getByTestId('toolbar-fetch').click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Fetch complete/i })
        .waitFor({ timeout: 15000 });
    });

    // --- Push -----------------------------------------------------------
    await step('push: create a local commit then toolbar Push', async () => {
      writeFile(path.join(fixture.workPath, 'push-e2e.txt'), 'pushed via e2e\n');
      git(['add', 'push-e2e.txt'], fixture.workPath);
      git(['commit', '-m', 'e2e: push flow commit'], fixture.workPath);

      await page.getByTestId('toolbar-push').click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Push complete/i })
        .waitFor({ timeout: 15000 });
    });

    await step('push: bare origin advanced to match local HEAD', async () => {
      const localSha = git(['rev-parse', 'HEAD'], fixture.workPath).trim();
      const bareSha = git(['rev-parse', 'main'], fixture.originPath).trim();
      if (localSha !== bareSha) {
        throw new Error(`origin/main=${bareSha} does not match local HEAD=${localSha}`);
      }
    });

    // --- Pull (ff-only) ---------------------------------------------------
    await step('pull ff-only: advance origin from second clone, then toolbar Pull', async () => {
      git(['fetch', 'origin'], secondClonePath);
      git(['reset', '--hard', 'origin/main'], secondClonePath);
      commitAndPush(secondClonePath, 'e2e: pull ff-only commit', 'pull-ff-e2e.txt');

      await page.getByTestId('toolbar-pull').click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Pull complete/i })
        .waitFor({ timeout: 15000 });
      await page
        .getByTestId('commit-row')
        .filter({ hasText: 'e2e: pull ff-only commit' })
        .first()
        .waitFor({ timeout: 15000 });
    });

    // --- Pull menu: merge mode -------------------------------------------
    await step('pull menu: merge mode via pull-menu dropdown', async () => {
      commitAndPush(secondClonePath, 'e2e: pull merge-mode commit', 'pull-merge-e2e.txt');

      await page.getByTestId('pull-menu').click();
      await page.getByRole('menuitem', { name: 'Pull (merge)', exact: true }).click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Pull complete/i })
        .waitFor({ timeout: 15000 });
      await page
        .getByTestId('commit-row')
        .filter({ hasText: 'e2e: pull merge-mode commit' })
        .first()
        .waitFor({ timeout: 15000 });
    });

    // --- Pull menu: rebase mode (optional per plan, still asserted) ------
    await step('pull menu: rebase mode via pull-menu dropdown', async () => {
      commitAndPush(secondClonePath, 'e2e: pull rebase-mode commit', 'pull-rebase-e2e.txt');

      await page.getByTestId('pull-menu').click();
      await page.getByRole('menuitem', { name: 'Pull (rebase)', exact: true }).click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Pull complete/i })
        .waitFor({ timeout: 15000 });
      await page
        .getByTestId('commit-row')
        .filter({ hasText: 'e2e: pull rebase-mode commit' })
        .first()
        .waitFor({ timeout: 15000 });
    });

    // --- Force push with lease: accept (no divergence since last fetch) --
    await step('force-with-lease: amend HEAD then Force Push via push-menu', async () => {
      git(['commit', '--amend', '-m', 'e2e: amended for force-with-lease (accept)'], fixture.workPath);

      await page.getByTestId('push-menu').click();
      await page
        .getByRole('menuitem', { name: 'Force Push (with lease)...', exact: true })
        .click();
      const dialog = page.getByTestId('force-push-confirm');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
      await page
        .getByTestId('notice')
        .filter({ hasText: /Push complete/i })
        .waitFor({ timeout: 15000 });
    });

    await step('force-with-lease: bare origin advanced to match amended HEAD', async () => {
      const localSha = git(['rev-parse', 'HEAD'], fixture.workPath).trim();
      const bareSha = git(['rev-parse', 'main'], fixture.originPath).trim();
      if (localSha !== bareSha) {
        throw new Error(`origin/main=${bareSha} does not match amended local HEAD=${localSha}`);
      }
    });

    // --- Force push with lease: reject (remote moved, never fetched) -----
    await step(
      'force-with-lease: reject when remote advanced behind our back without fetching',
      async () => {
        // Advance the bare repo again from the second clone (which fetches
        // first, so *its* push is a clean fast-forward) - simulating another
        // client pushing after our last successful force push above. The
        // browser-driven working clone never fetches this.
        git(['fetch', 'origin'], secondClonePath);
        git(['reset', '--hard', 'origin/main'], secondClonePath);
        commitAndPush(secondClonePath, 'e2e: remote advanced behind our back', 'stale-lease-e2e.txt');

        // Diverge the local branch again so there is something to force-push.
        git(
          ['commit', '--amend', '-m', 'e2e: amended for force-with-lease (reject)'],
          fixture.workPath,
        );

        await page.getByTestId('push-menu').click();
        await page
          .getByRole('menuitem', { name: 'Force Push (with lease)...', exact: true })
          .click();
        const dialog = page.getByTestId('force-push-confirm');
        await dialog.waitFor({ state: 'visible', timeout: 15000 });
        await dialog.getByTestId('dialog-submit').click();
        await dialog.waitFor({ state: 'hidden', timeout: 10000 });

        // Must surface as an error, not hang: the footer notice is set from
        // the caught error's message (see useWorkspace.ts runRemote).
        await page
          .getByTestId('notice')
          .filter({ hasText: /stale/i })
          .waitFor({ timeout: 15000 });
      },
    );

    await step('force-with-lease: bare origin unaffected by the rejected push', async () => {
      const secondCloneSha = git(['rev-parse', 'HEAD'], secondClonePath).trim();
      const bareSha = git(['rev-parse', 'main'], fixture.originPath).trim();
      if (secondCloneSha !== bareSha) {
        throw new Error(
          `origin/main=${bareSha} should still equal the second clone's push (${secondCloneSha}) - the rejected force push must not have landed`,
        );
      }
    });

    await step('UI stays responsive after the rejected force push', async () => {
      // A hang would time out this Fetch, failing the step; a working UI
      // completes it well within the shared 15s convention.
      await page.getByTestId('toolbar-fetch').click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Fetch complete/i })
        .waitFor({ timeout: 15000 });
    });

    // --- Git Tools: Credentials tab ---------------------------------------
    await step('switch to All Commits mode / Git Tools / Credentials tab', async () => {
      await page.getByTestId('commits-tab').click();
      await page.getByTestId('detail-tab-tools').click();
      await page.getByTestId('git-tools-panel').waitFor({ timeout: 10000 });
      await page.getByRole('tab', { name: 'Credentials', exact: true }).click();
      await page.getByTestId('add-credential-btn').waitFor({ timeout: 10000 });
    });

    const credentialLabel = `e2e-cred-${Date.now()}`;
    await step('credentials: add an https_token credential', async () => {
      await page.getByTestId('add-credential-btn').click();
      const dialog = page.getByTestId('credential-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.locator('#credential-label').fill(credentialLabel);
      await dialog.locator('#credential-host').fill('*.example.com');
      await dialog.locator('#credential-token').fill('e2e-dummy');
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
      await page
        .locator(`[data-testid="credential-row"]`)
        .filter({ hasText: credentialLabel })
        .first()
        .waitFor({ timeout: 15000 });
    });

    await step('credentials: delete the credential just added', async () => {
      const row = page
        .locator('[data-testid="credential-row"]')
        .filter({ hasText: credentialLabel })
        .first();
      await row.getByTestId('delete-credential-btn').click();
      const confirmDialog = page.getByTestId('delete-credential-dialog');
      await confirmDialog.waitFor({ state: 'visible', timeout: 15000 });
      await confirmDialog.getByTestId('dialog-submit').click();
      await confirmDialog.waitFor({ state: 'hidden', timeout: 10000 });
      await row.waitFor({ state: 'detached', timeout: 15000 });
    });

    await step('credentials: invalid host pattern shows a field-level error', async () => {
      await page.getByTestId('add-credential-btn').click();
      const dialog = page.getByTestId('credential-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.locator('#credential-label').fill(`e2e-cred-bad-${Date.now()}`);
      await dialog.locator('#credential-host').fill('*bad.com');
      await dialog.locator('#credential-token').fill('e2e-dummy');
      await dialog.getByTestId('dialog-submit').click();

      // Dialog must stay open with an inline field error, not close/succeed.
      await dialog
        .locator('#credential-host')
        .and(page.locator('[aria-invalid="true"]'))
        .waitFor({ timeout: 15000 });
      await dialog.waitFor({ state: 'visible', timeout: 1000 });

      await dialog.getByTestId('dialog-cancel').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
    });

    // --- Git Tools: Remotes tab --------------------------------------------
    await step('remotes tab: origin remote row visible with file:// URL', async () => {
      await page.getByRole('tab', { name: 'Remotes', exact: true }).click();
      const row = page.locator('[data-testid="remote-row"][data-remote="origin"]');
      await row.waitFor({ timeout: 15000 });
      const text = await row.innerText();
      if (!text.includes('file://')) {
        throw new Error(`expected origin remote row to show a file:// URL, got: ${text}`);
      }
    });

    await step('remotes tab: add a second remote', async () => {
      await page.getByTestId('add-remote-btn').click();
      const dialog = page.getByTestId('remote-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.locator('#remote-name').fill('e2e-second');
      await dialog.locator('#remote-url').fill(`file://${fixture.originPath}`);
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
      await page
        .locator('[data-testid="remote-row"][data-remote="e2e-second"]')
        .waitFor({ timeout: 15000 });
    });

    await step('remotes tab: delete the second remote', async () => {
      const row = page.locator('[data-testid="remote-row"][data-remote="e2e-second"]');
      await row.getByTestId('remote-more-btn').click();
      await page.getByRole('menuitem', { name: 'Delete…', exact: true }).click();
      const dialog = page.getByTestId('delete-remote-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
      await row.waitFor({ state: 'detached', timeout: 15000 });
    });

    if (browser) {
      await browser.close();
      browser = null;
    }
  } finally {
    if (browser) {
      await browser.close().catch(() => {});
    }
    if (fixture) {
      try {
        await cleanup(fixture);
        console.log(`Cleanup: removed repository ${fixture.repoId}`);
      } catch (error) {
        console.error(`Cleanup FAILED for repository ${fixture.repoId}: ${error.message}`);
      }
    }
    fs.rmSync(tmpRoot, { recursive: true, force: true });
  }

  const passed = results.filter((r) => r.ok).length;
  const failed = results.filter((r) => !r.ok).length;
  console.log('---');
  console.log(`${passed} passed, ${failed} failed, ${results.length} total`);

  if (failed > 0) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error('FATAL:', error);
  process.exitCode = 1;
});
