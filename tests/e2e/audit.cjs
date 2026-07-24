#!/usr/bin/env node
'use strict';

// Repeatable Playwright click-through for the Zync React web UI
// (web/apps/web/src). Builds a disposable fixture repository
// (tests/e2e/fixture.cjs), drives the UI through the flows below via stable
// `data-testid` hooks, logs PASS/FAIL per step exactly as it happens, always
// tears the fixture back down, and exits 1 if anything failed.
//
// Target origin:
//   E2E_BASE_URL          - defaults to http://127.0.0.1:5173 (Vite dev
//                            server for web/apps/web). CI can point this at
//                            http://127.0.0.1:58271, where zync-server serves
//                            a production build of the same app same-origin.
// Target API (used only by fixture.cjs to register/remove the fixture repo):
//   E2E_API_BASE           - defaults to http://127.0.0.1:58271.

const os = require('node:os');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');
const { buildFixture, cleanup } = require('./fixture.cjs');

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

// Branch/local-change rows carry an exact-match data attribute (data-branch-name /
// data-path) alongside their data-testid, so lookups don't fall prey to
// substring collisions between fixture names (e.g. a branch and its renamed
// sibling).
function branchRow(page, name) {
  return page.locator(`[data-testid="branch-row"][data-branch-name="${name}"]`);
}

function localChangeRow(page, filePath) {
  return page.locator(`[data-testid="local-change-row"][data-path="${filePath}"]`);
}

async function openBranchMenu(page, branchName) {
  await branchRow(page, branchName).click({ button: 'right' });
  const menu = page.getByRole('menu');
  await menu.waitFor({ state: 'visible', timeout: 15000 });
  return menu;
}

async function clickMenuItem(page, label) {
  await page.getByRole('menuitem', { name: label, exact: true }).click();
}

async function main() {
  const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'zync-e2e-'));
  let fixture = null;
  let browser = null;

  try {
    fixture = await buildFixture(tmpRoot);
    console.log(`Fixture ready: repoId=${fixture.repoId} repoName=${fixture.repoName} workPath=${fixture.workPath}`);

    // Prefer the system Chrome (no separate download); fall back to the
    // Playwright-managed chromium (what CI installs via `playwright install`).
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

    await step('repo tabs: switch to fixture repo tab', async () => {
      const tab = page.locator(`[data-testid="repo-minibar-item"][data-repo-id="${fixture.repoId}"]`);
      await tab.waitFor({ timeout: 15000 });
      await tab.click();
      // Anchor on a commit from the fixture's own history to confirm the
      // workspace actually switched (All Commits is the default center mode).
      await page
        .getByTestId('commit-row')
        .filter({ hasText: 'Extend app.txt' })
        .first()
        .waitFor({ timeout: 15000 });
    });

    await step('commit section: open Local Changes tab', async () => {
      await page.getByTestId('changes-tab').click();
      await localChangeRow(page, 'src/app.txt').waitFor({ timeout: 15000 });
    });

    await step('row select: pick the dirty tracked file', async () => {
      const row = localChangeRow(page, 'src/app.txt');
      await row.locator('code').first().click();
      await page.getByTestId('diff-inline').waitFor({ timeout: 15000 });
    });

    await step('diff toggle: Inline (default)', async () => {
      await page
        .getByTestId('diff-inline')
        .and(page.locator('[aria-pressed="true"]'))
        .waitFor({ timeout: 15000 });
    });

    await step('diff toggle: Split', async () => {
      await page.getByTestId('diff-split').click();
      await page
        .getByTestId('diff-split')
        .and(page.locator('[aria-pressed="true"]'))
        .waitFor({ timeout: 15000 });
    });

    await step('diff toggle: Blame', async () => {
      await page.getByTestId('diff-blame').click();
      await page
        .getByTestId('diff-blame')
        .and(page.locator('[aria-pressed="true"]'))
        .waitFor({ timeout: 15000 });
    });

    await step('diff toggle: back to Inline for hunk staging', async () => {
      await page.getByTestId('diff-inline').click();
      await page
        .getByTestId('diff-inline')
        .and(page.locator('[aria-pressed="true"]'))
        .waitFor({ timeout: 15000 });
    });

    await step('stage hunk on dirty file', async () => {
      const stageHunk = page.getByTestId('stage-hunk').first();
      await stageHunk.waitFor({ timeout: 15000 });
      await stageHunk.click();
    });

    await step('stage untracked file', async () => {
      const row = localChangeRow(page, 'notes.txt');
      await row.waitFor({ timeout: 15000 });
      await row.getByTestId('stage-btn').click();
    });

    await step('commit via footer composer', async () => {
      const input = page.getByTestId('commit-input');
      await input.waitFor({ timeout: 15000 });
      await input.fill('e2e: stage hunk and untracked file');
      await page.getByTestId('commit-btn').click();
      // Commit clears the message input on success.
      await input.and(page.locator('[value=""]')).waitFor({ timeout: 10000 });
    });

    await step('toolbar: Fetch', async () => {
      await page.getByTestId('toolbar-fetch').click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Fetch complete/i })
        .waitFor({ timeout: 15000 });
    });

    await step('toolbar: Pull', async () => {
      await page.getByTestId('toolbar-pull').click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Pull complete/i })
        .waitFor({ timeout: 15000 });
    });

    await step('toolbar: Push', async () => {
      await page.getByTestId('toolbar-push').click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Push complete/i })
        .waitFor({ timeout: 15000 });
    });

    await step('sidebar checkout: click main branch row', async () => {
      await branchRow(page, 'main').click();
    });

    const newBranchName = `e2e-feature-${Date.now()}`;
    await step('New Branch dialog with stash-reapply local mode', async () => {
      await openBranchMenu(page, 'main');
      await clickMenuItem(page, 'New Branch...');
      const dialog = page.getByTestId('new-branch-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.locator('#new-branch-name').fill(newBranchName);
      await dialog.getByTestId('new-branch-local-stash-reapply').click();
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
    });

    await step('sidebar checkout: back to main', async () => {
      await branchRow(page, 'main').click();
    });

    await step('merge dialog: merge new branch into main', async () => {
      await openBranchMenu(page, newBranchName);
      await clickMenuItem(page, 'Merge into current branch...');
      const dialog = page.getByTestId('merge-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
    });

    const tagName = `e2e-tag-${Date.now()}`;
    await step('tag dialog: create tag on main', async () => {
      await openBranchMenu(page, 'main');
      await clickMenuItem(page, 'New Tag...');
      const dialog = page.getByTestId('tag-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.locator('#tag-name').fill(tagName);
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
    });

    const renamedBranchName = `${newBranchName}-renamed`;
    await step('rename dialog: rename feature branch', async () => {
      await openBranchMenu(page, newBranchName);
      await clickMenuItem(page, 'Rename...');
      const dialog = page.getByTestId('rename-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.locator('#rename-branch').fill(renamedBranchName);
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
    });

    await step('delete dialog: delete renamed feature branch', async () => {
      await openBranchMenu(page, renamedBranchName);
      await clickMenuItem(page, 'Delete...');
      const dialog = page.getByTestId('delete-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
    });

    await step('switch to All Commits mode', async () => {
      await page.getByTestId('commits-tab').click();
      await page.getByTestId('commit-list').waitFor({ timeout: 10000 });
    });

    await step('load more commits', async () => {
      await page.getByTestId('load-more').click();
    });

    await step('row select: click a commit row', async () => {
      const row = page.getByTestId('commit-row').first();
      await row.click();
    });

    await step('detail tabs: Commit', async () => {
      await page.getByTestId('detail-tab-commit').click();
      await page.getByText('SHA', { exact: true }).waitFor({ timeout: 15000 });
    });

    await step('detail tabs: Git Tools', async () => {
      await page.getByTestId('detail-tab-tools').click();
      await page.getByTestId('git-tools-panel').waitFor({ timeout: 10000 });
    });

    await step('detail tabs: Repository (stats)', async () => {
      await page.getByTestId('detail-tab-repository').click();
      await page.getByTestId('repo-stats').waitFor({ timeout: 15000 });
    });

    await step('repository stats: commit count card visible', async () => {
      const card = page.getByTestId('repo-stats').getByText('Commits', { exact: true });
      await card.waitFor({ timeout: 15000 });
    });

    // The React app has no stash surface in the sidebar yet (no UI ever sets
    // ActiveDialog "stashApply" in web/apps/web/src/App.tsx) - the Dioxus
    // original's stash-apply-dialog flow has nothing to drive it, so it is
    // logged as a skip rather than a failure.
    skip('stash apply dialog', 'React app has no stash list/apply entry point yet (App.tsx dialog kind "stashApply" is unreachable from the UI)');

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
