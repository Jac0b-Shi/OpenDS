// Runs on GitHub after publication. Never builds software or writes Git tags.
import { createHash } from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export const GITHUB = 'Jac0b-Shi/OpenDS';
export const GITEE = 'Jac0b_Shi/OpenDS';
export const FINGERPRINT = 'B801AC543CB5B2BBF9407DB2EDDF1F5FEF954168';
const API = `https://gitee.com/api/v5/repos/${GITEE}`;
const GH_API = `https://api.github.com/repos/${GITHUB}`;
const sha256 = data => createHash('sha256').update(data).digest('hex');
const ensure = (condition, message) => { if (!condition) throw new Error(message); };
export class HttpError extends Error {
  constructor(status) { super(`Release request returned HTTP ${status}.`); this.status = status; }
}
export class NetworkError extends Error {}

export async function curlUpload(url, body, { timeout = 180000, spawnImpl = spawn, log = () => {} } = {}) {
  ensure(url.startsWith(`${API}/`) && body instanceof FormData, 'Invalid Gitee upload destination or payload.');
  const multipart = new Request(url, { method: 'POST', body });
  const bytes = Buffer.from(await multipart.arrayBuffer());
  ensure(bytes.length <= 64 * 1024 * 1024, 'Release upload exceeds 64 MiB.');
  // Feed the multipart payload through stdin. The token is never placed in
  // process arguments, a temporary file, curl output or a redirect request.
  const report = '\nOPENDS_CURL:%{http_code}:%{size_upload}:%{speed_upload}:%{time_namelookup}:%{time_connect}:%{time_appconnect}:%{time_starttransfer}:%{time_total}\n';
  const args = ['--disable', '--silent', '--show-error', '--ipv4', '--http1.1', '--proto', '=https',
    '--connect-timeout', '15', '--max-time', String(timeout / 1000),
    '--speed-limit', '1024', '--speed-time', '45',
    '--request', 'POST', '--header', 'User-Agent: OpenDS-release-sync',
    '--header', `Content-Type: ${multipart.headers.get('content-type')}`,
    '--header', 'Expect:', '--data-binary', '@-', '--write-out', report, url];
  return new Promise((resolve, reject) => {
    let child;
    try { child = spawnImpl('curl', args, { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }); }
    catch { reject(new NetworkError('curl upload could not start.')); return; }
    const chunks = [];
    let size = 0;
    let tooLarge = false;
    const timer = setTimeout(() => child.kill(), timeout + 5000);
    timer.unref();
    child.stdout.on('data', chunk => {
      size += chunk.length;
      if (size > 64 * 1024 * 1024) { tooLarge = true; child.kill(); }
      else chunks.push(chunk);
    });
    child.stderr.resume(); // curl errors can include URLs; never publish them.
    child.stdin.on('error', () => {}); // Early HTTP rejection may close the pipe.
    child.on('error', () => { clearTimeout(timer); reject(new NetworkError('curl upload could not start.')); });
    child.on('close', code => {
      clearTimeout(timer);
      if (tooLarge) { reject(new Error('Release response exceeds 64 MiB.')); return; }
      const result = Buffer.concat(chunks);
      const status = /\nOPENDS_CURL:(\d{3}):(\d+):([\d.]+):([\d.]+):([\d.]+):([\d.]+):([\d.]+):([\d.]+)\n$/.exec(result.toString('utf8'));
      const phase = !status ? 'unknown' : Number(status[6]) === 0 ? 'connection/TLS'
        : Number(status[2]) < bytes.length ? 'upload' : Number(status[7]) === 0 ? 'server response' : 'response download';
      if (status) log(`Gitee upload: phase=${phase}, sent=${status[2]}/${bytes.length} B, speed=${status[3]} B/s, DNS=${status[4]} s, TCP=${status[5]} s, TLS=${status[6]} s, first-byte=${status[7]} s, total=${status[8]} s`);
      if (code !== 0) { reject(new NetworkError(`curl upload failed (exit ${Number.isInteger(code) ? code : 'signal'}, phase=${phase}); no automatic POST retry.`)); return; }
      if (!status) { reject(new NetworkError('curl upload returned no HTTP status.')); return; }
      const value = Number(status[1]);
      if (value < 200 || value >= 300) { reject(new HttpError(value)); return; }
      resolve(result.subarray(0, result.length - status[0].length));
    });
    child.stdin.end(bytes);
  });
}

export function requester({ fetchImpl = fetch, githubToken = '', log = () => {}, uploadImpl,
  pause = ms => new Promise(resolve => setTimeout(resolve, ms)) } = {}) {
  const once = async (url, { body, timeout = 30000 } = {}) => {
    ensure(!body || url.startsWith(`${API}/`), 'Credentials may only be sent to the Gitee repository API.');
    const headers = { 'User-Agent': 'OpenDS-release-sync' };
    if (url.startsWith(`${GH_API}/`) && githubToken) headers.Authorization = `Bearer ${githubToken}`;
    if (body instanceof URLSearchParams) headers['Content-Type'] = 'application/x-www-form-urlencoded';
    // Log only the original public endpoint, never query strings, headers,
    // response bodies or signed redirect destinations.
    const endpoint = new URL(url);
    const operation = `${body ? 'POST' : 'GET'} ${endpoint.hostname}${endpoint.pathname}`;
    log(operation);
    try {
      if (uploadImpl && body instanceof FormData) return await uploadImpl(url, body, { timeout, log });
      const response = await fetchImpl(url, { method: body ? 'POST' : 'GET', body, headers,
        redirect: body || headers.Authorization ? 'error' : 'follow', signal: AbortSignal.timeout(timeout) });
      if (!response.ok) throw new HttpError(response.status);
      const chunks = [];
      let bytes = 0;
      for await (const chunk of response.body) {
        bytes += chunk.length;
        ensure(bytes <= 64 * 1024 * 1024, 'Release response exceeds 64 MiB.');
        chunks.push(chunk);
      }
      return Buffer.concat(chunks);
    } catch (error) {
      if (error instanceof HttpError || error instanceof NetworkError) throw error;
      // Never log response bodies, redirected URLs, request options, or credentials.
      const code = error.name === 'TimeoutError' ? `timeout after ${timeout} ms`
        : /^[A-Z][A-Z0-9_]{1,39}$/.test(error.cause?.code ?? '') ? error.cause.code : 'connection or response failure';
      throw new NetworkError(`Release network request failed: ${operation} (${code}).`);
    }
  };
  return async (url, options = {}) => {
    for (let attempt = 0; ; attempt++) {
      try { return await once(url, options); }
      catch (error) {
        const transient = error instanceof NetworkError || error instanceof HttpError && [429, 502, 503, 504].includes(error.status);
        if (options.body || attempt === 1 || !transient) throw error;
        log('Transient read failure; retrying this GET once after 2 seconds.');
        await pause(2000);
      }
    }
  };
}

export function validateTag(tag, verification) {
  ensure(/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(tag), 'Invalid release tag.');
  ensure(verification.tag === tag && verification.version === tag.slice(1)
    && verification.fingerprint === FINGERPRINT
    && /^[0-9a-f]{40}$/.test(verification.commit)
    && /^[0-9a-f]{40}$/.test(verification.tag_object), 'Missing or inconsistent owner signature verification.');
  return [`OpenDS-${tag.slice(1)}-windows-x64.zip`, `OpenDS-${tag.slice(1)}-source.zip`, 'TAG-SIGNATURE.json', 'SHA256SUMS.txt'];
}

export function verifyAssets(tag, verification, files) {
  const expected = validateTag(tag, verification);
  ensure(files.size === 4 && expected.every(name => files.has(name)), 'Expected exactly four release attachments.');
  const checksums = new Map();
  for (const line of files.get('SHA256SUMS.txt').toString('utf8').trim().split('\n')) {
    const match = /^([0-9a-f]{64})  (?:\.\/)?([^/\\]+)$/.exec(line);
    ensure(match && expected.slice(0, 3).includes(match[2]) && !checksums.has(match[2]), 'Unexpected checksum manifest entry.');
    checksums.set(match[2], match[1]);
  }
  ensure(checksums.size === 3, 'Checksum manifest must cover both archives and the tag receipt.');
  for (const [name, digest] of checksums) ensure(sha256(files.get(name)) === digest, `Checksum mismatch: ${name}`);
  const receipt = JSON.parse(files.get('TAG-SIGNATURE.json'));
  ensure(receipt.tag === tag && receipt.commit === verification.commit
    && receipt.tag_object === verification.tag_object
    && receipt.authorized_primary_fingerprint === FINGERPRINT, 'Release receipt differs from the verified signed tag.');
}

export function mirroredTag(tag) {
  try {
    const output = execFileSync('git', ['-c', 'http.followRedirects=false', 'ls-remote', '--exit-code',
      `https://gitee.com/${GITEE}.git`, `refs/tags/${tag}`], { encoding: 'utf8', timeout: 15000,
      windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, GIT_TERMINAL_PROMPT: '0' } }).trim();
    const lines = output.split('\n');
    ensure(lines.length === 1, 'Unexpected tag references.');
    return lines[0].split(/\s+/)[0];
  } catch {
    throw new Error('Gitee signed tag is unavailable. Wait for repository synchronization, then rerun this job.');
  }
}

export async function synchronize({ tag, verification, token, request = requester(), remoteTag = mirroredTag }) {
  ensure(token, 'Configure the GitHub repository secret GITEE_TOKEN.');
  const names = validateTag(tag, verification);
  const json = async (url, options) => JSON.parse(await request(url, options));
  const checkTags = async () => {
    const ref = await json(`${GH_API}/git/ref/tags/${encodeURIComponent(tag)}`);
    ensure(ref.object?.type === 'tag' && ref.object.sha === verification.tag_object
      && await remoteTag(tag) === verification.tag_object, 'GitHub/Gitee signed tag identities differ.');
  };
  await checkTags();
  const release = await json(`${GH_API}/releases/tags/${encodeURIComponent(tag)}`);
  ensure(release.tag_name === tag && release.draft === false && release.prerelease === verification.prerelease,
    'Only the published, verified GitHub release may be synchronized.');
  ensure(Array.isArray(release.assets) && release.assets.length === 4
    && new Set(release.assets.map(a => a.name)).size === 4
    && release.assets.every(a => names.includes(a.name)), 'Unexpected GitHub attachment set.');
  const files = new Map(await Promise.all(names.map(async name => [name,
    await request(`https://github.com/${GITHUB}/releases/download/${encodeURIComponent(tag)}/${name}`)])));
  verifyAssets(tag, verification, files);
  await checkTags();
  let target;
  try { target = await json(`${API}/releases/tags/${encodeURIComponent(tag)}`); }
  catch (error) {
    if (!(error instanceof HttpError) || error.status !== 404) throw error;
  }
  // Gitee also returns HTTP 200 with JSON null for an absent release.
  if (target == null) {
    target = await json(`${API}/releases`, { body: new URLSearchParams({ access_token: token, tag_name: tag,
      name: release.name || tag, body: release.body || `https://github.com/${GITHUB}/releases/tag/${tag}`,
      target_commitish: verification.commit, prerelease: String(release.prerelease) }) });
  }
  ensure(target.tag_name === tag && (target.prerelease ?? false) === release.prerelease
    && Number.isSafeInteger(target.id) && target.id > 0, 'Existing Gitee release differs; manual review required.');
  const attachments = `${API}/releases/${target.id}/attach_files`;
  const existing = await json(`${attachments}?per_page=100`);
  ensure(Array.isArray(existing) && new Set(existing.map(a => a.name)).size === existing.length
    && existing.every(a => files.has(a.name) && Number.isSafeInteger(a.id) && a.id > 0),
  'Unexpected existing Gitee attachments; no files were replaced.');
  const checkAsset = async asset => {
    const data = await request(`${attachments}/${asset.id}/download`, { timeout: 60000 });
    ensure(sha256(data) === sha256(files.get(asset.name)), `Gitee attachment differs: ${asset.name}`);
  };
  for (const asset of existing) await checkAsset(asset);
  // Manifest last. Reruns verify existing files and upload only missing files.
  for (const name of names) {
    if (existing.some(a => a.name === name)) continue;
    const form = new FormData();
    form.set('access_token', token);
    form.set('file', new Blob([files.get(name)], { type: 'application/octet-stream' }), name);
    let asset;
    try {
      asset = await json(attachments, { body: form, timeout: 180000 });
    } catch (error) {
      if (!(error instanceof NetworkError)) throw error;
      // A lost response does not prove that an upload failed. Reconcile once
      // without issuing another POST; only a matching read-back can recover it.
      const after = await json(`${attachments}?per_page=100`);
      const matches = Array.isArray(after) ? after.filter(a => a.name === name) : [];
      if (matches.length !== 1) throw error;
      asset = matches[0];
    }
    ensure(Number.isSafeInteger(asset.id) && asset.id > 0, 'Gitee upload returned no attachment identity.');
    await checkAsset({ id: asset.id, name });
  }
  const final = await json(`${attachments}?per_page=100`);
  ensure(Array.isArray(final) && final.length === 4 && new Set(final.map(a => a.name)).size === 4
    && final.every(a => files.has(a.name)), 'Gitee attachment set is incomplete.');
  await checkTags();
  return `Gitee release verified: ${tag} (four matching attachments)`;
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try {
    const args = process.argv.slice(2);
    ensure(args.length === 4 && args[0] === '--tag' && args[2] === '--verification', 'Usage: --tag VERSION --verification FILE');
    console.log(await synchronize({ tag: args[1], verification: JSON.parse(await readFile(args[3], 'utf8')),
      token: process.env.GITEE_TOKEN, request: requester({ githubToken: process.env.GH_TOKEN, log: console.log, uploadImpl: curlUpload }) }));
  } catch (error) {
    console.error(error instanceof SyntaxError ? 'Invalid release metadata JSON.' : error.message);
    process.exitCode = 1;
  }
}
