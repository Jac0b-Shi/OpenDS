import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { FINGERPRINT, GITHUB, GITEE, HttpError, requester, synchronize, verifyAssets } from '../sync-gitee-release.mjs';

const tag = 'v0.2.0';
const verification = { tag, version: '0.2.0', commit: 'a'.repeat(40), tag_object: 'b'.repeat(40),
  fingerprint: FINGERPRINT, prerelease: false };
const hash = data => createHash('sha256').update(data).digest('hex');
const api = `https://gitee.com/api/v5/repos/${GITEE}`;

function fixture() {
  const receipt = { tag, commit: verification.commit, tag_object: verification.tag_object,
    authorized_primary_fingerprint: FINGERPRINT };
  const files = new Map([
    ['OpenDS-0.2.0-windows-x64.zip', Buffer.from('binary')],
    ['OpenDS-0.2.0-source.zip', Buffer.from('source')],
    ['TAG-SIGNATURE.json', Buffer.from(JSON.stringify(receipt))],
  ]);
  files.set('SHA256SUMS.txt', Buffer.from([...files].map(([name, bytes]) => `${hash(bytes)}  ./${name}\n`).join('')));
  let target = null;
  const assets = [];
  const calls = [];
  let failUpload = '';
  let moved = false;
  const json = value => Buffer.from(JSON.stringify(value));
  const request = async (url, options = {}) => {
    calls.push({ url, ...options });
    if (url.includes('/git/ref/tags/')) return json({ object: { type: 'tag', sha: verification.tag_object } });
    if (url.startsWith('https://api.github.com/') && url.includes('/releases/tags/')) {
      return json({ tag_name: tag, name: 'OpenDS 0.2.0', body: 'Published release', draft: false,
        prerelease: false, assets: [...files.keys()].map(name => ({ name })) });
    }
    if (url.startsWith(`https://github.com/${GITHUB}/releases/download/${tag}/`)) return files.get(url.split('/').at(-1));
    if (url === `${api}/releases/tags/${tag}`) {
      if (!target) throw new HttpError(404);
      return json(target);
    }
    if (url === `${api}/releases` && options.body) {
      assert.ok(options.body instanceof URLSearchParams);
      assert.equal(options.body.get('target_commitish'), verification.commit);
      target = { id: 1, tag_name: tag, prerelease: false };
      return json(target);
    }
    if (url === `${api}/releases/1/attach_files?per_page=100`) return json(assets.map(({ name, id }) => ({ name, id })));
    if (url === `${api}/releases/1/attach_files` && options.body) {
      const blob = options.body.get('file');
      if (blob.name === failUpload) throw new HttpError(503);
      const asset = { id: assets.length + 1, name: blob.name, bytes: Buffer.from(await blob.arrayBuffer()) };
      assets.push(asset);
      return json({ id: asset.id });
    }
    const download = /\/attach_files\/(\d+)\/download$/.exec(url);
    if (download) return assets.find(a => a.id === Number(download[1])).bytes;
    throw new Error(`Unexpected request: ${url}`);
  };
  return { files, assets, calls, request, setTarget: value => { target = value; },
    failUpload: name => { failUpload = name; }, moveTag: () => { moved = true; },
    run: (overrides = {}) => synchronize({ tag, verification, token: 'fixture-secret', request,
      remoteTag: async () => moved ? 'c'.repeat(40) : verification.tag_object, ...overrides }) };
}

test('publishes exactly four assets and uploads the manifest last', async () => {
  const f = fixture();
  assert.match(await f.run(), /four matching attachments/);
  assert.equal(f.assets.length, 4);
  assert.equal(f.assets.at(-1).name, 'SHA256SUMS.txt');
  assert.equal(f.calls.filter(c => c.body).length, 5);
});

test('rerun verifies existing files without uploading or deleting them', async () => {
  const f = fixture();
  await f.run();
  f.calls.length = 0;
  await f.run();
  assert.equal(f.calls.filter(c => c.body).length, 0);
});

test('Gitee HTTP 200 null for an absent release is handled like HTTP 404', async () => {
  const f = fixture();
  assert.match(await f.run({ request: async (url, options) => {
    if (url === `${api}/releases/tags/${tag}`) return Buffer.from('null');
    return f.request(url, options);
  } }), /four matching attachments/);
});

test('partial upload fails the job and a rerun only fills missing assets', async () => {
  const f = fixture();
  f.failUpload('OpenDS-0.2.0-source.zip');
  await assert.rejects(f.run(), /HTTP 503/);
  assert.equal(f.assets.length, 1);
  f.failUpload('');
  await f.run();
  assert.equal(f.assets.length, 4);
});

test('existing corrupt asset stops before uploading any additional file', async () => {
  const f = fixture();
  await f.run();
  f.assets[0].bytes = Buffer.from('tampered');
  f.assets.pop();
  f.calls.length = 0;
  await assert.rejects(f.run(), /attachment differs/);
  assert.equal(f.calls.filter(c => c.body).length, 0);
});

test('moved mirror tag prevents all writes', async () => {
  const f = fixture();
  f.moveTag();
  await assert.rejects(f.run(), /identities differ/);
  assert.equal(f.calls.filter(c => c.body).length, 0);
});

test('invalid owner signature receipt and corrupted archives are rejected', () => {
  const f = fixture();
  f.files.set('OpenDS-0.2.0-source.zip', Buffer.from('tampered'));
  assert.throws(() => verifyAssets(tag, verification, f.files), /Checksum mismatch/);
  assert.throws(() => verifyAssets(tag, { ...verification, fingerprint: 'wrong' }, f.files), /signature verification/);
});

test('unexpected manifest paths and incomplete manifests are rejected', () => {
  const f = fixture();
  f.files.set('SHA256SUMS.txt', Buffer.from(`${'0'.repeat(64)}  ../../outside\n`));
  assert.throws(() => verifyAssets(tag, verification, f.files), /manifest entry/);
});

test('a rehashed receipt cannot change the signed tag or authorized owner', () => {
  const f = fixture();
  const receipt = JSON.parse(f.files.get('TAG-SIGNATURE.json'));
  receipt.authorized_primary_fingerprint = 'C'.repeat(40);
  f.files.set('TAG-SIGNATURE.json', Buffer.from(JSON.stringify(receipt)));
  f.files.set('SHA256SUMS.txt', Buffer.from([...f.files].filter(([name]) => name !== 'SHA256SUMS.txt')
    .map(([name, bytes]) => `${hash(bytes)}  ${name}\n`).join('')));
  assert.throws(() => verifyAssets(tag, verification, f.files), /receipt differs/);
});

test('missing credential fails before any network call', async () => {
  const f = fixture();
  await assert.rejects(f.run({ token: '' }), /GITEE_TOKEN/);
  assert.equal(f.calls.length, 0);
});

test('HTTPS and credential handling never forward authenticated requests', async () => {
  const calls = [];
  const request = requester({ githubToken: 'github-fixture', fetchImpl: async (url, options) => {
    calls.push({ url, options });
    return new Response('ok');
  } });
  await request(`${api}/releases`, { body: new URLSearchParams({ access_token: 'gitee-fixture' }) });
  assert.equal(calls[0].options.redirect, 'error');
  assert.equal(calls[0].options.headers.Authorization, undefined);
  await request(`https://api.github.com/repos/${GITHUB}/releases/tags/${tag}`);
  assert.equal(calls[1].options.headers.Authorization, 'Bearer github-fixture');
  assert.equal(calls[1].options.redirect, 'error');
  await assert.rejects(request('https://example.com', { body: new URLSearchParams() }), /Credentials may only/);
});

test('network errors do not print credentials or signed redirect URLs', async () => {
  const request = requester({ fetchImpl: async () => { throw new Error('secret-token signed-url'); } });
  await assert.rejects(request(`${api}/releases`), error => {
    assert.doesNotMatch(error.message, /secret-token|signed-url/);
    return /network request failed/.test(error.message);
  });
});

test('actual published v0.2.0 fixtures can be checked locally when present', async t => {
  try {
    const names = [...fixture().files.keys()];
    const files = new Map(await Promise.all(names.map(async name => [name,
      await readFile(new URL(`../../target/review/gitee-native-release/${name}`, import.meta.url))])));
    const receipt = JSON.parse(files.get('TAG-SIGNATURE.json'));
    verifyAssets(tag, { ...verification, commit: receipt.commit, tag_object: receipt.tag_object }, files);
  } catch (error) {
    if (error.code === 'ENOENT') return t.skip('Optional local published assets are absent on clean CI runners.');
    throw error;
  }
});
