// Web spike (Stage 12, leaf 0Mfl87): the .wasm handler's C-ABI exports
// (crates/web/src/ffi.rs) wrapped for JavaScript, over any WASI
// implementation. The page uses @bjorn3/browser_wasi_shim with an in-memory
// filesystem; bench.mjs uses the same shim, or Node's own node:wasi.
// Spike code: disposable.

const enc = new TextEncoder();
const dec = new TextDecoder();
const FAILED = 0xffffffffffffffffn;

/// One instantiated handler module. `instance` is initialized already.
export class Gbw {
  constructor(instance) {
    this.x = instance.exports;
  }

  #put(bytes) {
    const ptr = this.x.gbw_alloc(bytes.length);
    new Uint8Array(this.x.memory.buffer, ptr, bytes.length).set(bytes);
    return [ptr, bytes.length];
  }

  // A wasm i64 reaches JS as a signed BigInt; the result is `ptr << 32 | len`.
  #take(signed) {
    const packed = BigInt.asUintN(64, signed);
    if (packed === FAILED) return null;
    const ptr = Number(packed >> 32n);
    const len = Number(packed & 0xffffffffn);
    const out = new Uint8Array(this.x.memory.buffer, ptr, len).slice();
    this.x.gbw_free(ptr, len);
    return out;
  }

  #call(fn, ...buffers) {
    const args = buffers.flatMap((b) => this.#put(b));
    const out = this.#take(fn(...args));
    for (let i = 0; i < args.length; i += 2) this.x.gbw_free(args[i], args[i + 1]);
    return out;
  }

  /// Writes bytes to a path in the module's filesystem.
  putFile(path, bytes) {
    const err = dec.decode(this.#call(this.x.gbw_put_file, enc.encode(path), bytes));
    if (err) throw new Error(`put_file: ${err}`);
  }

  /// Reads a path from the module's filesystem, or null.
  getFile(path) {
    return this.#call(this.x.gbw_get_file, enc.encode(path));
  }

  /// Opens the handler over `{dir, session, rom, model?}`.
  open(spec) {
    const out = JSON.parse(dec.decode(this.#call(this.x.gbw_open, enc.encode(JSON.stringify(spec)))));
    if (!out.ok) throw new Error(`open: ${JSON.stringify(out.error)}`);
  }

  /// Answers one protocol request (an object) with the response object.
  request(req) {
    return JSON.parse(this.requestLine(JSON.stringify(req)));
  }

  /// Runs one `gbcli` invocation in-process (argv without the program
  /// name, in `cwd`): `{code, out, err}`.
  cli(args, cwd = "/work") {
    return JSON.parse(dec.decode(this.#call(this.x.gbw_cli, enc.encode(JSON.stringify({ args, cwd })))));
  }

  /// Answers one request line with the response line, as text.
  requestLine(line) {
    const out = this.#call(this.x.gbw_request, enc.encode(line));
    if (!out) throw new Error("request: no handler");
    return dec.decode(out);
  }
}

// WASI `fstflags` bits and the `filestat` timestamp offsets (the spec's).
const ATIM = 1, ATIM_NOW = 2, MTIM = 4, MTIM_NOW = 8;
const FILESTAT_ATIM = 40, FILESTAT_MTIM = 48, FILESTAT_CTIM = 56;
const nowNs = () => BigInt(Date.now()) * 1000000n;

/// Gives the shim's in-memory files modification times. The bundle store
/// needs them: a session's lock file's mtime records its last use (and a
/// new lock file is dated to the epoch with `fd_filestat_set_times`), and a
/// ROM's stamp compares mtimes. browser_wasi_shim 0.4.2 keeps no times,
/// answers `fd_filestat_set_times` with ENOTSUP (which fails `open`), and
/// writes `filestat`'s timestamps at offsets 38/46/52 instead of 40/48/56;
/// they are always zero there, so only the overwrite below matters.
function addTimes(wasi) {
  const w = wasi.wasiImport;
  const memory = () => new DataView(wasi.inst.exports.memory.buffer);
  const inodeOf = (fd) => fd?.file ?? fd?.dir ?? null;
  const touch = (inode) => {
    if (inode) inode.mtim = nowNs();
  };
  const writeTimes = (ptr, inode) => {
    const t = inode?.mtim ?? 0n;
    const view = memory();
    view.setBigUint64(ptr + FILESTAT_ATIM, t, true);
    view.setBigUint64(ptr + FILESTAT_MTIM, t, true);
    view.setBigUint64(ptr + FILESTAT_CTIM, t, true);
  };
  const wrap = (name, after) => {
    const orig = w[name];
    w[name] = (...args) => {
      const ret = orig(...args);
      if (!ret) after(...args);
      return ret;
    };
  };
  wrap("fd_filestat_get", (fd, ptr) => writeTimes(ptr, inodeOf(wasi.fds[fd])));
  wrap("path_filestat_get", (fd, _flags, pathPtr, pathLen, ptr) => {
    const path = new TextDecoder().decode(new Uint8Array(wasi.inst.exports.memory.buffer, pathPtr, pathLen));
    writeTimes(ptr, wasi.fds[fd].path_lookup(path, 0).inode_obj);
  });
  wrap("path_open", (_fd, _dirflags, _p, _l, oflags, _rb, _ri, _ff, fdPtr) => {
    const inode = inodeOf(wasi.fds[memory().getUint32(fdPtr, true)]);
    if (inode && (inode.mtim === undefined || oflags & 8 /* TRUNC */)) touch(inode);
  });
  for (const name of ["fd_write", "fd_pwrite", "fd_filestat_set_size", "fd_allocate"]) {
    wrap(name, (fd) => touch(inodeOf(wasi.fds[fd])));
  }
  w.fd_filestat_set_times = (fd, atim, mtim, flags) => {
    const inode = inodeOf(wasi.fds[fd]);
    if (!inode) return 8; // EBADF
    if (flags & MTIM_NOW) inode.mtim = nowNs();
    else if (flags & MTIM) inode.mtim = BigInt.asUintN(64, mtim);
    void atim, ATIM, ATIM_NOW; // one time per inode: access time is mtime
    return 0;
  };
}

/// Instantiates the module with @bjorn3/browser_wasi_shim, `/work` preopened
/// as an empty in-memory directory. `shim` is the shim's module namespace,
/// passed in so the browser and Node can each import it their own way.
export async function instantiateWithShim(shim, module, log = () => {}) {
  const fds = [
    new shim.OpenFile(new shim.File([])),
    shim.ConsoleStdout.lineBuffered((m) => log(`[stdout] ${m}`)),
    shim.ConsoleStdout.lineBuffered((m) => log(`[stderr] ${m}`)),
    new shim.PreopenDirectory("/work", []),
  ];
  const wasi = new shim.WASI(["gbcli-web"], [], fds, { debug: false });
  addTimes(wasi);
  const instance = await WebAssembly.instantiate(module, {
    wasi_snapshot_preview1: wasi.wasiImport,
  });
  wasi.initialize(instance);
  return { gbw: new Gbw(instance), fds };
}
