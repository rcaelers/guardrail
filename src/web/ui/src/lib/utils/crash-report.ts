import type { Crash, DumpFrame, DumpThread } from '$lib/adapters/types';

export function shortenExceptionType(value: string | undefined | null): string {
  if (!value) return '';
  let shortened = value.replace(/^EXCEPTION_/, '').replace(/^EXC_/, '').replace(/^SIG/, '');
  shortened = shortened
    .toLowerCase()
    .replace(/\s*\/\s*/g, '_')
    .replace(/[^a-z0-9_]+/g, '_')
    .replace(/_+/g, '_')
    .replace(/^_|_$/g, '');
  return shortened;
}

export function platformOf(os: string | undefined | null): Crash['platform'] {
  if (!os) return undefined;
  if (/windows/i.test(os)) return 'windows';
  if (/mac|darwin/i.test(os)) return 'macos';
  return 'linux';
}

export function shortenSourcePath(path: string | undefined | null): string {
  if (!path) return '<no source>';
  return path.replace(/^\\\\\?\\/, '').split(/[\\/]/).slice(-2).join('/') || path;
}

export function getCrashingThread(crash: Crash): DumpThread | null {
  if (crash.crashing_thread) return crash.crashing_thread;
  const threads = crash.threads ?? [];
  if (threads.length === 0) return null;

  const crashingThread = crash.crash_info?.crashing_thread;
  if (typeof crashingThread !== 'number') return threads[0] ?? null;

  return (
    threads[crashingThread]
    ?? threads.find((thread) => thread.thread_id === crashingThread)
    ?? threads[0]
    ?? null
  );
}

export function getTopFrame(crash: Crash): DumpFrame | null {
  return getCrashingThread(crash)?.frames?.[0] ?? null;
}

export function topFrameLabel(crash: Crash): string {
  if (crash.topFrame) return crash.topFrame;
  const frame = getTopFrame(crash);
  if (!frame) return '<unknown frame>';
  return frame.function || `<${frame.module ?? 'unknown'}+${frame.module_offset ?? '??'}>`;
}

export function topFrameFile(crash: Crash): string {
  if (crash.file) return crash.file;
  return shortenSourcePath(getTopFrame(crash)?.file);
}

export function topFrameLine(crash: Crash): number {
  return crash.line ?? getTopFrame(crash)?.line ?? 0;
}

export function exceptionType(crash: Crash): string {
  return crash.exceptionType || crash.crash_info?.type || 'UNKNOWN';
}

export function exceptionTypeShort(crash: Crash): string {
  return crash.exceptionTypeShort || shortenExceptionType(exceptionType(crash)) || 'unknown';
}

export function crashAddress(crash: Crash): string {
  return crash.address || crash.crash_info?.address || '—';
}

export function crashPlatform(crash: Crash): Crash['platform'] {
  return crash.platform || platformOf(crash.system_info?.os);
}

export function crashTitle(crash: Crash): string {
  if (crash.title) return crash.title;
  const top = topFrameLabel(crash);
  const short = exceptionTypeShort(crash);
  const leaf = top.split('::').slice(-2).join('::') || top;
  return `${short} in ${leaf}`;
}

export function crashOs(crash: Crash): string {
  if (crash.os) return crash.os;
  const os = crash.system_info?.os;
  const version = crash.system_info?.os_ver;
  return [os, version].filter(Boolean).join(' ') || '—';
}

export function mainModuleName(crash: Crash): string {
  if (typeof crash.main_module !== 'number' || !crash.modules) return 'unknown';
  return crash.modules[crash.main_module]?.filename || 'unknown';
}

export function threadDisplayName(crash: Crash, thread: DumpThread): string {
  if (thread.thread_name) return thread.thread_name;
  const crashing = getCrashingThread(crash);
  if (crashing?.thread_id === thread.thread_id) return 'crashing thread';
  return `thread ${thread.thread_id}`;
}
