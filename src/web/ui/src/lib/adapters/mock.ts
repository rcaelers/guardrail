// Deterministic mock adapter — mirrors the prototype's data.jsx.
// In-memory: mutations persist only for the server's lifetime.
//
// Architecture matches the prototype:
//   1. A handful of "archetype" minidump reports (one per platform × crash kind)
//      defined in `ARCHETYPES`. These are shaped like minidump-stackwalk JSON.
//   2. `ingest()` derives normalized facts (title, exception type, top
//      symbol, platform, symbol coverage) from a raw dump.
//   3. `buildDataset()` fans each archetype out into ~60 crash groups, assigns
//      each to one of the 4 seeded products, and attaches logs/breadcrumbs/notes.
//   4. Session / identity tables (users, products, memberships, symbols) are
//      seeded inline below the dataset so they live alongside the crash DB.

import type {
  GuardrailAdapter, Crash, CrashGroup, CrashGroupSummary, Dump, DumpThread, DumpModule,
  Derived, ListQuery, ListResult, Note, Status,
  User, Product, Membership, MembershipWithUser, MembershipWithProduct,
  Role, Symbol as SymbolRow, SymbolQuery
} from './types';

const VERSIONS = ['2.14.0', '2.13.4', '2.13.3', '2.13.2', '2.12.9', '2.12.7'];
const ASSIGNEES = ['mlin', 'rwang', 'tkowalski', 'ahmed', 'sofia'];

// ------------------------------------------------------------------
// RNG + small utilities
// ------------------------------------------------------------------
function mulberry32(seed: number) {
  return function () {
    let t = (seed += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const rnd = mulberry32(42);
const pick = <T>(a: T[]) => a[Math.floor(rnd() * a.length)];
const randInt = (lo: number, hi: number) => lo + Math.floor(rnd() * (hi - lo + 1));
const hex = (n: number) => {
  let s = '';
  for (let i = 0; i < n; i++) s += '0123456789abcdef'[Math.floor(rnd() * 16)];
  return s;
};
function relTime(daysAgo: number, hoursAgo = 0) {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  d.setHours(d.getHours() - hoursAgo);
  return d.toISOString();
}
function humanSize(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '—';
  const units = ['B', 'KB', 'MB', 'GB'];
  let u = 0, v = bytes;
  while (v >= 1024 && u < units.length - 1) { v /= 1024; u++; }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[u]}`;
}

// ------------------------------------------------------------------
// Archetypes — each produces a platform-typical minidump report.
// ------------------------------------------------------------------

interface Archetype {
  key: string;
  platform: 'windows' | 'macos' | 'linux';
  make(): Dump;
  summary: string;
}

// Shared module tables per platform (trimmed — enough to look real).
const WIN_MODULES: DumpModule[] = [
  { filename: 'crash.exe',      version: '0.0.0.0',          base_addr: '0x00007ff604c60000', end_addr: '0x00007ff60533d000', code_id: '6521619a6dd000',  debug_id: 'EE9E2672A6863B084C4C44205044422E1', debug_file: 'crash.pdb',      loaded_symbols: true,  corrupt_symbols: false, missing_symbols: false },
  { filename: 'ntdll.dll',      version: '10.0.25967.1000',  base_addr: '0x00007ff98d380000', end_addr: '0x00007ff98d782000', code_id: '4092c1b7402000',  debug_id: '4B245FC21BEBADAF3FD11ADC96DCB0F81', debug_file: 'ntdll.pdb',      loaded_symbols: false, corrupt_symbols: false, missing_symbols: true  },
  { filename: 'KERNEL32.DLL',   version: '10.0.25967.1000',  base_addr: '0x00007ff98b210000', end_addr: '0x00007ff98b370000', code_id: '1262bad2160000',  debug_id: '9E1847A64C35879EEBF8E36717AAA2A21', debug_file: 'kernel32.pdb',   loaded_symbols: false, corrupt_symbols: false, missing_symbols: true  },
  { filename: 'KERNELBASE.dll', version: '10.0.25967.1000',  base_addr: '0x00007ff9893d0000', end_addr: '0x00007ff98998a000', code_id: '576b4dd15ba000',  debug_id: 'BC3E05277B8255453110393C72E034411', debug_file: 'kernelbase.pdb', loaded_symbols: false, corrupt_symbols: false, missing_symbols: true  },
  { filename: 'ucrtbase.dll',   version: '10.0.25967.1000',  base_addr: '0x00007ff988be0000', end_addr: '0x00007ff988df9000', code_id: 'a2593e56219000',  debug_id: 'F90E76195B781DBCF6F144A11DC33D811', debug_file: 'ucrtbase.pdb',   loaded_symbols: false, corrupt_symbols: false, missing_symbols: false },
  { filename: 'USER32.dll',     version: '10.0.25967.1000',  base_addr: '0x00007ff98b8e0000', end_addr: '0x00007ff98bb3f000', code_id: '2837953c25f000',  debug_id: '0F412FA5969B84F98BCAEF1501085E2C1', debug_file: 'user32.pdb',     loaded_symbols: false, corrupt_symbols: false, missing_symbols: false }
];

const MAC_MODULES: DumpModule[] = [
  { filename: 'Guardrail',                    version: '2.14.0', base_addr: '0x0000000104a80000', end_addr: '0x0000000106e40000', code_id: 'mac-guardrail-01', debug_id: 'A1B2C3D4E5F6', debug_file: 'Guardrail.dSYM',               loaded_symbols: true,  corrupt_symbols: false, missing_symbols: false },
  { filename: 'libsystem_pthread.dylib',      version: '',       base_addr: '0x00007ff81a100000', end_addr: '0x00007ff81a120000', code_id: 'mac-pthread-01',   debug_id: '00000000000000', debug_file: '',                            loaded_symbols: false, corrupt_symbols: false, missing_symbols: true  },
  { filename: 'libsystem_kernel.dylib',       version: '',       base_addr: '0x00007ff81a200000', end_addr: '0x00007ff81a240000', code_id: 'mac-kernel-01',    debug_id: '00000000000000', debug_file: '',                            loaded_symbols: false, corrupt_symbols: false, missing_symbols: true  },
  { filename: 'libc++.1.dylib',               version: '',       base_addr: '0x00007ff81a300000', end_addr: '0x00007ff81a360000', code_id: 'mac-libcxx-01',    debug_id: '00000000000000', debug_file: '',                            loaded_symbols: false, corrupt_symbols: false, missing_symbols: false },
  { filename: 'CoreFoundation',               version: '',       base_addr: '0x00007ff81b000000', end_addr: '0x00007ff81b400000', code_id: 'mac-cf-01',        debug_id: '00000000000000', debug_file: '',                            loaded_symbols: false, corrupt_symbols: false, missing_symbols: false }
];

const LINUX_MODULES: DumpModule[] = [
  { filename: 'guardrail',        version: '2.14.0', base_addr: '0x0000555500000000', end_addr: '0x00005555022c0000', code_id: 'lnx-guardrail-01', debug_id: 'A1B2C3D4E5F6', debug_file: 'guardrail.debug', loaded_symbols: true,  corrupt_symbols: false, missing_symbols: false },
  { filename: 'libc.so.6',        version: '',       base_addr: '0x00007fb8a0000000', end_addr: '0x00007fb8a01b0000', code_id: 'lnx-libc-01',      debug_id: '00000000000000', debug_file: '',               loaded_symbols: false, corrupt_symbols: false, missing_symbols: true  },
  { filename: 'libpthread.so.0',  version: '',       base_addr: '0x00007fb8a0200000', end_addr: '0x00007fb8a0220000', code_id: 'lnx-pthread-01',   debug_id: '00000000000000', debug_file: '',               loaded_symbols: false, corrupt_symbols: false, missing_symbols: true  },
  { filename: 'libstdc++.so.6',   version: '',       base_addr: '0x00007fb8a0300000', end_addr: '0x00007fb8a0450000', code_id: 'lnx-libstdcxx-01', debug_id: '00000000000000', debug_file: '',               loaded_symbols: false, corrupt_symbols: false, missing_symbols: false },
  { filename: 'ld-linux-x86-64.so.2', version: '',   base_addr: '0x00007fb8a0500000', end_addr: '0x00007fb8a0520000', code_id: 'lnx-ld-01',        debug_id: '00000000000000', debug_file: '',               loaded_symbols: false, corrupt_symbols: false, missing_symbols: true  }
];

function parkedThread(tid: number, modName: string): DumpThread {
  return {
    thread_id: tid, thread_name: null, frame_count: 3, last_error_value: 'ERROR_SUCCESS',
    frames: [0, 1, 2].map((i) => ({
      frame: i, trust: i === 0 ? 'context' : 'scan', module: modName,
      offset: '0x' + hex(12), module_offset: '0x' + hex(10),
      function: null, function_offset: null, file: null, line: null,
      inlines: null, missing_symbols: true, unloaded_modules: null
    }))
  };
}

function makeWindowsDump(opts: {
  exception: string; address: string;
  crashingFrames: Array<{ fn: string; file: string; line: number }>;
}): Dump {
  const topOffset = '0x' + hex(12);
  const crashingThread: DumpThread = {
    thread_id: 7520, thread_name: null, frame_count: opts.crashingFrames.length + 3,
    last_error_value: 'ERROR_SUCCESS',
    frames: [
      ...opts.crashingFrames.map((f, i) => ({
        frame: i, trust: i === 0 ? 'context' : 'cfi' as string,
        module: 'crash.exe', offset: i === 0 ? opts.address : topOffset,
        module_offset: '0x' + hex(10),
        function: f.fn, function_offset: '0x' + hex(4),
        file: `\\\\?\\C:\\mystuff\\src\\workrave-v1_11\\libs\\crash\\test\\${f.file}`,
        line: f.line,
        inlines: null, missing_symbols: false, unloaded_modules: null
      })),
      { frame: opts.crashingFrames.length, trust: 'cfi', module: 'KERNEL32.DLL', offset: '0x' + hex(12), module_offset: '0x' + hex(6), function: null, function_offset: null, file: null, line: null, inlines: null, missing_symbols: true, unloaded_modules: null },
      { frame: opts.crashingFrames.length + 1, trust: 'scan', module: 'ntdll.dll',  offset: '0x' + hex(12), module_offset: '0x' + hex(6), function: null, function_offset: null, file: null, line: null, inlines: null, missing_symbols: true, unloaded_modules: null },
      { frame: opts.crashingFrames.length + 2, trust: 'scan', module: 'KERNELBASE.dll', offset: '0x' + hex(12), module_offset: '0x' + hex(6), function: null, function_offset: null, file: null, line: null, inlines: null, missing_symbols: true, unloaded_modules: null }
    ]
  };
  return {
    pid: 11424, status: 'OK', thread_count: 5, main_module: 0,
    crash_info: { type: opts.exception, address: opts.address, crashing_thread: crashingThread.thread_id },
    system_info: { os: 'Windows NT', os_ver: '10.0.25967', cpu_arch: 'amd64', cpu_info: 'family 21 model 0 stepping 1', cpu_count: 16, cpu_microcode_version: null },
    modules: WIN_MODULES,
    threads: [crashingThread, ...[912, 1292, 2900, 6556].map((tid) => parkedThread(tid, 'ntdll.dll'))],
    unloaded_modules: []
  };
}

function makeMacDump(opts: {
  exception: string; address: string;
  crashingFrames: Array<{ fn: string; file: string; line: number }>;
}): Dump {
  const ct: DumpThread = {
    thread_id: 259, thread_name: 'main', frame_count: opts.crashingFrames.length + 2,
    last_error_value: 'ERROR_SUCCESS',
    frames: [
      ...opts.crashingFrames.map((f, i) => ({
        frame: i, trust: i === 0 ? 'context' : 'cfi' as string,
        module: 'Guardrail', offset: '0x' + hex(12), module_offset: '0x' + hex(8),
        function: f.fn, function_offset: '0x' + hex(4),
        file: `/Users/runner/src/guardrail/${f.file}`, line: f.line,
        inlines: null, missing_symbols: false, unloaded_modules: null
      })),
      { frame: opts.crashingFrames.length,     trust: 'cfi',  module: 'libsystem_pthread.dylib', offset: '0x' + hex(12), module_offset: '0x' + hex(6), function: '_pthread_start', function_offset: '0x' + hex(4), file: null, line: null, inlines: null, missing_symbols: true, unloaded_modules: null },
      { frame: opts.crashingFrames.length + 1, trust: 'scan', module: 'libsystem_pthread.dylib', offset: '0x' + hex(12), module_offset: '0x' + hex(6), function: 'thread_start',   function_offset: '0x' + hex(4), file: null, line: null, inlines: null, missing_symbols: true, unloaded_modules: null }
    ]
  };
  return {
    pid: 54210, status: 'OK', thread_count: 4, main_module: 0,
    crash_info: { type: opts.exception, address: opts.address, crashing_thread: ct.thread_id },
    system_info: { os: 'Mac OS X', os_ver: '14.6.1 23G93', cpu_arch: 'arm64', cpu_info: 'Apple M3 Pro', cpu_count: 12, cpu_microcode_version: null },
    modules: MAC_MODULES,
    threads: [ct, ...[260, 261, 262].map((tid) => parkedThread(tid, 'libsystem_kernel.dylib'))],
    unloaded_modules: []
  };
}

function makeLinuxDump(opts: {
  exception: string; address: string;
  crashingFrames: Array<{ fn: string; file: string; line: number }>;
}): Dump {
  const ct: DumpThread = {
    thread_id: 4812, thread_name: 'guardrail', frame_count: opts.crashingFrames.length + 2,
    last_error_value: 'ERROR_SUCCESS',
    frames: [
      ...opts.crashingFrames.map((f, i) => ({
        frame: i, trust: i === 0 ? 'context' : 'cfi' as string,
        module: 'guardrail', offset: '0x' + hex(12), module_offset: '0x' + hex(8),
        function: f.fn, function_offset: '0x' + hex(4),
        file: `/build/guardrail/${f.file}`, line: f.line,
        inlines: null, missing_symbols: false, unloaded_modules: null
      })),
      { frame: opts.crashingFrames.length,     trust: 'cfi',  module: 'libpthread.so.0', offset: '0x' + hex(12), module_offset: '0x' + hex(6), function: 'start_thread',   function_offset: '0x' + hex(4), file: null, line: null, inlines: null, missing_symbols: true, unloaded_modules: null },
      { frame: opts.crashingFrames.length + 1, trust: 'scan', module: 'libc.so.6',       offset: '0x' + hex(12), module_offset: '0x' + hex(6), function: '__clone3',       function_offset: '0x' + hex(4), file: null, line: null, inlines: null, missing_symbols: true, unloaded_modules: null }
    ]
  };
  return {
    pid: 88213, status: 'OK', thread_count: 4, main_module: 0,
    crash_info: { type: opts.exception, address: opts.address, crashing_thread: ct.thread_id },
    system_info: { os: 'Linux', os_ver: '6.8.0-40-generic', cpu_arch: 'amd64', cpu_info: 'AMD Ryzen 9 7950X', cpu_count: 32, cpu_microcode_version: '0xa601203' },
    modules: LINUX_MODULES,
    threads: [ct, ...[4813, 4814, 4815].map((tid) => parkedThread(tid, 'libc.so.6'))],
    unloaded_modules: []
  };
}

const ARCHETYPES: Archetype[] = [
  {
    key: 'win-nullwrite', platform: 'windows',
    summary: 'Null-pointer write in renderer flush_batch',
    make: () => makeWindowsDump({
      exception: 'EXCEPTION_ACCESS_VIOLATION_WRITE', address: '0x0000000000000000',
      crashingFrames: [
        { fn: 'guardrail::renderer::Pipeline::flush_batch', file: 'renderer\\pipeline.cc', line: 482 },
        { fn: 'guardrail::renderer::Scene::draw',           file: 'renderer\\scene.cc',    line: 118 },
        { fn: 'guardrail::runtime::tick',                   file: 'runtime\\loop.cc',      line: 214 },
        { fn: 'main',                                        file: 'main.cc',               line: 44 }
      ]
    })
  },
  {
    key: 'win-rust-panic', platform: 'windows',
    summary: 'Panic: unwrap() on None in WsClient::handle_frame',
    make: () => makeWindowsDump({
      exception: 'EXCEPTION_BREAKPOINT', address: '0x0000000000000000',
      crashingFrames: [
        { fn: 'guardrail::net::WsClient::handle_frame', file: 'net\\ws.rs', line: 217 },
        { fn: 'core::option::Option::unwrap',           file: 'core\\option.rs', line: 934 },
        { fn: 'tokio::task::raw::poll',                  file: 'tokio-1.38\\src\\task\\raw.rs', line: 201 },
        { fn: 'std::sys::windows::thread::Thread::new', file: 'std\\sys\\windows\\thread.rs', line: 45 }
      ]
    })
  },
  {
    key: 'mac-uaf', platform: 'macos',
    summary: 'Use-after-free in Mixer::process',
    make: () => makeMacDump({
      exception: 'EXC_BAD_ACCESS / KERN_INVALID_ADDRESS', address: '0x00000000deadbeef',
      crashingFrames: [
        { fn: 'guardrail::audio::Mixer::process',  file: 'audio/mixer.cc',  line: 1104 },
        { fn: 'guardrail::audio::Stream::pull',    file: 'audio/stream.cc', line: 412 },
        { fn: 'guardrail::runtime::tick',          file: 'runtime/loop.cc', line: 214 },
        { fn: 'main',                               file: 'main.cc',         line: 44 }
      ]
    })
  },
  {
    key: 'mac-gpu-lost', platform: 'macos',
    summary: 'GPU device lost during submit',
    make: () => makeMacDump({
      exception: 'EXC_BAD_ACCESS / KERN_PROTECTION_FAILURE', address: '0x0000000000000018',
      crashingFrames: [
        { fn: 'guardrail::gpu::Device::submit',  file: 'gpu/device.cc',  line: 645 },
        { fn: 'guardrail::gpu::Queue::flush',    file: 'gpu/queue.cc',   line: 201 },
        { fn: 'guardrail::renderer::Scene::draw', file: 'renderer/scene.cc', line: 118 },
        { fn: 'guardrail::runtime::tick',        file: 'runtime/loop.cc', line: 214 }
      ]
    })
  },
  {
    key: 'linux-oom', platform: 'linux',
    summary: 'Out of memory allocating 2.3 GiB in Pool::grow',
    make: () => makeLinuxDump({
      exception: 'SIGABRT / abort()', address: '0x0000000000000006',
      crashingFrames: [
        { fn: 'guardrail::alloc::Pool::grow', file: 'alloc/pool.cc', line: 401 },
        { fn: 'operator new',                  file: 'libstdc++/new_op.cc', line: 52 },
        { fn: 'guardrail::storage::Index::rebuild', file: 'storage/index.cc', line: 89 },
        { fn: 'main',                          file: 'main.cc', line: 44 }
      ]
    })
  },
  {
    key: 'linux-sigbus', platform: 'linux',
    summary: 'SIGBUS reading shared-memory channel after peer exit',
    make: () => makeLinuxDump({
      exception: 'SIGBUS / BUS_ADRERR', address: '0x00007f6b12345678',
      crashingFrames: [
        { fn: 'guardrail::ipc::Channel::recv',    file: 'ipc/channel.cc', line: 158 },
        { fn: 'guardrail::ipc::Service::dispatch', file: 'ipc/service.cc', line: 44 },
        { fn: 'guardrail::runtime::tick',          file: 'runtime/loop.cc', line: 214 },
        { fn: 'main',                               file: 'main.cc', line: 44 }
      ]
    })
  }
];

// ------------------------------------------------------------------
// Ingest: derive normalized facts from a raw minidump. Mirrors the
// prototype's ingestReport().
// ------------------------------------------------------------------

function shortenExceptionType(t: string): string {
  if (!t) return '';
  let s = t.replace(/^EXCEPTION_/, '').replace(/^EXC_/, '').replace(/^SIG/, '');
  s = s.toLowerCase().replace(/\s*\/\s*/g, '_').replace(/[^a-z0-9_]+/g, '_').replace(/_+/g, '_').replace(/^_|_$/g, '');
  return s;
}

function platformOf(os: string): Derived['platform'] {
  if (/windows/i.test(os)) return 'windows';
  if (/mac|darwin/i.test(os)) return 'macos';
  return 'linux';
}

function symbolCoverage(dump: Dump): number {
  let total = 0, symbolicated = 0;
  for (const th of dump.threads) {
    for (const f of th.frames) {
      total++;
      if (f.function) symbolicated++;
    }
  }
  return total === 0 ? 0 : symbolicated / total;
}

function ingest(dump: Dump): { dump: Dump; derived: Derived } {
  const ct = dump.threads.find((t) => t.thread_id === dump.crash_info.crashing_thread) || dump.threads[0];
  const topFrame = ct.frames[0];
  const exShort = shortenExceptionType(dump.crash_info.type);
  const topSymbol = topFrame.function || `<${topFrame.module}+${topFrame.module_offset}>`;
  const mainMod = dump.modules[dump.main_module]?.filename || 'unknown';
  const topFile = topFrame.file ? topFrame.file.replace(/^\\\\\?\\/, '').split(/[\\/]/).slice(-2).join('/') : null;
  const derived: Derived = {
    title: `${exShort} in ${topSymbol.split('::').slice(-2).join('::')}`,
    exceptionType: dump.crash_info.type,
    exceptionTypeShort: exShort,
    address: dump.crash_info.address,
    topSymbol,
    topFile,
    topLine: topFrame.line,
    platform: platformOf(dump.system_info.os),
    mainModuleName: mainMod,
    symbolCoverage: symbolCoverage(dump)
  };
  return { dump, derived };
}

// ------------------------------------------------------------------
// Logs + breadcrumbs builders (parallel to the prototype).
// ------------------------------------------------------------------

function buildLogs(ingested: { dump: Dump; derived: Derived }) {
  const { derived, dump } = ingested;
  const d = new Date().toISOString().replace('T', ' ').slice(0, 19);
  const body = [
    `[${d}.221] INFO  app            startup complete (cold, 1812ms)`,
    `[${d}.884] WARN  gpu.swapchain  recreated swapchain for resize 1440x900 -> 2560x1600`,
    `[${d}.552] DEBUG autosave       snapshotting (lvl=diff)`,
    `[${d}.722] ERROR renderer       ${derived.title.toLowerCase()}`,
    `[${d}.722] ERROR runtime        fatal: ${derived.exceptionType} at ${derived.address}`,
    `[${d}.725] INFO  runtime        writing minidump (pid=${dump.pid})`,
    `[${d}.801] INFO  runtime        minidump written (3.2 MB); attaching logs`
  ].join('\n');
  return [
    { name: 'guardrail.log', size: '184.2 KB', lines: 2874, level: 'mixed', body },
    { name: 'stderr.log',    size: '1.2 KB',   lines: 18,   level: 'error',
      body: `fatal runtime error: ${derived.exceptionType} at ${derived.address}\n` }
  ];
}

function buildBreadcrumbs(ingested: { dump: Dump; derived: Derived }) {
  const { derived } = ingested;
  return [
    { t: '-00:00.04', level: 'error' as const, category: 'runtime', msg: `${derived.exceptionTypeShort} at ${derived.address}` },
    { t: '-00:00.12', level: 'info'  as const, category: 'ui',      msg: 'Project opened (442 MB, 318 layers)' },
    { t: '-00:00.48', level: 'warn'  as const, category: 'gpu',     msg: 'Swapchain recreated (window resize 1440→2560)' },
    { t: '-00:02.11', level: 'info'  as const, category: 'net',     msg: 'WebSocket connected to wss://sync.guardrail.app' },
    { t: '-00:03.77', level: 'info'  as const, category: 'app',     msg: 'Startup complete (cold start, 1.8s)' }
  ];
}

const USER_DESCRIPTIONS = [
  { author: 'elena.k@studio.co', body: "Was rendering a preview of the opening sequence when the app just vanished. No dialog, just gone. I'd been working for about 20 minutes without saving. This is the third time this week on the same project." },
  { author: 'd_whitman', body: 'Pressed cmd+shift+E to export and got the spinning beachball, then crash. Project is ~450MB with a lot of nested groups. Happens every single time with this file — fine with smaller projects.' },
  { author: 'anon', body: 'crashed during autosave i think. didnt lose work because the autosave actually completed (saw the toast right before). repro steps: open big file > let it sit for 5 min > crash' },
  { author: 'mboucher', body: "It crashed right after I plugged in my external monitor. Laptop screen is 1440p, external is 4K. Seems to happen any time I hot-plug displays while the app is in the foreground." },
  { author: null, body: null }
];

// ------------------------------------------------------------------
// Build the 60-group dataset.
// ------------------------------------------------------------------

// Build a Crash from a freshly-ingested archetype. Each call regenerates
// the dump (so randomized fields like offsets vary per crash), giving every
// crash in a group its own concrete detail blobs.
function makeCrash(opts: {
  i: number;
  j: number;
  archIdx: number;
  groupId: string;
  productId: string;
  build: string;
  version: string;
  firstSeenDays: number;
}): Crash {
  const arch = ARCHETYPES[opts.archIdx];
  const ingested = ingest(arch.make());
  const { dump, derived } = ingested;
  const ct = dump.threads.find((t) => t.thread_id === dump.crash_info.crashing_thread)!;
  const mainMod = dump.modules[dump.main_module]?.filename || '';
  const userCtx = USER_DESCRIPTIONS[(opts.i + opts.j) % USER_DESCRIPTIONS.length];
  return {
    id: `CR-${String(opts.i + 1).padStart(4, '0')}-${String(opts.j + 1).padStart(3, '0')}`,
    groupId: opts.groupId,
    productId: opts.productId,

    version: opts.j === 0 ? opts.version : rnd() < 0.7 ? opts.version : pick(VERSIONS),
    os: `${dump.system_info.os} ${dump.system_info.os_ver}`,
    at: relTime(randInt(0, opts.firstSeenDays), randInt(0, 23)),
    user: 'u_' + hex(6),
    similarity: 1 - rnd() * 0.05,
    commit: hex(7),

    signal: derived.exceptionTypeShort,
    title: derived.title,
    topFrame: derived.topSymbol,
    file: derived.topFile || '<no source>',
    line: derived.topLine || 0,
    address: derived.address,
    platform: derived.platform,
    exceptionType: derived.exceptionType,
    exceptionTypeShort: derived.exceptionTypeShort,
    build: opts.j === 0 ? opts.build : hex(7),

    stack: ct.frames.map((f) => ({
      fn: f.function || `<${f.module}+${f.module_offset}>`,
      file: f.file
        ? f.file.replace(/^\\\\\?\\/, '').split(/[\\/]/).slice(-2).join('/')
        : (f.module || ''),
      line: f.line || 0,
      addr: f.offset,
      inApp: f.module === mainMod,
      trust: f.trust
    })),
    threads: dump.threads.map((th) => ({
      id: th.thread_id,
      name: th.thread_name
        || (th.thread_id === dump.crash_info.crashing_thread ? 'crashing thread' : `thread ${th.thread_id}`),
      crashed: th.thread_id === dump.crash_info.crashing_thread,
      frames: th.frame_count
    })),
    modules: dump.modules.slice(0, 18).map((m) => ({
      name: m.filename,
      version: m.version !== '0.0.0.0' ? m.version : '',
      addr: m.base_addr,
      size: humanSize(parseInt(m.end_addr, 16) - parseInt(m.base_addr, 16)),
      inApp: m.filename === mainMod
    })),
    env: {
      os: `${dump.system_info.os} ${dump.system_info.os_ver}`,
      arch: dump.system_info.cpu_arch,
      cpu: `${dump.system_info.cpu_info} (${dump.system_info.cpu_count} cores)`,
      ram: pick(['16 GB', '32 GB', '64 GB']),
      gpu: pick(['Apple M3 Pro (integrated)', 'NVIDIA RTX 4070', 'AMD Radeon 7900 XT', 'Intel Arc A770']),
      locale: pick(['en-US', 'en-GB', 'de-DE', 'ja-JP', 'es-ES'])
    },
    breadcrumbs: buildBreadcrumbs(ingested),
    logs: buildLogs(ingested),
    userDescription: userCtx.body
      ? { author: userCtx.author, at: relTime(0, randInt(1, 48)), body: userCtx.body }
      : null,
    dump,
    derived
  };
}

function buildDataset(): CrashGroup[] {
  const groups: CrashGroup[] = [];
  for (let i = 0; i < 60; i++) {
    const archIdx = i % ARCHETYPES.length;
    const groupId = `GR-${String(i + 1).padStart(4, '0')}`;
    const count = randInt(2, i < 6 ? 420 : i < 20 ? 80 : 30);
    const firstSeenDays = randInt(1, 60);
    const lastSeenHours = randInt(0, 72);
    const version = pick(VERSIONS);
    const build = hex(7);
    const similarity = 1 - (i === 0 ? 0 : rnd() * 0.08);
    const roll = rnd();
    const status: Status = roll < 0.55 ? 'new' : roll < 0.85 ? 'triaged' : 'resolved';
    const assignee = status === 'new' ? null : pick(ASSIGNEES);

    const N = Math.min(count, 12);
    const crashes: Crash[] = [];
    for (let j = 0; j < N; j++) {
      crashes.push(makeCrash({
        i, j, archIdx, groupId, productId: '', build, version, firstSeenDays
      }));
    }
    crashes.sort((a, b) => (a.at < b.at ? 1 : -1));

    // Group's display fields come from the canonical (most-recent / first)
    // crash. They drive the list row, not the detail pane.
    const canonical = crashes[0];

    const group: CrashGroup = {
      id: groupId,
      productId: '', // assigned below once PRODUCTS exist
      signal: canonical.signal,
      exceptionType: canonical.exceptionType,
      exceptionTypeShort: canonical.exceptionTypeShort,
      title: canonical.title,
      topFrame: canonical.topFrame,
      file: canonical.file,
      line: canonical.line,
      address: canonical.address,
      platform: canonical.platform,
      version,
      build,
      count,
      similarity,
      status,
      assignee,
      firstSeen: relTime(firstSeenDays),
      lastSeen: relTime(0, lastSeenHours),
      crashes,
      notes: i === 0 ? [
        { author: 'mlin',  at: relTime(0, 6), body: 'Repro is reliable — null write in renderer, rax/rdx both zero at the trap site. Looks like a missing null-check on the object returned from lookup.' },
        { author: 'rwang', at: relTime(0, 3), body: 'Scan frames beyond #4 are into unsymbolicated Kernel32 — ignore those for root-cause.' }
      ] : i === 1 ? [
        { author: 'tkowalski', at: relTime(1, 2), body: 'Likely fixed by #4821 on main. Will verify after next beta cut.' }
      ] : [],
      related: []
    };
    groups.push(group);
  }

  // Link related groups by archetype (i % ARCHETYPES.length).
  for (const g of groups) {
    const idx = parseInt(g.id.slice(3), 10) - 1;
    const archKey = ARCHETYPES[idx % ARCHETYPES.length].key;
    g.related = groups
      .filter((o) => {
        const oi = parseInt(o.id.slice(3), 10) - 1;
        return o.id !== g.id && ARCHETYPES[oi % ARCHETYPES.length].key === archKey;
      })
      .slice(0, 3)
      .map((o) => ({ id: o.id, title: o.title, count: o.count }));
  }

  groups.sort((a, b) => b.count - a.count);
  return groups;
}

// ------------------------------------------------------------------
// Session / identity tables.
// ------------------------------------------------------------------

const PRODUCTS: Product[] = [
  { id: 'guardrail', name: 'Guardrail',      slug: 'guardrail', color: '#3b6fd4', description: 'Main desktop app' },
  { id: 'harpoon',   name: 'Harpoon',        slug: 'harpoon',   color: '#0f766e', description: 'Shared injection library (Windows)' },
  { id: 'rivet',     name: 'Rivet',          slug: 'rivet',     color: '#b45309', description: 'CLI build tooling' },
  { id: 'workrave',  name: 'Workrave Agent', slug: 'workrave',  color: '#9333ea', description: 'Background service' }
];

const USERS: User[] = [
  { id: 'u-you',       email: 'you@studio.co',      name: 'You',             avatar: 'YU', isAdmin: true,  joinedAt: relTime(220) },
  { id: 'u-mlin',      email: 'mlin@guardrail.co',  name: 'Maya Lin',        avatar: 'ML', isAdmin: false, joinedAt: relTime(180) },
  { id: 'u-rwang',     email: 'rwang@guardrail.co', name: 'Ren Wang',        avatar: 'RW', isAdmin: false, joinedAt: relTime(150) },
  { id: 'u-tkowalski', email: 'tk@guardrail.co',    name: 'Tomasz Kowalski', avatar: 'TK', isAdmin: true,  joinedAt: relTime(95)  },
  { id: 'u-ahmed',     email: 'ahmed@guardrail.co', name: 'Ahmed Raza',      avatar: 'AR', isAdmin: false, joinedAt: relTime(60)  },
  { id: 'u-sofia',     email: 'sofia@guardrail.co', name: 'Sofía Méndez',    avatar: 'SM', isAdmin: false, joinedAt: relTime(30)  },
  { id: 'u-jhale',     email: 'jhale@partners.co',  name: 'Jordan Hale',     avatar: 'JH', isAdmin: false, joinedAt: relTime(12)  }
];

const MEMBERSHIPS: Membership[] = [
  { userId: 'u-you',       productId: 'guardrail', role: 'maintainer' },
  { userId: 'u-you',       productId: 'harpoon',   role: 'readwrite'  },
  { userId: 'u-you',       productId: 'rivet',     role: 'readonly'   },
  { userId: 'u-mlin',      productId: 'guardrail', role: 'maintainer' },
  { userId: 'u-mlin',      productId: 'harpoon',   role: 'readwrite'  },
  { userId: 'u-rwang',     productId: 'guardrail', role: 'readwrite'  },
  { userId: 'u-rwang',     productId: 'rivet',     role: 'maintainer' },
  { userId: 'u-tkowalski', productId: 'guardrail', role: 'readwrite'  },
  { userId: 'u-tkowalski', productId: 'workrave',  role: 'maintainer' },
  { userId: 'u-ahmed',     productId: 'harpoon',   role: 'maintainer' },
  { userId: 'u-ahmed',     productId: 'rivet',     role: 'readwrite'  },
  { userId: 'u-sofia',     productId: 'guardrail', role: 'readonly'   },
  { userId: 'u-sofia',     productId: 'workrave',  role: 'readwrite'  },
  { userId: 'u-jhale',     productId: 'rivet',     role: 'readonly'   }
];

// Module-level singleton so mutations persist across requests.
const DB: CrashGroup[] = buildDataset();

// Assign groups to products round-robin (stable, by index).
DB.forEach((g, i) => {
  g.productId = PRODUCTS[i % PRODUCTS.length].id;
  for (const c of g.crashes) c.productId = g.productId;
});
// Re-link "related" within the same product.
for (const g of DB) {
  g.related = DB
    .filter((o) => o.id !== g.id && o.productId === g.productId
      && o.exceptionTypeShort === g.exceptionTypeShort)
    .slice(0, 3)
    .map((o) => ({ id: o.id, title: o.title, count: o.count }));
}

// ------------------------------------------------------------------
// Symbols.
// ------------------------------------------------------------------

function buildSymbols(): SymbolRow[] {
  const catalog: Record<string, Array<[string, string, string, string, string]>> = {
    guardrail: [
      ['Guardrail',         '2.14.0', 'arm64',  'dSYM', '172 MB'],
      ['Guardrail',         '2.13.4', 'arm64',  'dSYM', '168 MB'],
      ['Guardrail',         '2.14.0', 'x86_64', 'dSYM', '188 MB'],
      ['guardrail.exe',     '2.14.0', 'x86_64', 'PDB',   '84 MB'],
      ['guardrail.exe',     '2.13.4', 'x86_64', 'PDB',   '82 MB'],
      ['guardrail',         '2.14.0', 'x86_64', 'ELF',   '94 MB'],
      ['librenderer.dylib', '2.14.0', 'arm64',  'dSYM',  '42 MB'],
      ['libaudio.so',       '2.14.0', 'x86_64', 'ELF',   '18 MB'],
      ['libaudio.so',       '2.12.9', 'x86_64', 'ELF',   '17 MB'],
      ['libnet.dll',        '2.14.0', 'x86_64', 'PDB',   '12 MB']
    ],
    harpoon: [
      ['harpoon64.dll',      '1.4.2', 'x86_64', 'PDB', '8.4 MB'],
      ['harpoon64.dll',      '1.4.1', 'x86_64', 'PDB', '8.1 MB'],
      ['harpoon32.dll',      '1.4.2', 'x86',    'PDB', '6.2 MB'],
      ['harpoon-inject.exe', '1.4.2', 'x86_64', 'PDB', '3.8 MB'],
      ['harpoon-inject.exe', '1.4.0', 'x86_64', 'PDB', '3.7 MB']
    ],
    rivet: [
      ['rivet',     '0.9.2', 'x86_64', 'ELF', '22 MB'],
      ['rivet',     '0.9.2', 'arm64',  'ELF', '21 MB'],
      ['rivet.exe', '0.9.2', 'x86_64', 'PDB', '14 MB'],
      ['rivet.exe', '0.9.1', 'x86_64', 'PDB', '13 MB']
    ],
    workrave: [
      ['workraved',         '3.1.0', 'x86_64', 'ELF', '11 MB'],
      ['workraved',         '3.0.8', 'x86_64', 'ELF', '10 MB'],
      ['WorkraveAgent.exe', '3.1.0', 'x86_64', 'PDB', '7.4 MB']
    ]
  };
  const rows: SymbolRow[] = [];
  let n = 1;
  for (const [pid, mods] of Object.entries(catalog)) {
    for (const [name, version, arch, format, size] of mods) {
      rows.push({
        id: `SYM-${String(n).padStart(4, '0')}`,
        productId: pid,
        name, version, arch, format, size,
        debugId: (hex(8) + hex(8) + hex(8) + hex(8) + '1').toUpperCase(),
        codeId: hex(14),
        uploadedAt: relTime(randInt(0, 120), randInt(0, 23)),
        uploadedBy: pick(['u-mlin', 'u-rwang', 'u-tkowalski', 'u-ahmed', 'u-you']),
        referencedBy: randInt(0, 40)
      });
      n++;
    }
  }
  rows.sort((a, b) => (a.uploadedAt < b.uploadedAt ? 1 : -1));
  return rows;
}

const SYMBOLS: SymbolRow[] = buildSymbols();

// ------------------------------------------------------------------
// Adapter implementation.
// ------------------------------------------------------------------

function toSummary(g: CrashGroup): CrashGroupSummary {
  const { crashes, notes, related, ...s } = g;
  return s;
}

export const mockAdapter: GuardrailAdapter = {
  // --- session ---
  async signIn(email) {
    return USERS.find((u) => u.email.toLowerCase() === (email || '').toLowerCase()) ?? null;
  },
  async getUser(id) { return USERS.find((u) => u.id === id) ?? null; },

  // --- products ---
  async listProducts(scope = 'all', userId) {
    if (scope === 'all') return PRODUCTS.slice();
    if (!userId) return [];
    const ids = new Set(MEMBERSHIPS.filter((m) => m.userId === userId).map((m) => m.productId));
    return PRODUCTS.filter((p) => ids.has(p.id));
  },
  async getProduct(id) { return PRODUCTS.find((p) => p.id === id) ?? null; },
  async createProduct({ name, slug, description }) {
    const id = (slug || name.toLowerCase().replace(/[^a-z0-9]+/g, '-')).trim();
    if (PRODUCTS.some((p) => p.id === id)) throw new Error(`Product "${id}" already exists`);
    const p: Product = { id, name, slug: id, description: description || '', color: '#6b7280' };
    PRODUCTS.push(p);
    return p;
  },
  async deleteProduct(id) {
    const i = PRODUCTS.findIndex((p) => p.id === id); if (i >= 0) PRODUCTS.splice(i, 1);
    for (let j = MEMBERSHIPS.length - 1; j >= 0; j--)
      if (MEMBERSHIPS[j].productId === id) MEMBERSHIPS.splice(j, 1);
    for (let j = DB.length - 1; j >= 0; j--)
      if (DB[j].productId === id) DB.splice(j, 1);
    for (let j = SYMBOLS.length - 1; j >= 0; j--)
      if (SYMBOLS[j].productId === id) SYMBOLS.splice(j, 1);
  },

  // --- users ---
  async listUsers() { return USERS.slice(); },
  async createUser({ email, name }) {
    const clean = email.trim().toLowerCase();
    if (USERS.some((u) => u.email.toLowerCase() === clean))
      throw new Error(`A user with email "${clean}" already exists`);
    const id = 'u-' + clean.split('@')[0].replace(/[^a-z0-9]/gi, '');
    const finalName = (name || '').trim() || email;
    const u: User = {
      id, email, name: finalName,
      avatar: finalName.split(/\s+/).map((w) => w[0]).slice(0, 2).join('').toUpperCase() || 'U',
      isAdmin: false, joinedAt: new Date().toISOString()
    };
    USERS.push(u);
    return u;
  },
  async deleteUser(id) {
    const i = USERS.findIndex((u) => u.id === id); if (i >= 0) USERS.splice(i, 1);
    for (let j = MEMBERSHIPS.length - 1; j >= 0; j--)
      if (MEMBERSHIPS[j].userId === id) MEMBERSHIPS.splice(j, 1);
  },
  async setAdmin(id, isAdmin) {
    const u = USERS.find((x) => x.id === id); if (u) u.isAdmin = !!isAdmin;
  },

  // --- memberships ---
  async listMembers(productId): Promise<MembershipWithUser[]> {
    return MEMBERSHIPS
      .filter((m) => m.productId === productId)
      .map((m) => ({ ...m, user: USERS.find((u) => u.id === m.userId)! }))
      .filter((m) => m.user); // drop orphans defensively
  },
  async membershipsFor(userId): Promise<MembershipWithProduct[]> {
    return MEMBERSHIPS
      .filter((m) => m.userId === userId)
      .map((m) => ({ ...m, product: PRODUCTS.find((p) => p.id === m.productId)! }))
      .filter((m) => m.product);
  },
  async roleOf(userId, productId): Promise<Role | null> {
    return MEMBERSHIPS.find((m) => m.userId === userId && m.productId === productId)?.role ?? null;
  },
  async grantAccess({ userId, productId, role }) {
    const existing = MEMBERSHIPS.find((m) => m.userId === userId && m.productId === productId);
    if (existing) existing.role = role;
    else MEMBERSHIPS.push({ userId, productId, role });
  },
  async revokeAccess({ userId, productId }) {
    const i = MEMBERSHIPS.findIndex((m) => m.userId === userId && m.productId === productId);
    if (i >= 0) MEMBERSHIPS.splice(i, 1);
  },

  // --- crashes ---
  async listGroups(q: ListQuery): Promise<ListResult> {
    let r = DB.filter((g) => g.productId === q.productId);
    if (q.version && q.version !== 'all') r = r.filter((g) => g.version === q.version);
    if (q.status && q.status !== ('all' as Status)) r = r.filter((g) => g.status === q.status);
    if (q.search && q.search.trim()) {
      const s = q.search.toLowerCase();
      r = r.filter((g) => g.title.toLowerCase().includes(s) || g.topFrame.toLowerCase().includes(s));
    }
    r = [...r];
    switch (q.sort) {
      case 'recent':     r.sort((a, b) => (a.lastSeen < b.lastSeen ? 1 : -1)); break;
      case 'similarity': r.sort((a, b) => b.similarity - a.similarity); break;
      case 'version':    r.sort((a, b) => b.version.localeCompare(a.version)); break;
      default:           r.sort((a, b) => b.count - a.count);
    }
    const total = r.length;
    const off = q.offset ?? 0;
    const lim = q.limit ?? r.length;
    return { total, versions: VERSIONS, groups: r.slice(off, off + lim).map(toSummary) };
  },
  async getGroup(id) { return DB.find((g) => g.id === id) ?? null; },
  async getCrash(id) {
    for (const g of DB) {
      const crash = g.crashes.find((c) => c.id === id);
      if (crash) return { crash, group: g };
    }
    return null;
  },
  async setStatus(id, status) {
    const g = DB.find((x) => x.id === id);
    if (g) g.status = status;
  },
  async addNote(id, body, author): Promise<Note> {
    const g = DB.find((x) => x.id === id);
    const note: Note = { author, body, at: new Date().toISOString() };
    if (g) g.notes = [...g.notes, note];
    return note;
  },
  async mergeGroups(primaryId, mergedId) {
    const primary = DB.find((x) => x.id === primaryId);
    const merged = DB.find((x) => x.id === mergedId);
    if (!primary || !merged) return;
    primary.count += merged.count;
    primary.crashes = [...primary.crashes, ...merged.crashes]
      .sort((a, b) => (a.at < b.at ? 1 : -1));
    for (const c of primary.crashes) c.groupId = primary.id;
    const i = DB.indexOf(merged);
    if (i >= 0) DB.splice(i, 1);
  },

  // --- symbols ---
  async listSymbols(productId, q = {}): Promise<SymbolRow[]> {
    let r = SYMBOLS.filter((s) => s.productId === productId);
    if (q.search && q.search.trim()) {
      const s = q.search.toLowerCase();
      r = r.filter((x) => x.name.toLowerCase().includes(s) || x.debugId.toLowerCase().includes(s));
    }
    if (q.arch && q.arch !== 'all')     r = r.filter((x) => x.arch === q.arch);
    if (q.format && q.format !== 'all') r = r.filter((x) => x.format === q.format);
    r = [...r];
    switch (q.sort) {
      case 'name': r.sort((a, b) => a.name.localeCompare(b.name) || b.version.localeCompare(a.version)); break;
      case 'size': r.sort((a, b) => parseFloat(b.size) - parseFloat(a.size)); break;
      default:     r.sort((a, b) => (a.uploadedAt < b.uploadedAt ? 1 : -1));
    }
    return r;
  },
  async uploadSymbol(productId, spec): Promise<SymbolRow> {
    const n = SYMBOLS.length + 1;
    const s: SymbolRow = {
      id: `SYM-${String(n).padStart(4, '0')}`,
      productId,
      name: spec.name,
      version: spec.version || '0.0.0',
      arch: spec.arch || 'x86_64',
      format: spec.format || 'PDB',
      size: spec.size || '1.0 MB',
      debugId: (hex(8) + hex(8) + hex(8) + hex(8) + '1').toUpperCase(),
      codeId: hex(14),
      uploadedAt: new Date().toISOString(),
      uploadedBy: spec.uploadedBy,
      referencedBy: 0
    };
    SYMBOLS.unshift(s);
    return s;
  },
  async deleteSymbol(id) {
    const i = SYMBOLS.findIndex((s) => s.id === id); if (i >= 0) SYMBOLS.splice(i, 1);
  }
};
