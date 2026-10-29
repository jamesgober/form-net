# Platform notes

What form-net's upload storage (feature `fs`) does on each platform, and what a deployment should know. Everything here is done through safe `std` APIs; the crate has no `unsafe` and no platform crate.

---

## The private directory

`Store::temp()` creates `<temp dir>/form-net-<32 hex>`, the 32 hex characters from 16 bytes of the OS random source. The create fails if the path exists, so the store never adopts a directory someone else made, and a failing random source fails the call (no predictable fallback).

| Platform | Temp dir | Protection |
|---|---|---|
| Linux, macOS, BSD | `$TMPDIR`, else `/tmp` | the directory is created with mode `0700` |
| Windows | `%TMP%`, normally `C:\Users\<user>\AppData\Local\Temp` | inherits the per-user temp directory's ACL: the user, SYSTEM, and Administrators |

Windows ACLs cannot be set without `unsafe` or a third-party crate, so form-net relies on the inherited ACL of the per-user temp directory. A service account whose `%TMP%` points at a shared directory should use `Store::in_dir` with a directory created and locked down by the deployment.

`Store::in_dir(path)` uses a directory the caller made. It must be a directory and not a link (checked with `symlink_metadata`). Use it to put uploads on a volume of their own, so a full upload volume cannot stop the rest of the system.

## Spill files

Every spill file is `upload-<32 hex>.part`, created with `create_new` (never an existing path, never through a planted link):

| | Unix | Windows |
|---|---|---|
| permissions | `0600` | inherited from the directory |
| anonymous mode (default) | unlinked right after creation: the file has no name, and the kernel frees it when the handle closes or the process dies | opened with `FILE_FLAG_DELETE_ON_CLOSE`, `DELETE` access, and share mode 0: no other process can open it, and it is removed when the last handle closes, including when the process is killed |
| named mode | keeps its name until dropped or persisted; share mode lets the persist step link and remove it | the same, with share mode `READ \| DELETE` |
| hint | | `FILE_ATTRIBUTE_TEMPORARY`: kept in the cache where memory allows |

On Unix an anonymous file exists under its random name for the instant between creation and unlinking. A crash in exactly that instant leaves one empty or partial `upload-*.part`, which `sweep` removes.

Linux `O_TMPFILE` would create the file without any name and let `persist` link it instead of copying it, but it needs `libc` or `unsafe`; it is not used.

## Reading back

`StoredFile::reader` reads with positional reads (`read_at` on Unix, `seek_read` on Windows), so several readers of one file can exist at once without sharing a cursor. On Windows, `seek_read` does move the handle's position; form-net never relies on that position.

## Persist

`StoredFile::persist(dest)` never overwrites an existing file:

- Named mode on the same file system: a hard link to `dest` (which fails if `dest` exists), then the temporary name is removed. Constant time.
- Anonymous files, memory files, another file system, or a file system without hard links (FAT, some network shares): a copy into `dest` opened with `create_new`, from positional reads. A copy that fails part way removes the partial `dest`.

`dest` is the caller's: build it from your own id, or with `path_lib::join` and `FileName::sanitized` when the user's name must show.

## fsync

form-net does not call `fsync`. A persisted upload is in the page cache; after a power loss it may be missing or short. When an upload must survive power loss, after `persist`:

```rust
let f = std::fs::File::open(&dest)?;
f.sync_all()?;                            // the file's data and size
#[cfg(unix)]
std::fs::File::open(dest.parent().unwrap())?.sync_all()?; // the directory entry
```

On Windows `sync_all` calls `FlushFileBuffers`; directory entries are journaled by NTFS and need no separate flush.

## Disk budgets

`max_disk_bytes` caps one request; a `SpillBudget` (`SpillQuota` in-process) caps all requests through the stores that share it. Both are checked before every write. form-net does not query free space (that needs `unsafe` or a third-party crate); size the budget below the volume's free space, leaving room for everything else on it.

## After a crash

| Mode | What is left | What to do |
|---|---|---|
| anonymous (default) | nothing, but for the Unix creation instant above | run `sweep` at startup if you want that covered too |
| named | the spill files of every request in flight | `form_net::sweep(dir, Duration::from_secs(3600))` at startup, before accepting connections |

`sweep` removes only names form-net makes (`upload-` and 32 lowercase hex and `.part`), directly in the directory, never recursing and never following links. A store's own `form-net-*` directory under the temp dir is left behind by a crash; it is empty in anonymous mode, and the operating system's temp cleanup removes it.

## Counters

`Store::stats()` reports files and bytes in memory and on disk, files spilled, and `cleanup_failures` (a delete that failed during a drop, which cannot be returned as an error). Alert on `cleanup_failures` rising: it usually means another process holds a spill file open, or the volume went read-only.
