// Port-check harness for the Rust pure logic added in L003/L004.
//
// The machine that produced this change has no Rust toolchain, so `cargo test` cannot run.
// This script mirrors the Rust functions line by line (whisper segment parsing + argv,
// ffprobe duration parsing, chunk-cut argv, log formatting/redaction/limits, the
// size-rotating writer) and executes them against adversarial inputs on a real filesystem,
// so the algorithms can at least be falsified. Development aid, not part of CI.
//
// Mirrored files (keep in sync when the Rust changes):
//   src-tauri/src/runners/whisper_runner.rs
//   src-tauri/src/runners/ffmpeg_runner.rs
//   src-tauri/src/logging/file_log.rs
//
// Run: node scripts/port-checks.mjs
//
// It verifies semantics only — not Rust compilation, types, tokio cancellation or process
// spawning. Those still require `cargo test` on a machine with a toolchain.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

let failures = 0;
let checks = 0;

function check(name, fn) {
  checks += 1;
  try {
    fn();
    console.log(`PASS  ${name}`);
  } catch (error) {
    failures += 1;
    console.log(`FAIL  ${name}\n      ${error.message}`);
  }
}

function eq(actual, expected, label) {
  const a = JSON.stringify(actual);
  const e = JSON.stringify(expected);
  if (a !== e) throw new Error(`${label || 'value'}: got ${a}, want ${e}`);
}

function ok(value, label) {
  if (!value) throw new Error(`${label || 'value'} is falsy`);
}

// ---------------------------------------------------------------------------
// Rust f64::from_str semantics (accepts inf/nan/exponent, rejects junk)
// ---------------------------------------------------------------------------

function parseRustF64(text) {
  const t = text.trim();
  if (t === '') return null;
  const lower = t.toLowerCase();
  if (['inf', '+inf', 'infinity', '+infinity'].includes(lower)) return Infinity;
  if (['-inf', '-infinity'].includes(lower)) return -Infinity;
  if (['nan', '+nan', '-nan'].includes(lower)) return NaN;
  if (!/^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/.test(t)) return null;
  const n = Number(t);
  return Number.isNaN(n) ? null : n;
}

function rustF64Max(a, b) {
  if (Number.isNaN(a)) return b;
  if (Number.isNaN(b)) return a;
  return Math.max(a, b);
}

// ---------------------------------------------------------------------------
// whisper_runner port
// ---------------------------------------------------------------------------

function parseClockComponent(value) {
  const seconds = parseRustF64(value);
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return null;
  return seconds;
}

function parseTimestamp(value) {
  const parts = value.split(':');
  if (parts.length === 3) {
    const hours = parseClockComponent(parts[0]);
    const minutes = parseClockComponent(parts[1]);
    const seconds = parseClockComponent(parts[2]);
    if (hours === null || minutes === null || seconds === null) return null;
    return hours * 3600 + minutes * 60 + seconds;
  }
  if (parts.length === 2) {
    const minutes = parseClockComponent(parts[0]);
    const seconds = parseClockComponent(parts[1]);
    if (minutes === null || seconds === null) return null;
    return minutes * 60 + seconds;
  }
  return null;
}

function parseSegmentLine(line) {
  const stripped = line.replace(/^\s+/, '');
  if (!stripped.startsWith('[')) return null;
  const rest = stripped.slice(1);
  const bracket = rest.indexOf(']');
  if (bracket < 0) return null;
  const timestamps = rest.slice(0, bracket);
  const text = rest.slice(bracket + 1);
  const arrow = timestamps.indexOf('-->');
  if (arrow < 0) return null;
  const start = parseTimestamp(timestamps.slice(0, arrow).trim());
  const end = parseTimestamp(timestamps.slice(arrow + 3).trim());
  if (start === null || end === null || end < start) return null;
  return { start, end, text: text.trim() };
}

function whisperArgs(settings, chunk, outputDir) {
  const fp16 = settings.fp16 && settings.device === 'cuda';
  const args = [
    '--model', settings.model,
    '--device', settings.device,
    '--task', 'transcribe',
    '--output_format', 'json',
    '--verbose', 'True',
    '--condition_on_previous_text', settings.conditionOnPreviousText ? 'True' : 'False',
    '--fp16', fp16 ? 'True' : 'False',
  ];
  if (settings.language === 'en') args.push('--language', 'en');
  args.push('--output_dir', outputDir, chunk);
  return args;
}

function isOutOfMemory(stderr) {
  const lower = stderr.toLowerCase();
  return lower.includes('cuda out of memory') || lower.includes('torch.cuda.outofmemoryerror');
}

function chunkJsonPath(chunk, outputDir) {
  // Rust Path::file_stem: last dot that is not the first character.
  const base = chunk.split(/[\\/]/).pop();
  const lastDot = base.lastIndexOf('.');
  const stem = lastDot > 0 ? base.slice(0, lastDot) : base;
  return path.join(outputDir, `${stem}.json`);
}

// ---------------------------------------------------------------------------
// ffmpeg_runner port
// ---------------------------------------------------------------------------

function parseFfprobeDuration(stdout) {
  let value;
  try {
    value = JSON.parse(stdout.trim());
  } catch {
    return null;
  }
  const raw = value?.format?.duration;
  let seconds = null;
  if (typeof raw === 'string') seconds = parseRustF64(raw);
  else if (typeof raw === 'number') seconds = Number.isFinite(raw) ? raw : raw;
  if (seconds === null || seconds === undefined) return null;
  if (!Number.isFinite(seconds) || seconds <= 0) return null;
  return seconds;
}

function formatSeconds(value) {
  return value.toFixed(3);
}

function chunkCutArgs(start, end, input, output) {
  const duration = rustF64Max(end - start, 0);
  return [
    '-hide_banner', '-nostdin', '-y',
    '-ss', formatSeconds(start),
    '-i', input,
    '-t', formatSeconds(duration),
    '-c', 'copy', output,
  ];
}

// ---------------------------------------------------------------------------
// logging port (file_log.rs)
// ---------------------------------------------------------------------------

const SENSITIVE_FIELD_NAMES = [
  'apikey', 'api_key', 'authorization', 'bearer', 'bearertoken', 'bearer_token',
  'cookie', 'cookies', 'headers', 'password', 'secret', 'token',
  'videopassword', 'video_password',
];

const TITLE_MAX_CHARS = 80;
const RESPONSE_MAX_CHARS = 500;
const STDERR_MAX_CHARS = 2048;
const FIELD_MAX_CHARS = 8192;

function sanitizeField(value) {
  let out = '';
  for (const ch of value) {
    if (ch === '\n' || ch === '\r' || ch === '\t' || ch === '\u2028' || ch === '\u2029') out += ' ';
    else if (!/\p{Cc}/u.test(ch)) out += ch;
  }
  return out;
}

function truncateChars(value, maxChars) {
  const chars = [...value];
  if (chars.length <= maxChars) return value;
  return chars.slice(0, Math.max(maxChars - 1, 0)).join('') + '…';
}

function isSensitiveField(name) {
  return SENSITIVE_FIELD_NAMES.includes(name.toLowerCase());
}

function fieldLimit(name) {
  if (name === 'title') return TITLE_MAX_CHARS;
  if (name === 'tail') return STDERR_MAX_CHARS;
  if (name === 'reason' || name === 'body' || name === 'response') return RESPONSE_MAX_CHARS;
  return FIELD_MAX_CHARS;
}

function formatLine({ level, fields, name }) {
  const visitor = { event: null, message: null, run: null, group: null, stage: null, fields: [] };
  for (const [fieldName, raw] of fields) {
    const value = isSensitiveField(fieldName)
      ? '[redacted]'
      : truncateChars(sanitizeField(raw), fieldLimit(fieldName));
    if (fieldName === 'event') visitor.event = value;
    else if (fieldName === 'message') visitor.message = value;
    else if (fieldName === 'run') visitor.run = value;
    else if (fieldName === 'group') visitor.group = value;
    else if (fieldName === 'stage') visitor.stage = value;
    else visitor.fields.push([fieldName, value]);
  }

  let out = `TS | ${level} | `;
  out += visitor.event !== null ? visitor.event : visitor.message !== null ? visitor.message : name;
  out += ' |';
  for (const [key, value] of [['run', visitor.run], ['group', visitor.group], ['stage', visitor.stage]]) {
    if (value !== null) out += ` ${key}=${value}`;
  }
  for (const [key, value] of visitor.fields) out += ` ${key}=${value}`;
  if (visitor.event !== null && visitor.message !== null && visitor.message !== '') {
    out += ` msg=${visitor.message}`;
  }
  return out + '\n';
}

// ---------------------------------------------------------------------------
// SizeRotatingFile port (real filesystem)
// ---------------------------------------------------------------------------

class RotatingWriter {
  constructor(dir, fileName, maxBytes, maxFiles) {
    this.dir = dir;
    this.fileName = fileName;
    this.maxBytes = maxBytes;
    this.maxFiles = Math.max(maxFiles, 1);
    this.fd = null;
    this.size = 0;
    this.buffer = Buffer.alloc(0);
  }

  open() {
    fs.mkdirSync(this.dir, { recursive: true });
    const target = path.join(this.dir, this.fileName);
    this.fd = fs.openSync(target, 'a');
    this.size = fs.fstatSync(this.fd).size;
  }

  rotate() {
    if (this.fd !== null) {
      fs.closeSync(this.fd);
      this.fd = null;
    }
    if (this.maxFiles <= 1) {
      const base = path.join(this.dir, this.fileName);
      if (fs.existsSync(base)) fs.rmSync(base);
      this.size = 0;
      this.open();
      return;
    }
    for (let index = this.maxFiles - 2; index >= 1; index -= 1) {
      this.move(this.rotated(index), this.rotated(index + 1));
    }
    this.move(path.join(this.dir, this.fileName), this.rotated(1));
    this.size = 0;
    this.open();
  }

  rotated(index) {
    return path.join(this.dir, `${this.fileName}.${index}`);
  }

  move(from, to) {
    if (!fs.existsSync(from)) return;
    if (fs.existsSync(to)) fs.rmSync(to);
    fs.renameSync(from, to);
  }

  /** Mirrors Write::write + commit-on-newline. */
  write(bytes) {
    this.buffer = Buffer.concat([this.buffer, Buffer.from(bytes)]);
    if (this.buffer.includes(0x0a)) this.commit();
  }

  commit() {
    if (this.buffer.length === 0) return;
    const bytes = this.buffer;
    this.buffer = Buffer.alloc(0);
    this.writeLine(bytes);
  }

  writeLine(bytes) {
    if (this.fd === null) this.open();
    if (this.size > 0 && this.size + bytes.length > this.maxBytes) this.rotate();
    fs.writeSync(this.fd, bytes);
    this.size += bytes.length;
  }

  close() {
    this.commit();
    if (this.fd !== null) fs.closeSync(this.fd);
  }
}

function tempDir(prefix) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `${prefix}-`));
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

check('segment: parses HH:MM:SS.mmm line with double space', () => {
  eq(parseSegmentLine('[00:01:02.500 --> 00:01:05.000]  Hello world'),
    { start: 62.5, end: 65, text: 'Hello world' });
});

check('segment: parses MM:SS.mmm line', () => {
  eq(parseSegmentLine('[01:02.500 --> 01:05.000] text'),
    { start: 62.5, end: 65, text: 'text' });
});

check('segment: keeps ] inside text', () => {
  eq(parseSegmentLine('[00:00.000 --> 00:01.000]  a]b').text, 'a]b');
});

check('segment: rejects tqdm / language / malformed lines', () => {
  ok(parseSegmentLine(' 50%|#####     | 3/10 [00:05<00:10, 1.2it/s]') === null, 'tqdm');
  ok(parseSegmentLine('Detected language: English') === null, 'language');
  ok(parseSegmentLine('[not a timestamp] text') === null, 'junk');
  ok(parseSegmentLine('[00:00.000 --> 00:01.000') === null, 'missing bracket');
  ok(parseSegmentLine('') === null, 'empty');
});

check('segment: ADVERSARIAL inf/nan/negative timestamps must be rejected', () => {
  ok(parseSegmentLine('[inf:00 --> 00:01] x') === null, 'inf hours accepted');
  ok(parseSegmentLine('[NaN:00 --> 00:01] x') === null, 'NaN minutes accepted');
  ok(parseSegmentLine('[-1:00 --> 00:01] x') === null, 'negative accepted');
  ok(parseSegmentLine('[00:01.000 --> 00:00.000] x') === null, 'reversed segment accepted');
});

check('whisper args: default matches AC-03 snapshot', () => {
  eq(whisperArgs(
    { model: 'small', device: 'cuda', fp16: true, language: 'en', conditionOnPreviousText: false },
    '/out/.work/audio.000.m4a',
    '/out/.work/chunks',
  ), [
    '--model', 'small', '--device', 'cuda', '--task', 'transcribe', '--output_format', 'json',
    '--verbose', 'True', '--condition_on_previous_text', 'False', '--fp16', 'True',
    '--language', 'en', '--output_dir', '/out/.work/chunks', '/out/.work/audio.000.m4a',
  ]);
});

check('whisper args: cpu disables fp16, auto omits language, medium/large-v3 map', () => {
  const cpu = whisperArgs(
    { model: 'medium', device: 'cpu', fp16: true, language: 'auto', conditionOnPreviousText: false },
    'c.m4a', 'out',
  );
  ok(!cpu.includes('--language'), 'language flag present for auto');
  eq(cpu[cpu.indexOf('--fp16') + 1], 'False', 'cpu fp16');
  eq(cpu[cpu.indexOf('--model') + 1], 'medium', 'model');
  const large = whisperArgs(
    { model: 'large-v3', device: 'cuda', fp16: true, language: 'en', conditionOnPreviousText: true },
    'c.m4a', 'out',
  );
  eq(large[large.indexOf('--model') + 1], 'large-v3', 'large model');
  eq(large[large.indexOf('--condition_on_previous_text') + 1], 'True', 'condition');
});

check('whisper OOM detection + classification', () => {
  ok(isOutOfMemory('RuntimeError: CUDA out of memory. Tried to allocate 1.20 GiB'), 'oom');
  ok(isOutOfMemory('torch.cuda.OutOfMemoryError: CUDA out of memory.'), 'torch oom');
  ok(!isOutOfMemory('RuntimeError: unexpected EOF'), 'false positive');
});

check('chunk json path mirrors Rust file_stem', () => {
  eq(chunkJsonPath('/out/.work/audio.000.m4a', '/out/.work/chunks'), path.join('/out/.work/chunks', 'audio.000.json'));
  eq(chunkJsonPath('audio', 'out'), path.join('out', 'audio.json'));
  eq(chunkJsonPath('.hidden', 'out'), path.join('out', '.hidden.json'));
  eq(chunkJsonPath('a.b.c', 'out'), path.join('out', 'a.b.json'));
});

check('ffprobe duration: string / number / whitespace', () => {
  eq(parseFfprobeDuration('{"format":{"duration":"1200.500000"}}'), 1200.5);
  eq(parseFfprobeDuration('{"format":{"duration":42}}'), 42);
  eq(parseFfprobeDuration('{"format":{"duration":" 12.5 "}}'), 12.5);
  eq(parseFfprobeDuration('{"format":{"duration":"1e3"}}'), 1000);
});

check('ffprobe duration: rejects junk / zero / negative / inf / nan', () => {
  ok(parseFfprobeDuration('') === null, 'empty');
  ok(parseFfprobeDuration('not json') === null, 'junk');
  ok(parseFfprobeDuration('{"format":{"duration":"N/A"}}') === null, 'N/A');
  ok(parseFfprobeDuration('{"format":{"duration":"0"}}') === null, 'zero');
  ok(parseFfprobeDuration('{"format":{"duration":"-5"}}') === null, 'negative');
  ok(parseFfprobeDuration('{"format":{"duration":"inf"}}') === null, 'inf');
  ok(parseFfprobeDuration('{"format":{"duration":"nan"}}') === null, 'nan');
  ok(parseFfprobeDuration('{"streams":[]}') === null, 'missing');
});

check('chunk cut argv: -t is end-start and fixed boundary flags', () => {
  eq(chunkCutArgs(1200, 2400, '/tmp/audio.m4a', '/tmp/chunks/audio.001.m4a'), [
    '-hide_banner', '-nostdin', '-y', '-ss', '1200.000', '-i', '/tmp/audio.m4a',
    '-t', '1200.000', '-c', 'copy', '/tmp/chunks/audio.001.m4a',
  ]);
  const reversed = chunkCutArgs(100, 40, 'i', 'o');
  eq(reversed[reversed.indexOf('-t') + 1], '0.000', 'negative span clamped');
  for (const forbidden of ['silencedetect', '-af', '-filter_complex', '-vf']) {
    ok(!chunkCutArgs(0, 60, 'i', 'o').includes(forbidden), `unexpected ${forbidden}`);
  }
});

check('log: field order run/group/stage is fixed, extras keep order', () => {
  const line = formatLine({
    level: 'INFO',
    name: 'x',
    fields: [
      ['event', 'stage.change'],
      ['group', 'g1'],
      ['chunk', '3'],
      ['run', 'r1'],
      ['stage', 'transcribing'],
    ],
  });
  eq(line, 'TS | INFO | stage.change | run=r1 group=g1 stage=transcribing chunk=3\n');
});

check('log: message-only events use the message slot', () => {
  eq(
    formatLine({ level: 'DEBUG', name: 'x', fields: [['message', 'detected environment: portable'], ['count', '2']] }),
    'TS | DEBUG | detected environment: portable | count=2\n',
  );
});

check('log: event + message appends msg=', () => {
  eq(
    formatLine({ level: 'WARN', name: 'x', fields: [['event', 'log.write_failed'], ['message', 'degraded']] }),
    'TS | WARN | log.write_failed | msg=degraded\n',
  );
});

check('log: sensitive fields are redacted, has_cookies is not', () => {
  const line = formatLine({
    level: 'WARN',
    name: 'x',
    fields: [
      ['event', 'probe.missing'],
      ['apiKey', 'sk-test-DO-NOT-LEAK'],
      ['password', 'hunter2'],
      ['Authorization', 'Bearer abc'],
      ['has_cookies', 'true'],
      ['whisper', 'missing'],
    ],
  });
  ok(line.includes('apiKey=[redacted]'), 'apiKey');
  ok(line.includes('password=[redacted]'), 'password');
  ok(line.includes('Authorization=[redacted]'), 'authorization');
  ok(line.includes('has_cookies=true'), 'has_cookies must stay');
  ok(!line.includes('sk-test-DO-NOT-LEAK') && !line.includes('hunter2'), 'leak');
});

check('log: title/reason/tail limits and multibyte safety', () => {
  const title = '汉'.repeat(200);
  const line = formatLine({ level: 'INFO', name: 'x', fields: [['event', 'transcript.en.ok'], ['title', title]] });
  const value = line.split(' title=')[1].replace(/\n$/, '');
  eq([...value].length, TITLE_MAX_CHARS, 'title length');
  ok(value.endsWith('…'), 'title marker');

  const reason = 'R'.repeat(900);
  const long = formatLine({ level: 'ERROR', name: 'x', fields: [['event', 'translate.block.fail'], ['reason', reason]] });
  const reasonValue = long.split(' reason=')[1].replace(/\n$/, '');
  eq([...reasonValue].length, RESPONSE_MAX_CHARS, 'reason length');

  const tail = formatLine({ level: 'ERROR', name: 'x', fields: [['event', 'whisper.fail'], ['tail', 'T'.repeat(5000)]] });
  eq([...tail.split(' tail=')[1].replace(/\n$/, '')].length, STDERR_MAX_CHARS, 'tail length');
});

check('log: control characters collapse, U+2028/U+2029 too', () => {
  const line = formatLine({ level: 'INFO', name: 'x', fields: [['event', 'e'], ['title', 'a\nb\tc\rd\u0007e']] });
  ok(line.includes('title=a b c de'), `collapse/control drop got ${line}`);
  const u2028 = formatLine({ level: 'INFO', name: 'x', fields: [['event', 'e'], ['title', 'a\u2028b\u2029c']] });
  ok(u2028.includes('title=a b c'), 'U+2028/U+2029 must collapse to spaces');
});

check('rotation: keeps <= maxFiles files, per-file cap, no line loss', () => {
  const dir = tempDir('rotate');
  const writer = new RotatingWriter(dir, 'transcribe.log', 128, 3);
  for (let i = 0; i < 20; i += 1) {
    writer.write(`${'x'.repeat(59)}${String(i).padStart(2, '0')}\n`);
  }
  writer.close();

  const files = fs.readdirSync(dir).sort();
  eq(files.length, 3, 'file count');
  ok(files.includes('transcribe.log'), 'base exists');
  ok(files.includes('transcribe.log.1'), '.1 exists');
  ok(files.includes('transcribe.log.2'), '.2 exists');
  ok(!fs.existsSync(path.join(dir, 'transcribe.log.3')), '.3 must not exist');

  const sizes = files.map((file) => fs.statSync(path.join(dir, file)).size);
  ok(sizes.every((size) => size <= 128), `file size cap violated: ${sizes}`);
  const last = fs.readFileSync(path.join(dir, 'transcribe.log'), 'utf8');
  ok(last.endsWith('19\n'), 'newest file must contain the last line');
});

check('rotation: maxFiles=1 truncates instead of rotating', () => {
  const dir = tempDir('rotate1');
  const writer = new RotatingWriter(dir, 'transcribe.log', 64, 1);
  for (let i = 0; i < 5; i += 1) writer.write(`${'y'.repeat(40)}\n`);
  writer.close();
  eq(fs.readdirSync(dir), ['transcribe.log']);
  ok(fs.statSync(path.join(dir, 'transcribe.log')).size <= 64, 'size cap');
});

check('rotation: single oversized write is kept as-is (documented edge)', () => {
  const dir = tempDir('rotate-big');
  const writer = new RotatingWriter(dir, 'transcribe.log', 32, 3);
  writer.write(`${'z'.repeat(100)}\n`);
  writer.close();
  eq(fs.statSync(path.join(dir, 'transcribe.log')).size, 101);
  // Real lines are bounded by FIELD_MAX_CHARS (8 KB) x few fields << 5 MB, so the
  // production cap cannot be exceeded this way; recorded as a known edge.
});

console.log(`\n${checks - failures}/${checks} checks passed`);
process.exit(failures === 0 ? 0 : 1);
