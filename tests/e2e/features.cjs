#!/usr/bin/env node
'use strict';

// P1 feature flows for the Zync React web UI (P1.8 in PLAN.md), sibling to
// audit.cjs / remote.cjs. Covers the surfaces that shipped in P1.1-P1.7:
// tags (sidebar create/copy-sha/delete), commit search (in-graph + all-
// history), the diff file tree, image diff, per-file history + blame, and
// interactive rebase (including the drop-then-squash guard).
//
// Builds its own dedicated fixture (buildFixture(..., { dirty: false })) so
// interactive rebase's "working tree must be clean" guard has a clean tree
// to start from, then extends it with plain `git` calls (via fixture.cjs's
// exported `git()` helper) for a baseline+modified image pair. Never touches
// the pre-existing registered repositories (zync, Orca, appmo, vane) - see
// README.md "Safety".
//
// Target origin:
//   E2E_BASE_URL  - defaults to http://127.0.0.1:5173 (Vite dev server).
// Target API (fixture.cjs registration/cleanup only):
//   E2E_API_BASE  - defaults to http://127.0.0.1:58271.

const os = require('node:os');
const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');
const { chromium } = require('playwright');
const { buildFixture, cleanup, git, API_BASE } = require('./fixture.cjs');

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

// --- tiny dependency-free PNG encoder (solid-color square) -----------------
// Produces a real, valid PNG (not just bytes with a .png extension) so the
// <img> element the UI renders actually decodes - not just present in the DOM.
function crc32(buf) {
  const table = crc32.table || (crc32.table = (() => {
    const t = [];
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? (0xedb88320 ^ (c >>> 1)) : c >>> 1;
      t[n] = c;
    }
    return t;
  })());
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) crc = table[(crc ^ buf[i]) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}
function pngChunk(type, data) {
  const typeBuf = Buffer.from(type, 'ascii');
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crcBuf]);
}
function makePng(r, g, b, size = 4) {
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 2; // color type: truecolor (RGB)
  const rowLen = 1 + size * 3;
  const raw = Buffer.alloc(rowLen * size);
  for (let y = 0; y < size; y++) {
    const rowStart = y * rowLen;
    raw[rowStart] = 0; // filter: none
    for (let x = 0; x < size; x++) {
      raw[rowStart + 1 + x * 3] = r;
      raw[rowStart + 1 + x * 3 + 1] = g;
      raw[rowStart + 1 + x * 3 + 2] = b;
    }
  }
  const idat = zlib.deflateSync(raw);
  return Buffer.concat([
    sig,
    pngChunk('IHDR', ihdr),
    pngChunk('IDAT', idat),
    pngChunk('IEND', Buffer.alloc(0)),
  ]);
}

function writeFile(filePath, contents) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, contents);
}

// Forces a client-driven SCOPE_ALL refresh (see useWorkspace.ts's runRemote,
// which calls refresh() directly on success - not dependent on the
// live-sync websocket) after a filesystem change made *outside* the
// browser (raw fs writes / `git` CLI calls below), so the UI actually
// reflects it. NOTE: relying on the live-sync file watcher to pick these
// changes up organically (matching a real user editing files in another
// app while Zync is open) was tried first and never worked in this
// environment - see the P1.8 results write-up's "file watcher" finding.
// Toolbar Fetch is used as the trigger because it's always visible, safe to
// spam (the fixture's origin never changes), and its success handler calls
// refresh(SCOPE_ALL) unconditionally.
async function forceRefresh(page) {
  await page.getByTestId('toolbar-fetch').click();
  await page
    .getByTestId('notice')
    .filter({ hasText: /Fetch complete/i })
    .waitFor({ timeout: 15000 });
}

async function main() {
  const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'zync-e2e-features-'));
  let fixture = null;
  let browser = null;

  try {
    // dirty:false - the interactive rebase flows need a clean working tree
    // to begin with; any earlier step that leaves the tree dirty must clean
    // up after itself before the rebase steps run (see step ordering below).
    fixture = await buildFixture(tmpRoot, { dirty: false });
    console.log(
      `Fixture ready: repoId=${fixture.repoId} repoName=${fixture.repoName} workPath=${fixture.workPath}`,
    );

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
      await page
        .getByTestId('commit-row')
        .filter({ hasText: 'Extend app.txt' })
        .first()
        .waitFor({ timeout: 15000 });
    });

    // -----------------------------------------------------------------
    // P1.1 - Tags
    // -----------------------------------------------------------------
    const tagName = `e2e-feature-tag-${Date.now()}`;

    await step('tags: create a tag on main via branch context menu', async () => {
      const branchRow = page.locator('[data-testid="branch-row"][data-branch-name="main"]');
      await branchRow.click({ button: 'right' });
      const menu = page.getByRole('menu');
      await menu.waitFor({ state: 'visible', timeout: 15000 });
      await page.getByRole('menuitem', { name: 'New Tag...', exact: true }).click();
      const dialog = page.getByTestId('tag-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.locator('#tag-name').fill(tagName);
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
    });

    let tagRow;
    await step('tags: tag-row appears in sidebar Tags section', async () => {
      tagRow = page.locator(`[data-testid="tag-row"][data-tag-name="${tagName}"]`);
      await tagRow.waitFor({ timeout: 15000 });
    });

    await step('tags: open tag-context-menu', async () => {
      await tagRow.click({ button: 'right' });
      const menu = page.getByTestId('tag-context-menu');
      await menu.waitFor({ state: 'visible', timeout: 15000 });
      // Close it without acting - the next two steps reopen it themselves.
      await page.keyboard.press('Escape');
      await menu.waitFor({ state: 'hidden', timeout: 10000 });
    });

    await step('tags: Copy SHA sets the footer notice', async () => {
      await tagRow.click({ button: 'right' });
      const menu = page.getByTestId('tag-context-menu');
      await menu.waitFor({ state: 'visible', timeout: 15000 });
      await page.getByRole('menuitem', { name: 'Copy SHA', exact: true }).click();
      await page
        .getByTestId('notice')
        .filter({ hasText: /Copied [0-9a-f]{6,8}/i })
        .waitFor({ timeout: 15000 });
    });

    await step('tags: Delete (confirm) removes the row', async () => {
      await tagRow.click({ button: 'right' });
      const menu = page.getByTestId('tag-context-menu');
      await menu.waitFor({ state: 'visible', timeout: 15000 });
      await page.getByRole('menuitem', { name: 'Delete...', exact: true }).click();
      const dialog = page.getByTestId('delete-tag-dialog');
      await dialog.waitFor({ state: 'visible', timeout: 15000 });
      await dialog.getByTestId('dialog-submit').click();
      await dialog.waitFor({ state: 'hidden', timeout: 10000 });
      await tagRow.waitFor({ state: 'detached', timeout: 15000 });
    });

    // -----------------------------------------------------------------
    // P1.3 - Commit search
    // -----------------------------------------------------------------
    await step('search: All Commits mode', async () => {
      await page.getByTestId('commits-tab').click();
      await page.getByTestId('commit-list').waitFor({ timeout: 10000 });
    });

    await step('search: type an author query, result count updates', async () => {
      const input = page.getByTestId('search-input');
      await input.fill('Zync E2E');
      // All 3 fixture commits share this author.
      await page
        .getByTestId('search-result-count')
        .filter({ hasText: /^3 matches$/ })
        .waitFor({ timeout: 15000 });
    });

    await step('search: non-matching query dims non-matches', async () => {
      const input = page.getByTestId('search-input');
      await input.fill('Extend app.txt');
      await page
        .getByTestId('search-result-count')
        .filter({ hasText: /^1 match$/ })
        .waitFor({ timeout: 15000 });
      const matchingRow = page.getByTestId('commit-row').filter({ hasText: 'Extend app.txt' }).first();
      const otherRow = page.getByTestId('commit-row').filter({ hasText: 'Initial commit' }).first();
      const matchingOpacity = await matchingRow.evaluate((el) => getComputedStyle(el).opacity);
      const otherOpacity = await otherRow.evaluate((el) => getComputedStyle(el).opacity);
      if (matchingOpacity !== '1') {
        throw new Error(`expected the matching row to be fully opaque, got opacity=${matchingOpacity}`);
      }
      if (otherOpacity === '1') {
        throw new Error(`expected the non-matching row to be dimmed, got opacity=${otherOpacity}`);
      }
    });

    await step('search: clear resets to no dimming / no count', async () => {
      await page.getByTestId('search-clear').click();
      await page.getByTestId('search-input').and(page.locator('[value=""]')).waitFor({ timeout: 10000 });
      await page.getByTestId('search-result-count').waitFor({ state: 'hidden', timeout: 10000 });
    });

    await step('search: search-all-history returns results', async () => {
      const input = page.getByTestId('search-input');
      await input.fill('Extend app.txt');
      await page.getByTestId('search-all-history').waitFor({ timeout: 15000 });
      await page.getByTestId('search-all-history').click();
      await page
        .getByText(/History results \(\d+\)/)
        .waitFor({ timeout: 15000 });
      const backButton = page.getByTestId('search-back-to-graph');
      await backButton.waitFor({ timeout: 15000 });
      const historyRow = page.getByTestId('commit-row').filter({ hasText: 'Extend app.txt' }).first();
      await historyRow.waitFor({ timeout: 15000 });
      await backButton.click();
      await page.getByTestId('search-back-to-graph').waitFor({ state: 'hidden', timeout: 10000 });
    });

    await step('search: clear query after all-history round trip', async () => {
      await page.getByTestId('search-clear').click();
      await page.getByTestId('search-input').and(page.locator('[value=""]')).waitFor({ timeout: 10000 });
    });

    // -----------------------------------------------------------------
    // P1.6 - Interactive rebase (run while the tree is still clean, before
    // any of the dirty-tree diff-file-tree/image steps below).
    // -----------------------------------------------------------------
    let targetCommitId = null;
    await step('rebase: locate "Add app.txt" commit id via git', async () => {
      targetCommitId = git(['log', '--format=%H', '--grep=^Add app.txt$'], fixture.workPath).trim();
      if (!targetCommitId) throw new Error('could not resolve "Add app.txt" commit id');
    });

    await step('rebase: open interactive-rebase-dialog from that commit', async () => {
      const row = page.getByTestId('commit-row').filter({ hasText: 'Add app.txt' }).first();
      await row.click({ button: 'right' });
      const menu = page.getByRole('menu');
      await menu.waitFor({ state: 'visible', timeout: 15000 });
      await page.getByRole('menuitem', { name: 'Interactive Rebase...', exact: true }).click();
      await page.getByTestId('interactive-rebase-dialog').waitFor({ state: 'visible', timeout: 15000 });
      const rows = page.getByTestId('rebase-row');
      await rows.first().waitFor({ timeout: 15000 });
      const count = await rows.count();
      if (count !== 2) {
        throw new Error(`expected 2 rebase rows (Add app.txt, Extend app.txt), got ${count}`);
      }
    });

    await step('rebase: drop-then-squash guard disables Execute', async () => {
      const rows = page.getByTestId('rebase-row');
      // Row 0 = oldest ("Add app.txt") -> drop. Row 1 = "Extend app.txt" -> squash.
      await rows.nth(0).getByTestId('rebase-action-select').click();
      await page.getByRole('option', { name: 'Drop', exact: true }).click();
      await rows.nth(1).getByTestId('rebase-action-select').click();
      await page.getByRole('option', { name: 'Squash', exact: true }).click();
      const executeBtn = page.getByTestId('rebase-execute');
      await executeBtn.and(page.locator('[disabled]')).waitFor({ timeout: 15000 });
    });

    await step('rebase: cancel the guarded plan without executing', async () => {
      await page.getByTestId('dialog-cancel').click();
      await page.getByTestId('interactive-rebase-dialog').waitFor({ state: 'hidden', timeout: 10000 });
    });

    let commitCountBefore = null;
    await step('rebase: reopen dialog fresh, squash the newest row into the base', async () => {
      commitCountBefore = parseInt(
        git(['rev-list', '--count', 'HEAD'], fixture.workPath).trim(),
        10,
      );
      const row = page.getByTestId('commit-row').filter({ hasText: 'Add app.txt' }).first();
      await row.click({ button: 'right' });
      const menu = page.getByRole('menu');
      await menu.waitFor({ state: 'visible', timeout: 15000 });
      await page.getByRole('menuitem', { name: 'Interactive Rebase...', exact: true }).click();
      await page.getByTestId('interactive-rebase-dialog').waitFor({ state: 'visible', timeout: 15000 });
      const rows = page.getByTestId('rebase-row');
      await rows.first().waitFor({ timeout: 15000 });
      // Row 0 stays "pick", row 1 ("Extend app.txt") -> squash into it.
      await rows.nth(1).getByTestId('rebase-action-select').click();
      await page.getByRole('option', { name: 'Squash', exact: true }).click();
      const executeBtn = page.getByTestId('rebase-execute');
      await executeBtn.waitFor({ timeout: 10000 });
      const disabled = await executeBtn.getAttribute('disabled');
      if (disabled !== null) throw new Error('expected Execute to be enabled for a plain squash plan');
      await executeBtn.click();
      await page.getByTestId('interactive-rebase-dialog').waitFor({ state: 'hidden', timeout: 15000 });
    });

    await step('rebase: commit count dropped by one, verified via git', async () => {
      // Give the mutating route's broadcast_git_change + UI refresh a moment,
      // then verify against the real repository on disk (not just the UI).
      await page
        .getByTestId('notice')
        .filter({ hasText: /Rebased/i })
        .waitFor({ timeout: 15000 });
      const commitCountAfter = parseInt(
        git(['rev-list', '--count', 'HEAD'], fixture.workPath).trim(),
        10,
      );
      if (commitCountAfter !== commitCountBefore - 1) {
        throw new Error(
          `expected commit count to drop by 1 (${commitCountBefore} -> ${commitCountBefore - 1}), got ${commitCountAfter}`,
        );
      }
    });

    await step('rebase: add a post-rebase commit (file history needs 2+ entries for src/app.txt)', async () => {
      // The squash above collapsed "Add app.txt" + "Extend app.txt" into one
      // commit, so src/app.txt's history is down to a single entry - add a
      // real, committed follow-up change so the P1.2 file-history flow below
      // has more than one row to select between.
      fs.appendFileSync(path.join(fixture.workPath, 'src', 'app.txt'), 'line five (post-rebase)\n');
      git(['add', 'src/app.txt'], fixture.workPath);
      git(['commit', '-m', 'Post-rebase extend app.txt'], fixture.workPath);
    });

    // -----------------------------------------------------------------
    // P1.4 - Diff file tree
    //
    // NOTE: there is currently no UI path to view a *selected commit's*
    // full multi-file diff - CommitGraph never fetches a commit diff, and
    // the "Commit" detail tab (App.tsx's CommitDetail, shown in All Commits
    // mode) renders only metadata (author/SHA/parents/message), never a
    // diff. DiffPanel (the component that owns diff-file-tree) is only ever
    // mounted while the center pane is in "Local Changes" mode, fed
    // ws.diff - which defaults to the whole *workdir* diff
    // (api.diffWorkdir), a multi-file patch when 2+ files are dirty. So
    // "select a multi-file commit -> diff-file-tree" as literally worded is
    // not reachable; this step instead exercises the same DiffFileTree/
    // DiffFileRow component via a multi-file *workdir* diff, which is the
    // only way diff-file-tree is reachable today. See results write-up.
    // -----------------------------------------------------------------
    await step('diff file tree: dirty two tracked files for a multi-file workdir diff', async () => {
      fs.appendFileSync(path.join(fixture.workPath, 'src', 'app.txt'), 'line four (e2e dirty)\n');
      fs.appendFileSync(path.join(fixture.workPath, 'README.md'), '\nE2E dirty edit.\n');
      await page.getByTestId('changes-tab').click();
      await forceRefresh(page);
      await page
        .locator('[data-testid="local-change-row"][data-path="src/app.txt"]')
        .waitFor({ timeout: 15000 });
    });

    await step('diff file tree: diff-file-tree lists both dirty files', async () => {
      await page.getByTestId('diff-file-tree').waitFor({ timeout: 15000 });
      const rows = page.getByTestId('diff-file-row');
      await rows.first().waitFor({ timeout: 15000 });
      const count = await rows.count();
      if (count !== 2) throw new Error(`expected 2 diff-file-row entries, got ${count}`);
    });

    await step('diff file tree: clicking a diff-file-row updates the diff to that file', async () => {
      const readmeRow = page.getByTestId('diff-file-row').filter({ hasText: 'README.md' }).first();
      await readmeRow.click();
      await readmeRow.and(page.locator('[aria-current="true"]')).waitFor({ timeout: 15000 });
      await page.getByTestId('diff-inline').waitFor({ timeout: 15000 });
      // Confirm the diff pane actually switched to README's content (its
      // unique dirty-edit text), not still showing src/app.txt's.
      await page.getByText('E2E dirty edit.').waitFor({ timeout: 15000 });
    });

    await step('diff file tree: discard the dirty edits (return to a clean tree)', async () => {
      git(['checkout', '--', 'src/app.txt', 'README.md'], fixture.workPath);
      await forceRefresh(page);
      await page
        .locator('[data-testid="local-change-row"][data-path="src/app.txt"]')
        .waitFor({ state: 'detached', timeout: 15000 });
    });

    // -----------------------------------------------------------------
    // P1.5 - Image diff
    // -----------------------------------------------------------------
    await step('image diff: commit a baseline PNG, then modify it (uncommitted)', async () => {
      const imgPath = path.join(fixture.workPath, 'logo.png');
      writeFile(imgPath, makePng(40, 60, 220)); // blue baseline
      git(['add', 'logo.png'], fixture.workPath);
      git(['commit', '-m', 'Add logo.png'], fixture.workPath);
      writeFile(imgPath, makePng(220, 40, 40)); // red, uncommitted
      await forceRefresh(page);
      await page
        .locator('[data-testid="local-change-row"][data-path="logo.png"]')
        .waitFor({ timeout: 15000 });
    });

    await step('image diff: selecting logo.png shows before/after image panes', async () => {
      const row = page.locator('[data-testid="local-change-row"][data-path="logo.png"]');
      await row.locator('code').first().click();
      const before = page.getByTestId('diff-image-before');
      const after = page.getByTestId('diff-image-after');
      await before.waitFor({ timeout: 15000 });
      await after.waitFor({ timeout: 15000 });
      const beforeSrc = await before.getAttribute('src');
      const afterSrc = await after.getAttribute('src');
      if (!beforeSrc || !beforeSrc.includes('revision=HEAD')) {
        throw new Error(`expected before src to reference revision=HEAD, got ${beforeSrc}`);
      }
      const afterRevision = new URL(afterSrc, API_BASE).searchParams.get('revision');
      if (afterRevision !== ':workdir') {
        throw new Error(`expected after src's revision param to be :workdir, got ${afterRevision} (full src: ${afterSrc})`);
      }
      if (beforeSrc === afterSrc) {
        throw new Error('before/after image src should differ (different revisions)');
      }
    });

    await step('image diff: both image responses actually load in the browser', async () => {
      const before = page.getByTestId('diff-image-before');
      const after = page.getByTestId('diff-image-after');
      const naturalWidths = await Promise.all([
        before.evaluate((img) => img.naturalWidth),
        after.evaluate((img) => img.naturalWidth),
      ]);
      if (naturalWidths.some((w) => !w)) {
        throw new Error(`expected both <img> elements to decode with nonzero width, got ${JSON.stringify(naturalWidths)}`);
      }
    });

    await step('image diff (API-level): blob route serves nosniff + image/png', async () => {
      const url = `${API_BASE}/repositories/${fixture.repoId}/git/blob?path=logo.png&revision=HEAD`;
      const response = await fetch(url);
      if (!response.ok) throw new Error(`GET ${url} -> ${response.status}`);
      const contentType = response.headers.get('content-type');
      const nosniff = response.headers.get('x-content-type-options');
      if (contentType !== 'image/png') throw new Error(`expected content-type image/png, got ${contentType}`);
      if (nosniff !== 'nosniff') throw new Error(`expected X-Content-Type-Options: nosniff, got ${nosniff}`);
      const body = Buffer.from(await response.arrayBuffer());
      if (body.length === 0 || body[0] !== 0x89 || body[1] !== 0x50) {
        throw new Error('blob route did not return PNG bytes');
      }
    });

    await step('image diff: discard the dirty logo.png edit (return to a clean tree)', async () => {
      git(['checkout', '--', 'logo.png'], fixture.workPath);
      await forceRefresh(page);
      await page
        .locator('[data-testid="local-change-row"][data-path="logo.png"]')
        .waitFor({ state: 'detached', timeout: 15000 });
    });

    // -----------------------------------------------------------------
    // P1.2 - File history + blame
    // -----------------------------------------------------------------
    let fileHistoryRowCount = 0;
    await step('file history: open-file-history on src/app.txt from the diff panel', async () => {
      // Dirty it so it's selectable in Local Changes, select it (so the
      // DiffPanel header shows it), then open History via the DiffPanel's
      // own open-file-history button (P1.4's entry point into P1.2).
      fs.appendFileSync(path.join(fixture.workPath, 'src', 'app.txt'), 'line for history e2e\n');
      await forceRefresh(page);
      const row = page.locator('[data-testid="local-change-row"][data-path="src/app.txt"]');
      await row.waitFor({ timeout: 15000 });
      await row.locator('code').first().click();
      await page.getByTestId('diff-inline').waitFor({ timeout: 15000 });
      await page.getByTestId('open-file-history').click();
      await page.getByTestId('file-history-view').waitFor({ state: 'visible', timeout: 15000 });
    });

    await step('file history: file-history-view lists file-history-row entries', async () => {
      const rows = page.getByTestId('file-history-row');
      await rows.first().waitFor({ timeout: 15000 });
      fileHistoryRowCount = await rows.count();
      if (fileHistoryRowCount < 2) {
        throw new Error(`expected at least 2 commits touching src/app.txt, got ${fileHistoryRowCount}`);
      }
    });

    await step('file history: selecting a row shows its diff', async () => {
      const sheet = page.getByTestId('file-history-view');
      const headerCode = sheet.locator('header code');
      const initialText = (await headerCode.innerText()).trim();
      const rows = page.getByTestId('file-history-row');
      // First row is already selected by default (most recent) - pick the
      // next one and confirm the header (`path @ <sha>`) - and therefore the
      // diff it drives - actually changed to the newly-selected commit.
      await rows.nth(1).click();
      await rows.nth(1).and(page.locator('[aria-current="true"]')).waitFor({ timeout: 15000 });
      await headerCode.filter({ hasNotText: initialText }).waitFor({ timeout: 15000 });
    });

    await step('file history: close the sheet', async () => {
      await page.keyboard.press('Escape');
      await page.getByTestId('file-history-view').waitFor({ state: 'hidden', timeout: 10000 });
    });

    await step('file history: discard the dirty edit (return to a clean tree)', async () => {
      git(['checkout', '--', 'src/app.txt'], fixture.workPath);
      await forceRefresh(page);
      await page
        .locator('[data-testid="local-change-row"][data-path="src/app.txt"]')
        .waitFor({ state: 'detached', timeout: 15000 });
    });

    let blameTargetSha = null;
    await step('blame: select src/app.txt, switch DiffPanel to Blame', async () => {
      // Dirty it again (trivially) so it's selectable in Local Changes -
      // blame is keyed off ws.selectedFile regardless of dirty content.
      fs.appendFileSync(path.join(fixture.workPath, 'src', 'app.txt'), 'line for blame e2e\n');
      await forceRefresh(page);
      const row = page.locator('[data-testid="local-change-row"][data-path="src/app.txt"]');
      await row.waitFor({ timeout: 15000 });
      await row.locator('code').first().click();
      await page.getByTestId('diff-inline').waitFor({ timeout: 15000 });
      await page.getByTestId('diff-blame').click();
      await page
        .getByTestId('diff-blame')
        .and(page.locator('[aria-pressed="true"]'))
        .waitFor({ timeout: 15000 });
    });

    await step('blame: blame-commit-link is present', async () => {
      const link = page.getByTestId('blame-commit-link').first();
      await link.waitFor({ timeout: 15000 });
      blameTargetSha = await link.innerText();
    });

    await step('blame: clicking blame-commit-link selects that commit', async () => {
      const link = page.getByTestId('blame-commit-link').first();
      await link.click();
      // Selecting a commit from blame doesn't switch center-pane mode by
      // itself; switch to All Commits and confirm the Commit detail tab now
      // shows that same short SHA (the SHA <code> is the sibling right after
      // the "SHA" section label in CommitDetail).
      await page.getByTestId('commits-tab').click();
      await page.getByTestId('detail-tab-commit').click();
      const shaCode = page
        .getByText('SHA', { exact: true })
        .locator('xpath=following-sibling::code[1]');
      await shaCode.waitFor({ timeout: 15000 });
      const shaText = (await shaCode.innerText()).trim();
      if (!shaText.toLowerCase().startsWith(blameTargetSha.toLowerCase())) {
        throw new Error(`expected the Commit tab's SHA (${shaText}) to start with the blame link's short SHA (${blameTargetSha})`);
      }
    });

    await step('blame: discard the dirty edit (return to a clean tree)', async () => {
      git(['checkout', '--', 'src/app.txt'], fixture.workPath);
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
