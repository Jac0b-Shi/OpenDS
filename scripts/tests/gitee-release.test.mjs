import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { EventEmitter } from 'node:events';
import { PassThrough, Writable } from 'node:stream';
import { FINGERPRINT, GITHUB, GITEE, HttpError, NetworkError, curlUpload, requester, synchronize, uploadTimeout, verifyAssets } from '../sync-gitee-release.mjs';

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

test('upload budgets accommodate measured slow routes but remain bounded by file size', async () => {
  const f = fixture();
  await f.run();
  const uploads = f.calls.filter(c => c.body instanceof FormData);
  assert.equal(uploads.length, 4);
  assert.ok(uploads.every(c => c.timeout === 60000));
  const large = 4 * 1024 * 1024;
  assert.ok(uploadTimeout(large) > large * 1000 / (12 * 1024));
  assert.ok(uploadTimeout(large) > uploadTimeout(1024 * 1024));
  assert.equal(uploadTimeout(64 * 1024 * 1024), 600000);
  assert.throws(() => uploadTimeout(-1), /Invalid upload size/);
  assert.throws(() => uploadTimeout(64 * 1024 * 1024 + 1), /Invalid upload size/);
});

test('lost upload response is recovered by listing and hashing without a duplicate POST', async () => {
  const f = fixture();
  let lost = false;
  await f.run({ request: async (url, options) => {
    const result = await f.request(url, options);
    if (!lost && options?.body instanceof FormData) {
      lost = true;
      throw new NetworkError('response lost');
    }
    return result;
  } });
  assert.equal(f.assets.length, 4);
  assert.equal(f.calls.filter(c => c.body instanceof FormData).length, 4);
});

test('network failure without a stored attachment fails without retrying POST', async () => {
  const f = fixture();
  let uploads = 0;
  await assert.rejects(f.run({ request: async (url, options) => {
    if (options?.body instanceof FormData) {
      uploads++;
      throw new NetworkError('upload lost');
    }
    return f.request(url, options);
  } }), /upload lost/);
  assert.equal(uploads, 1);
  assert.equal(f.assets.length, 0);
});

test('lost upload response cannot recover a corrupt stored attachment', async () => {
  const f = fixture();
  await assert.rejects(f.run({ request: async (url, options) => {
    const result = await f.request(url, options);
    if (options?.body instanceof FormData) {
      f.assets.at(-1).bytes = Buffer.from('corrupt');
      throw new NetworkError('response lost');
    }
    return result;
  } }), /attachment differs/);
  assert.equal(f.assets.length, 1);
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

test('curl upload keeps token in stdin, refuses redirects and parses the HTTP result', async () => {
  const form = new FormData();
  form.set('access_token', 'fixture-private-token');
  form.set('file', new Blob(['fixture-file']), 'source.zip');
  let payload, argumentsUsed;
  const spawnImpl = (program, args) => {
    assert.equal(program, 'curl');
    argumentsUsed = args;
    const child = new EventEmitter();
    child.stdout = new PassThrough();
    child.stderr = new PassThrough();
    child.kill = () => {};
    child.stdin = new Writable({ write(chunk, encoding, callback) { payload = Buffer.from(chunk); callback(); } });
    child.stdin.on('finish', () => {
      child.stdout.write('{"id":42}\nOPENDS_CURL:201:999:1000:0.1:0.2:0.3:0.4:0.5\n');
      child.emit('close', 0);
    });
    return child;
  };
  assert.deepEqual(JSON.parse(await curlUpload(`${api}/releases/1/attach_files`, form, { spawnImpl })), {id:42});
  assert.match(payload.toString(), /fixture-private-token/);
  assert.match(payload.toString(), /fixture-file/);
  assert.doesNotMatch(argumentsUsed.join(' '), /fixture-private-token/);
  assert.ok(!argumentsUsed.some(a => a === '-L' || a === '--location'));
  assert.ok(argumentsUsed.includes('--ipv4'));
  await assert.rejects(curlUpload('https://example.com/upload', form, {spawnImpl}), /Invalid Gitee/);
});

test('transient GET failures retry once but POST and validation failures never replay', async () => {
  let calls = 0, pauses = 0;
  const request = requester({pause:async () => {pauses++;}, fetchImpl:async () => {
    calls++;
    if (calls === 1) throw new Error('connection failed');
    return new Response('ok');
  }});
  assert.equal((await request(`${api}/releases`)).toString(), 'ok');
  assert.equal(calls, 2);
  assert.equal(pauses, 1);
  let posts = 0;
  const failure = requester({pause:async () => {},fetchImpl:async () => {posts++; throw new Error('failed');}});
  await assert.rejects(failure(`${api}/releases`, {body:new URLSearchParams({access_token:'fixture'})}), /network request/);
  assert.equal(posts, 1);
  posts = 0;
  await assert.rejects(failure(`${api}/releases`), /network request/);
  assert.equal(posts, 2);
  let forbidden = 0;
  const denied = requester({pause:async () => {},fetchImpl:async () => {forbidden++; return new Response('',{status:403});}});
  await assert.rejects(denied(`${api}/releases`), /HTTP 403/);
  assert.equal(forbidden, 1);
});

test('curl timeout diagnostics distinguish a completed upload waiting for the server without exposing stderr', async () => {
  const logs = [];
  const form = new FormData();
  form.set('access_token','fixture-secret-token');
  const spawnImpl = () => {
    const child = new EventEmitter();
    child.stdout = new PassThrough(); child.stderr = new PassThrough(); child.kill=()=>{};
    child.stdin = new Writable({write(chunk, encoding, callback) {callback();}});
    child.stdin.on('finish', () => {
      child.stderr.write('fixture-secret-token https://example.com/?signed-url=secret');
      child.stdout.write('\nOPENDS_CURL:000:999999:1000:0.1:0.2:0.3:0:45\n');
      child.emit('close',28);
    });
    return child;
  };
  await assert.rejects(curlUpload(`${api}/releases/1/attach_files`,form,{spawnImpl,log:l=>logs.push(l)}),error=>{
    assert.match(error.message,/phase=server response/);
    assert.doesNotMatch(error.message,/fixture-secret-token|signed-url/);
    return true;
  });
  assert.match(logs.join(' '), /sent=999999/);
  assert.doesNotMatch(logs.join(' '), /fixture-secret-token|signed-url/);
});

test('requester uses upload transport only for Gitee multipart bodies', async () => {
  let uploads = 0;
  const request = requester({ uploadImpl: async () => { uploads++; return Buffer.from('uploaded'); },
    fetchImpl: async () => new Response('read') });
  const form = new FormData();
  form.set('access_token', 'fixture-token');
  assert.equal((await request(`${api}/releases/1/attach_files`, {body:form})).toString(), 'uploaded');
  assert.equal((await request(`${api}/releases/tags/${tag}`)).toString(), 'read');
  await assert.rejects(request('https://example.com/upload', {body:form}), /Credentials may only/);
  assert.equal(uploads, 1);
});

test('request diagnostics identify the original endpoint and safe error code without query secrets', async () => {
  const logs = [];
  const request = requester({ pause:async () => {}, log: line => logs.push(line), fetchImpl: async () => {
    throw Object.assign(new Error('secret-token signed-url'), { cause: { code: 'UND_ERR_CONNECT_TIMEOUT' } });
  } });
  await assert.rejects(request(`${api}/releases?secret-token=signed-url`), error => {
    assert.match(error.message, /GET gitee.com\/api\/v5\/repos\/Jac0b_Shi\/OpenDS\/releases/);
    assert.match(error.message, /UND_ERR_CONNECT_TIMEOUT/);
    assert.doesNotMatch(error.message, /secret-token|signed-url/);
    return true;
  });
  assert.equal(logs.filter(line => line.startsWith('GET ')).length, 2);
  assert.doesNotMatch(logs.join(' '), /secret-token|signed-url|\?/);
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
