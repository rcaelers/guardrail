// Shape of the data Guardrail consumes. Any backend (mock, HTTP, gRPC,
// SQL-over-tauri, etc.) should implement GuardrailAdapter.
//
// The adapter has three concerns:
//   - Session + identity (signIn, me, currentProduct)
//   - Product-scoped reads (crashes, symbols, members)
//   - Admin writes (users, products)

export type Status = 'new' | 'triaged' | 'resolved';
export type Signal = 'SIGSEGV' | 'SIGABRT' | 'panic' | 'SIGBUS' | 'OOM' | 'assertion' | string;
export type Role = 'readonly' | 'readwrite' | 'maintainer';

// ------------------------------------------------------------------
// Crash group shapes (unchanged from previous revision).
// ------------------------------------------------------------------

export interface Occurrence {
  id: string;
  version: string;
  os: string;
  at: string;
  user: string;
  similarity: number;
  commit: string;
}

export interface StackFrame {
  fn: string;
  file: string;
  line: number;
  addr: string;
  inApp: boolean;
  trust?: string;
}

export interface Thread {
  id: number;
  name: string;
  crashed: boolean;
  frames: number;
}

export interface Module {
  name: string;
  version: string;
  addr: string;
  size: string;
  inApp: boolean;
}

export interface Breadcrumb {
  t: string;
  level: 'info' | 'warn' | 'error' | 'debug';
  category: string;
  msg: string;
}

export interface LogFile {
  name: string;
  size: string;
  lines: number;
  level: string;
  body: string;
}

export interface UserDescription {
  author: string | null;
  at: string;
  body: string;
}

export interface Note {
  author: string;
  at: string;
  body: string;
}

export interface CrashAttachment {
  id: string;
  name: string;
  filename: string;
  mimeType: string;
  size: number;
  createdAt: string;
}

export interface CrashUserText {
  attachmentId: string;
  body: string;
  filename: string;
  createdAt: string;
}

export interface RelatedRef {
  id: string;
  title: string;
  count: number;
}

export interface Environment {
  os: string;
  arch: string;
  cpu: string;
  ram: string;
  gpu: string;
  locale: string;
}

export interface CrashGroupSummary {
  id: string;
  productId: string;
  signal: Signal;
  exceptionType?: string;
  exceptionTypeShort?: string;
  title: string;
  topFrame: string;
  file: string;
  line: number;
  address?: string;
  platform?: 'windows' | 'macos' | 'linux' | string;
  version: string;
  build: string;
  count: number;
  similarity: number;
  status: Status;
  assignee: string | null;
  firstSeen: string;
  lastSeen: string;
}

// ------------------------------------------------------------------
// Raw minidump shape — mirrors minidump-stackwalk JSON output.
// ------------------------------------------------------------------

export interface DumpFrame {
  frame: number;
  trust: string;
  module: string | null;
  offset: string;
  module_offset: string | null;
  function: string | null;
  function_offset: string | null;
  file: string | null;
  line: number | null;
  inlines: unknown;
  missing_symbols: boolean;
  unloaded_modules: unknown;
}

export interface DumpThread {
  thread_id: number;
  thread_name: string | null;
  frame_count: number;
  last_error_value: string;
  threads_index?: number;
  frames: DumpFrame[];
}

export interface DumpModule {
  filename: string;
  version: string;
  base_addr: string;
  end_addr: string;
  code_id: string;
  debug_id: string;
  debug_file: string;
  loaded_symbols: boolean;
  corrupt_symbols: boolean;
  missing_symbols: boolean;
}

export interface CrashHandle {
  handle: number;
  type_name: string | null;
  object_name: string | null;
}

export interface Dump {
  pid: number;
  status: string;
  thread_count: number;
  main_module: number;
  crash_info: {
    type: string;
    address: string;
    crashing_thread: number;
  };
  system_info: {
    os: string;
    os_ver: string;
    cpu_arch: string;
    cpu_info: string;
    cpu_count: number;
    cpu_microcode_version: string | null;
  };
  modules: DumpModule[];
  threads: DumpThread[];
  unloaded_modules: unknown[];
}

export interface CrashReport extends Dump {
  crash_info: Dump['crash_info'] & {
    assertion?: string | null;
    instruction?: string | null;
    memory_accesses?: unknown;
    adjusted_address?: string | null;
    possible_bit_flips?: unknown;
  };
  crashing_thread?: DumpThread | null;
  handles?: CrashHandle[];
  linux_memory_map_count?: number | null;
  lsb_release?: string | null;
  mac_boot_args?: string | null;
  mac_crash_info?: unknown;
  modules_contains_cert_info?: boolean;
  proc_limits?: unknown;
}

export interface Derived {
  title: string;
  exceptionType: string;
  exceptionTypeShort: string;
  address: string;
  topSymbol: string;
  topFile: string | null;
  topLine: number | null;
  platform: 'windows' | 'macos' | 'linux' | string;
  mainModuleName: string;
  symbolCoverage: number;
}

// One crash event. The detail pane renders this — `Crash` is what the user
// is looking at. Multiple crashes with the same fingerprint share a group.
export interface Crash extends Partial<CrashReport> {
  id: string;
  groupId: string;
  productId: string;

  // Per-crash metadata
  version: string;
  os: string;
  at: string;
  user: string;
  similarity: number;
  commit: string;

  // Per-crash summary (what shows in the detail header for THIS crash)
  signal?: Signal;
  title?: string;
  topFrame?: string;
  file?: string;
  line?: number;
  address?: string;
  platform?: 'windows' | 'macos' | 'linux' | string;
  exceptionType?: string;
  exceptionTypeShort?: string;
  build: string;
  attachments?: CrashAttachment[];
  userText?: CrashUserText | null;
}

// Lightweight crash summary used in the expanded group row.
export interface CrashSummary {
  id: string;
  version: string;
  os: string;
  at: string;
  user: string;
  similarity: number;
  commit: string;
}

// A group is the workflow + aggregation entity. List/header summary fields
// come from a canonical crash (the first one); detail comes from `crashes`.
export interface CrashGroup extends CrashGroupSummary {
  crashes: Crash[];
  notes: Note[];
  related: RelatedRef[];
}

// ------------------------------------------------------------------
// Identity, products, memberships.
// ------------------------------------------------------------------

export interface User {
  id: string;
  email: string;
  name: string;
  avatar: string; // initials
  isAdmin: boolean;
  joinedAt: string;
}

export interface Product {
  id: string;
  name: string;
  slug: string;
  description: string;
  color: string; // hex
  public: boolean;
}

export interface Membership {
  userId: string;
  productId: string;
  role: Role;
}

export interface MembershipWithUser extends Membership {
  user: User;
}

export interface MembershipWithProduct extends Membership {
  product: Product;
}

// ------------------------------------------------------------------
// Symbols.
// ------------------------------------------------------------------

export type SymbolFormat = 'PDB' | 'dSYM' | 'Breakpad' | 'ELF';
export type SymbolArch = 'x86_64' | 'x86' | 'arm64';

export interface Symbol {
  id: string;
  productId: string;
  name: string;
  version: string;
  arch: SymbolArch | string;
  format: SymbolFormat | string;
  size: string;
  debugId: string;
  codeId: string;
  uploadedAt: string;
  uploadedBy: string; // userId
  referencedBy: number;
}

export interface SymbolQuery {
  search?: string;
  arch?: SymbolArch | 'all';
  format?: SymbolFormat | 'all';
  sort?: 'recent' | 'name' | 'size';
}

// ------------------------------------------------------------------
// Crash-list query.
// ------------------------------------------------------------------

export interface ListQuery {
  productId: string;
  version?: string;
  status?: Status;
  search?: string;
  sort?: 'count' | 'recent' | 'similarity' | 'version';
  limit?: number;
  offset?: number;
}

export interface ListResult {
  groups: CrashGroupSummary[];
  total: number;
  versions: string[];
}

// ------------------------------------------------------------------
// The adapter contract.
// ------------------------------------------------------------------

export interface GuardrailAdapter {
  // --- session ---
  signIn(email: string): Promise<User | null>;
  getUser(id: string): Promise<User | null>;

  // --- products ---
  listProducts(scope?: 'all' | 'mine' | 'public', userId?: string): Promise<Product[]>;
  getProduct(id: string): Promise<Product | null>;
  createProduct(spec: { name: string; slug?: string; description?: string }): Promise<Product>;
  updateProduct(id: string, patch: {
    name?: string;
    slug?: string;
    description?: string;
    color?: string;
  }): Promise<Product>;
  deleteProduct(id: string): Promise<void>;

  // --- users ---
  listUsers(): Promise<User[]>;
  createUser(spec: { email: string; name?: string }): Promise<User>;
  updateUser(id: string, patch: { email?: string; name?: string }): Promise<User>;
  deleteUser(id: string): Promise<void>;
  setAdmin(id: string, isAdmin: boolean): Promise<void>;

  // --- memberships ---
  listMembers(productId: string): Promise<MembershipWithUser[]>;
  membershipsFor(userId: string): Promise<MembershipWithProduct[]>;
  roleOf(userId: string, productId: string): Promise<Role | null>;
  grantAccess(spec: { userId: string; productId: string; role: Role }): Promise<void>;
  revokeAccess(spec: { userId: string; productId: string }): Promise<void>;

  // --- crashes ---
  listGroups(q: ListQuery): Promise<ListResult>;
  getGroup(id: string): Promise<CrashGroup | null>;
  /** Returns a single crash plus its parent group, or null. */
  getCrash(id: string): Promise<{ crash: Crash; group: CrashGroup } | null>;
  downloadAttachment(id: string): Promise<Response | null>;
  setStatus(id: string, status: Status): Promise<void>;
  addNote(id: string, body: string, author: string): Promise<Note>;
  mergeGroups(primaryId: string, mergedId: string): Promise<void>;

  // --- symbols ---
  listSymbols(productId: string, q?: SymbolQuery): Promise<Symbol[]>;
  uploadSymbol(productId: string, spec: {
    name: string;
    version?: string;
    arch?: string;
    format?: string;
    size?: string;
    uploadedBy: string;
  }): Promise<Symbol>;
  deleteSymbol(id: string): Promise<void>;
}
