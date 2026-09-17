// Port-check harness for the Rust pure logic added in L003/L004/L005.
//
// The machine that produced this change has no Rust toolchain, so `cargo test` cannot run.
// This script mirrors the Rust functions line by line (whisper segment parsing + argv,
// ffprobe duration parsing, chunk-cut argv, chunk planning, segment merging, mechanical
// transcript paragraphing, log formatting/redaction/limits, the size-rotating writer) and
// executes them against adversarial inputs on a real filesystem, so the algorithms can at
// least be falsified. Development aid, not part of CI.
//
// Mirrored files (keep in sync when the Rust changes):
//   src-tauri/src/runners/whisper_runner.rs
//   src-tauri/src/runners/ffmpeg_runner.rs
//   src-tauri/src/transcribe/chunking.rs
//   src-tauri/src/transcribe/merge.rs
//   src-tauri/src/transcribe/transcript.rs
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
// transcribe::chunking port
// ---------------------------------------------------------------------------

const MAX_DURATION_SECONDS = 30 * 24 * 60 * 60;
const SECONDS_PER_MINUTE = 60;

/** Returns the plan, or null for the Rust `Err(ChunkingError::InvalidDuration)`. */
function planChunks(duration, chunkMinutes) {
  if (!Number.isFinite(duration) || duration <= 0 || duration > MAX_DURATION_SECONDS) return null;
  if (chunkMinutes === 0) return [{ index: 0, start: 0, end: duration }];

  const chunkSeconds = chunkMinutes * SECONDS_PER_MINUTE;
  const count = Math.ceil(duration / chunkSeconds);
  const chunks = [];
  for (let index = 0; index < count; index += 1) {
    const start = index * chunkSeconds;
    const end = Math.min((index + 1) * chunkSeconds, duration);
    chunks.push({ index, start, end });
  }
  return chunks;
}

// ---------------------------------------------------------------------------
// transcribe::merge port
// ---------------------------------------------------------------------------

const BOUNDARY_TAIL_SECONDS = 0.3;
const BOUNDARY_HEAD_SECONDS = 0.02;
const COVERAGE_TOLERANCE_SECONDS = 2.0;

function validateMergeChunks(chunks) {
  let previousOffset = 0;
  for (let chunkIndex = 0; chunkIndex < chunks.length; chunkIndex += 1) {
    const chunk = chunks[chunkIndex];
    if (!Number.isFinite(chunk.offset) || chunk.offset < 0) {
      return { error: 'InvalidOffset', chunkIndex };
    }
    if (!Number.isFinite(chunk.plannedEnd) || chunk.plannedEnd < chunk.offset) {
      return { error: 'InvalidPlannedEnd', chunkIndex };
    }
    if (chunkIndex > 0 && chunk.offset < previousOffset) {
      return { error: 'UnsortedChunks', chunkIndex };
    }
    previousOffset = chunk.offset;

    for (let segmentIndex = 0; segmentIndex < chunk.segments.length; segmentIndex += 1) {
      const segment = chunk.segments[segmentIndex];
      const ordered
        = Number.isFinite(segment.start)
          && Number.isFinite(segment.end)
          && segment.start >= 0
          && segment.end >= segment.start;
      if (!ordered) return { error: 'InvalidSegment', chunkIndex, segmentIndex };
    }
  }
  return null;
}

/** Mirrors merge.rs::merge_segments; returns `{ error, ... }` or `{ outcome }`. */
function mergeSegments(chunks) {
  const invalid = validateMergeChunks(chunks);
  if (invalid !== null) return invalid;

  const segments = [];
  let coveredSeconds = 0;
  const missingGaps = [];
  const partialGaps = [];

  for (let chunkIndex = 0; chunkIndex < chunks.length; chunkIndex += 1) {
    const chunk = chunks[chunkIndex];
    const span = chunk.plannedEnd - chunk.offset;

    let coveredLocal = 0.0; // fold(0.0, f64::max)
    for (const segment of chunk.segments) coveredLocal = Math.max(coveredLocal, segment.end);
    // Rust: covered_local.min(span); validation keeps the value non-negative.
    const covered = Math.min(coveredLocal, span);
    coveredSeconds += covered;

    const gap = span - covered;
    if (chunk.segments.length === 0) {
      missingGaps.push({ chunkIndex, gapSeconds: gap });
    } else if (gap > 0) {
      partialGaps.push({ chunkIndex, gapSeconds: gap });
    }

    for (let segmentIndex = 0; segmentIndex < chunk.segments.length; segmentIndex += 1) {
      const segment = chunk.segments[segmentIndex];
      segments.push({
        id: `${chunkIndex}:${segmentIndex}`,
        start: segment.start + chunk.offset,
        end: segment.end + chunk.offset,
        text: segment.text,
      });
    }
  }

  segments.sort((left, right) => {
    if (left.start !== right.start) return left.start - right.start;
    if (left.end !== right.end) return left.end - right.end;
    return left.id < right.id ? -1 : left.id > right.id ? 1 : 0;
  });

  const coverageGaps = missingGaps.slice();
  const totalGap = coverageGaps
    .concat(partialGaps)
    .reduce((total, gap) => total + gap.gapSeconds, 0);
  if (totalGap > COVERAGE_TOLERANCE_SECONDS) {
    coverageGaps.push(...partialGaps);
    coverageGaps.sort((left, right) => left.chunkIndex - right.chunkIndex);
  }

  const boundaryRisks = [];
  for (let chunkIndex = 0; chunkIndex + 1 < chunks.length; chunkIndex += 1) {
    const current = chunks[chunkIndex];
    const next = chunks[chunkIndex + 1];
    const span = current.plannedEnd - current.offset;

    let lastEnd = null;
    for (const segment of current.segments) {
      lastEnd = lastEnd === null ? segment.end : Math.max(lastEnd, segment.end);
    }
    let firstStart = null;
    for (const segment of next.segments) {
      firstStart = firstStart === null ? segment.start : Math.min(firstStart, segment.start);
    }

    const tailTooClose = lastEnd !== null && span - lastEnd < BOUNDARY_TAIL_SECONDS;
    const headTooEarly = firstStart !== null && firstStart <= BOUNDARY_HEAD_SECONDS;
    if (tailTooClose || headTooEarly) {
      boundaryRisks.push({ chunkIndex, tailTooClose, headTooEarly });
    }
  }

  return { outcome: { segments, coveredSeconds, coverageGaps, boundaryRisks } };
}

// ---------------------------------------------------------------------------
// transcribe::transcript port
// ---------------------------------------------------------------------------

const PARAGRAPH_GAP_SECONDS = 1.2;
const PARAGRAPH_MAX_CHARS = 700;
const PARAGRAPH_SENTENCE_LIMIT = 3;
const SENTENCE_END_CHARS = ['.', '!', '?', '…'];

/** Rust `str::chars().count()`: Unicode scalars, not UTF-16 units. */
function charCount(text) {
  return [...text].length;
}

function endsWithSentencePunctuation(text) {
  if (text.length === 0) return false;
  return SENTENCE_END_CHARS.includes([...text].pop());
}

/** Mirrors transcript.rs::format_english_transcript. */
function formatEnglishTranscript(segments) {
  const paragraphs = [];
  let current = [];
  let currentChars = 0;
  let currentSentenceEnds = 0;
  let previousEnd = null;
  let lastEndsSentence = false;

  for (const segment of segments) {
    const text = segment.text.trim();
    if (text === '') continue;

    const startsParagraph
      = previousEnd !== null
        && (segment.start - previousEnd > PARAGRAPH_GAP_SECONDS
          || currentChars >= PARAGRAPH_MAX_CHARS
          || (currentSentenceEnds >= PARAGRAPH_SENTENCE_LIMIT && lastEndsSentence));

    if (startsParagraph && current.length > 0) {
      paragraphs.push(current.join(' '));
      current = [];
      currentChars = 0;
      currentSentenceEnds = 0;
    }

    if (current.length > 0) currentChars += 1; // the joining space
    currentChars += charCount(text);
    const endsSentence = endsWithSentencePunctuation(text);
    if (endsSentence) currentSentenceEnds += 1;
    lastEndsSentence = endsSentence;
    current.push(text);
    previousEnd = segment.end;
  }

  if (current.length > 0) paragraphs.push(current.join(' '));
  const text = paragraphs.join('\n\n');
  return {
    paragraphs,
    text,
    paragraphCount: paragraphs.length,
    charCount: charCount(text),
  };
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

function assertPlanTiles(chunks, duration, label) {
  ok(chunks !== null && chunks.length > 0, `${label}: plan missing`);
  eq(chunks[0].start, 0, `${label}: first start`);
  eq(chunks[chunks.length - 1].end, duration, `${label}: last end`);
  chunks.forEach((chunk, index) => {
    eq(chunk.index, index, `${label}: index ${index}`);
    ok(chunk.end > chunk.start, `${label}: chunk ${index} must not be empty`);
  });
  for (let index = 0; index + 1 < chunks.length; index += 1) {
    eq(chunks[index].end, chunks[index + 1].start, `${label}: boundary ${index}`);
  }
}

check('plan_chunks: 0 minutes keeps one chunk covering the whole audio', () => {
  eq(planChunks(600, 0), [{ index: 0, start: 0, end: 600 }]);
  const long = planChunks(3601, 0);
  eq(long.length, 1, 'chunk count');
  eq(long[0].end, 3601, 'end');
});

check('plan_chunks: 20 minutes tiles 3601s into four fixed-boundary chunks', () => {
  const chunks = planChunks(3601, 20);
  eq(chunks, [
    { index: 0, start: 0, end: 1200 },
    { index: 1, start: 1200, end: 2400 },
    { index: 2, start: 2400, end: 3600 },
    { index: 3, start: 3600, end: 3601 },
  ]);
  assertPlanTiles(chunks, 3601, 'ac05');
});

check('plan_chunks: exact multiples never add an empty trailing chunk', () => {
  eq(planChunks(1200, 20).length, 1, 'exactly one chunk');
  const chunks = planChunks(2400, 20);
  eq(chunks.length, 2, 'two chunks');
  eq(chunks[1].end, 2400, 'last end');
  assertPlanTiles(chunks, 2400, 'multiple');
});

check('plan_chunks: union covers [0,duration] across a duration/size table', () => {
  for (const duration of [0.5, 59.9, 60, 60.1, 600, 1200, 3601, 86400]) {
    for (const minutes of [0, 1, 20]) {
      const chunks = planChunks(duration, minutes);
      assertPlanTiles(chunks, duration, `${duration}s/${minutes}min`);
      const expected = minutes === 0 ? 1 : Math.ceil(duration / (minutes * 60));
      eq(chunks.length, expected, `${duration}s/${minutes}min count`);
    }
  }
});

check('plan_chunks: rejects zero/negative/NaN/inf/absurd durations', () => {
  const hostile = [0, -1, NaN, Infinity, -Infinity, MAX_DURATION_SECONDS + 1, Number.MAX_VALUE];
  for (const duration of hostile) {
    ok(planChunks(duration, 20) === null, `${duration} accepted with chunking`);
    ok(planChunks(duration, 0) === null, `${duration} accepted without chunking`);
  }
  ok(planChunks(MAX_DURATION_SECONDS, 0) !== null, '30-day boundary rejected');
});

check('plan_chunks: floating boundaries stay tiled and respect ceil', () => {
  const justOver = planChunks(1200.0000000001, 20);
  eq(justOver.length, 2, 'just over one chunk');
  assertPlanTiles(justOver, 1200.0000000001, 'just-over');

  eq(planChunks(1199.9999999999, 20).length, 1, 'just under one chunk');

  const huge = planChunks(3601, 4294967295);
  eq(huge.length, 1, 'u32::MAX minutes still plans one chunk');
  eq(huge[0].end, 3601, 'end');
});

function chunkPort(offset, plannedEnd, segments) {
  return { offset, plannedEnd, segments };
}

function segPort(start, end, text) {
  return { start, end, text };
}

function boundaryFixture(firstEnd, secondStart) {
  return [
    chunkPort(0, 600, [segPort(0, firstEnd, 'tail')]),
    chunkPort(600, 1200, [segPort(secondStart, 599.5, 'head')]),
  ];
}

check('merge: shifts, sorts, ids and preserves every segment (AC-07)', () => {
  const { outcome } = mergeSegments([
    chunkPort(0, 600, [segPort(10, 12, 'one'), segPort(0, 599.5, 'zero')]),
    chunkPort(600, 1200, [segPort(0.5, 599.5, 'two')]),
    chunkPort(1200, 1800, [segPort(3, 4, 'three'), segPort(1, 599.5, 'two-b')]),
  ]);

  eq(outcome.segments.length, 5, 'segment count');
  eq(outcome.segments.map(segment => segment.start), [0, 10, 600.5, 1201, 1203], 'starts');
  eq(outcome.segments.map(segment => segment.id), ['0:1', '0:0', '1:0', '2:1', '2:0'], 'ids');
  eq(
    outcome.segments.map(segment => segment.text),
    ['zero', 'one', 'two', 'two-b', 'three'],
    'texts',
  );
  eq(new Set(outcome.segments.map(segment => segment.id)).size, 5, 'unique ids');
  ok(
    outcome.segments.every((segment, index, all) => index === 0 || all[index - 1].start <= segment.start),
    'starts must be monotonic',
  );
  eq(outcome.coveredSeconds, 1798.5, 'covered');
  eq(outcome.coverageGaps, [], 'no coverage gaps');
  eq(outcome.boundaryRisks, [], 'no boundary risks');
});

check('merge: identical timestamps from different chunks stay separate', () => {
  const { outcome } = mergeSegments([
    chunkPort(0, 600, [segPort(1, 2, 'a')]),
    chunkPort(600, 1200, [segPort(1, 2, 'b')]),
  ]);
  eq(outcome.segments.length, 2, 'count');
  eq(outcome.segments.map(segment => segment.id), ['0:0', '1:0'], 'ids');
  eq(outcome.segments.map(segment => segment.start), [1, 601], 'starts');
});

check('merge: boundary tail risk only below 0.3s (AC-06)', () => {
  const risk = mergeSegments(boundaryFixture(599.9, 1.2)).outcome;
  eq(risk.boundaryRisks.length, 1, 'count');
  eq(risk.boundaryRisks[0], { chunkIndex: 0, tailTooClose: true, headTooEarly: false });
  eq(mergeSegments(boundaryFixture(598, 1.2)).outcome.boundaryRisks, [], 'safe tail');

  const below = mergeSegments([
    chunkPort(0, 0.5, [segPort(0, 0.25, 'tail')]),
    chunkPort(0.5, 1, [segPort(0.1, 0.9, 'head')]),
  ]).outcome;
  eq(below.boundaryRisks.length, 1, '0.25 < 0.3');

  const atThreshold = mergeSegments([
    chunkPort(0, 0.5, [segPort(0, 0.2, 'tail')]),
    chunkPort(0.5, 1, [segPort(0.1, 0.9, 'head')]),
  ]).outcome;
  eq(atThreshold.boundaryRisks, [], 'exactly 0.3 is not a risk');
});

check('merge: boundary head risk at 0.02s or earlier (AC-06)', () => {
  const threshold = mergeSegments(boundaryFixture(598, 0.02)).outcome;
  eq(threshold.boundaryRisks[0], { chunkIndex: 0, tailTooClose: false, headTooEarly: true });
  const zero = mergeSegments(boundaryFixture(598, 0)).outcome;
  ok(zero.boundaryRisks[0].headTooEarly, 'start 0');
  eq(mergeSegments(boundaryFixture(598, 0.021)).outcome.boundaryRisks, [], 'just above');
  eq(mergeSegments(boundaryFixture(598, 1.2)).outcome.boundaryRisks, [], 'normal head');
});

check('merge: a boundary can report both conditions', () => {
  const both = mergeSegments(boundaryFixture(599.9, 0.01)).outcome;
  eq(both.boundaryRisks, [{ chunkIndex: 0, tailTooClose: true, headTooEarly: true }]);
});

check('merge: a missing chunk is always reported and localizes the shortfall', () => {
  const { outcome } = mergeSegments([
    chunkPort(0, 600, [segPort(0, 599.5, 'ok')]),
    chunkPort(600, 1200, []),
  ]);
  // The missing chunk is 600s short, so the total exceeds the 2s tolerance and every
  // chunk that lost audio is listed (the 0.5s tail of chunk 0 included).
  eq(
    outcome.coverageGaps,
    [{ chunkIndex: 0, gapSeconds: 0.5 }, { chunkIndex: 1, gapSeconds: 600 }],
    'gap list',
  );
  eq(outcome.coveredSeconds, 599.5, 'covered');
});

check('merge: small tail losses are tolerated until they add up', () => {
  const tolerated = mergeSegments([
    chunkPort(0, 600, [segPort(0, 599, 'a')]),
    chunkPort(600, 1200, [segPort(0.5, 599.5, 'b')]),
  ]).outcome;
  eq(tolerated.coverageGaps, [], '1.0 + 0.5 is within tolerance');
  eq(tolerated.coveredSeconds, 1198.5, 'covered');

  const accumulated = mergeSegments([
    chunkPort(0, 600, [segPort(0, 598, 'a')]),
    chunkPort(600, 1200, [segPort(0.5, 598.5, 'b')]),
  ]).outcome;
  eq(
    accumulated.coverageGaps,
    [{ chunkIndex: 0, gapSeconds: 2 }, { chunkIndex: 1, gapSeconds: 1.5 }],
    '3.5s total is localized per chunk',
  );
  eq(accumulated.coveredSeconds, 1196.5, 'covered');

  const single = mergeSegments([chunkPort(0, 600, [segPort(0, 597.5, 'a')])]).outcome;
  eq(single.coverageGaps, [{ chunkIndex: 0, gapSeconds: 2.5 }], 'single chunk');
  eq(single.coveredSeconds, 597.5, 'covered');
});

check('merge: rejects hostile offsets, orders and timestamps', () => {
  eq(mergeSegments([chunkPort(-1, 600, [])]).error, 'InvalidOffset');
  eq(mergeSegments([chunkPort(NaN, 600, [])]).error, 'InvalidOffset');
  eq(mergeSegments([chunkPort(600, 599, [])]).error, 'InvalidPlannedEnd');
  eq(mergeSegments([chunkPort(0, Infinity, [])]).error, 'InvalidPlannedEnd');
  eq(mergeSegments([chunkPort(600, 1200, []), chunkPort(0, 600, [])]).error, 'UnsortedChunks');

  const hostileSegments = [
    segPort(NaN, 1, 'nan start'),
    segPort(0, Infinity, 'inf end'),
    segPort(-0.1, 1, 'negative start'),
    segPort(2, 1, 'reversed'),
  ];
  for (const segment of hostileSegments) {
    const result = mergeSegments([chunkPort(0, 600, [segment])]);
    eq(result.error, 'InvalidSegment', JSON.stringify(segment));
  }
});

check('merge: empty input is a valid empty merge', () => {
  const { outcome } = mergeSegments([]);
  eq(outcome.segments, [], 'segments');
  eq(outcome.coveredSeconds, 0, 'covered');
  eq(outcome.coverageGaps, [], 'gaps');
  eq(outcome.boundaryRisks, [], 'risks');
});

check('merge: a segment running past its chunk span is clamped, never double counted', () => {
  const { outcome } = mergeSegments([
    chunkPort(0, 600, [segPort(0, 610, 'overrun')]),
    chunkPort(600, 1200, [segPort(1.2, 599.5, 'next')]),
  ]);
  eq(outcome.coveredSeconds, 600 + 599.5, 'covered is clamped to the plan span');
  eq(outcome.coverageGaps, [], 'tail loss of 0.5s is within tolerance');
  eq(outcome.boundaryRisks.length, 1, 'a negative tail distance is still a risk');
  ok(outcome.boundaryRisks[0].tailTooClose, 'tail flagged');
});

function textSegment(start, end, text) {
  return { id: '', start, end, text };
}

check('transcript: gap breaks only above 1.2s', () => {
  const kept = formatEnglishTranscript([
    textSegment(0, 0, 'First.'),
    textSegment(PARAGRAPH_GAP_SECONDS, 2, 'Second.'),
  ]);
  eq(kept.paragraphs, ['First. Second.'], 'exactly 1.2s stays');

  const split = formatEnglishTranscript([
    textSegment(0, 0, 'First.'),
    textSegment(PARAGRAPH_GAP_SECONDS + 0.001, 2, 'Second.'),
  ]);
  eq(split.paragraphs, ['First.', 'Second.'], '1.201s breaks');
  eq(split.text, 'First.\n\nSecond.', 'blank line between paragraphs');
});

check('transcript: closes at 700 rendered characters, 699 fits', () => {
  const long = 'a'.repeat(350);
  const tail = 'c'.repeat(10);
  const fits = formatEnglishTranscript([
    textSegment(0, 1, long),
    textSegment(1.1, 2, 'b'.repeat(348)),
    textSegment(2.1, 3, tail),
  ]);
  eq(fits.paragraphCount, 1, '699 fits');

  const split = formatEnglishTranscript([
    textSegment(0, 1, long),
    textSegment(1.1, 2, 'b'.repeat(349)),
    textSegment(2.1, 3, tail),
  ]);
  eq(split.paragraphCount, 2, '700 splits');
  eq(split.paragraphs[1], tail, 'tail paragraph');
  eq(split.charCount, 350 + 1 + 349 + 2 + 10, 'char count');
});

check('transcript: sentence rule needs three ends and a sentence-final tail', () => {
  const three = formatEnglishTranscript([
    textSegment(0, 1, 'One.'),
    textSegment(1.1, 2, 'Two.'),
    textSegment(2.1, 3, 'Three.'),
    textSegment(3.1, 4, 'Four.'),
  ]);
  eq(three.paragraphs, ['One. Two. Three.', 'Four.'], 'closes before Four.');

  const two = formatEnglishTranscript([
    textSegment(0, 1, 'One.'),
    textSegment(1.1, 2, 'Two.'),
    textSegment(2.1, 3, 'and more'),
  ]);
  eq(two.paragraphCount, 1, 'only two sentence ends');

  const continuation = formatEnglishTranscript([
    textSegment(0, 1, 'One.'),
    textSegment(1.1, 2, 'Two.'),
    textSegment(2.1, 3, 'Three.'),
    textSegment(3.1, 4, 'and more'),
    textSegment(4.1, 5, 'Four.'),
    textSegment(5.1, 6, 'Five.'),
  ]);
  eq(
    continuation.paragraphs,
    ['One. Two. Three.', 'and more Four. Five.'],
    'the continuation opens the next paragraph',
  );

  const quote = formatEnglishTranscript([
    textSegment(0, 1, 'One.'),
    textSegment(1.1, 2, 'Two.'),
    textSegment(2.1, 3, 'Three."'),
    textSegment(3.1, 4, 'Four.'),
    textSegment(4.1, 5, 'Five.'),
  ]);
  eq(quote.paragraphs, ['One. Two. Three." Four.', 'Five.'], 'quote is not sentence-final');
});

check('transcript: text stays verbatim apart from whitespace', () => {
  const transcript = formatEnglishTranscript([
    textSegment(0, 1, '  Hello   world . '),
    textSegment(1.1, 2, 'Second  part'),
  ]);
  eq(transcript.text, 'Hello   world . Second  part');
});

check('transcript: skips blank segments and measures gaps from the last real one', () => {
  const transcript = formatEnglishTranscript([
    textSegment(0, 1, 'First.'),
    textSegment(2, 2.5, '   '),
    textSegment(2.5, 3, ''),
    textSegment(3, 4, 'Second.'),
  ]);
  eq(transcript.paragraphs, ['First.', 'Second.'], '2.0s gap against the last real segment');
});

check('transcript: unicode scalars count once and text matches paragraphs', () => {
  const transcript = formatEnglishTranscript([
    textSegment(0, 1, '汉'.repeat(PARAGRAPH_MAX_CHARS)),
    textSegment(1.1, 2, 'next'),
  ]);
  eq(transcript.paragraphCount, 2, '700 CJK chars close the paragraph');
  eq(transcript.charCount, PARAGRAPH_MAX_CHARS + 2 + 4, 'scalar count');
  eq(transcript.text, transcript.paragraphs.join('\n\n'), 'invariant');

  const empty = formatEnglishTranscript([]);
  eq(empty.paragraphs, [], 'empty paragraphs');
  eq(empty.text, '', 'empty text');
});

check('transcript: emoji are single scalars and overlaps never break', () => {
  const emoji = formatEnglishTranscript([
    textSegment(0, 1, '😀'.repeat(PARAGRAPH_MAX_CHARS)),
    textSegment(1.1, 2, 'next'),
  ]);
  eq(emoji.paragraphCount, 2, '700 emoji scalars close the paragraph');

  const overlap = formatEnglishTranscript([
    textSegment(0, 5, 'First.'),
    textSegment(2, 6, 'Second.'),
  ]);
  eq(overlap.paragraphCount, 1, 'a negative gap must not break');
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

  const sizes = files.map(file => fs.statSync(path.join(dir, file)).size);
  ok(sizes.every(size => size <= 128), `file size cap violated: ${sizes}`);
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
