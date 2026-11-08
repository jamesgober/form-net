| Item | Input | Estimate | Measured |
|---|---|---|---|
| `Form::get` | the last of 10 fields | under 5 ns per field | 32.6 ns (3.3 ns per field) |
| `serialize_urlencoded` | one 1 MiB plain value | at least 1 GB/s | 514.90 µs (2.0 GB/s) |
| `serialize_urlencoded` | 1 MiB that all needs escaping | measure | 3.32 ms (0.3 GB/s) |
| `Boundary::new` | a 38-byte browser boundary | under 100 ns | 67.4 ns |
| `Boundary::generate` | OS randomness | under 1 µs | 143.9 ns |
| `FileName::sanitized` | a 24-byte name | about 100 ns | 162.8 ns |
| inspector calls | 100 text parts, no inspector | nothing | 32.35 µs |
| inspector calls | 100 text parts, three calls each | one monomorphized call per event | 29.52 µs (0.0 ns per call) |
| `SpillQuota::reserve` | reserve and release, one thread | 5 to 20 ns | 9.6 ns |
| typed extraction | sign-up, 8 fields | 10 to 30 ns per field over the parse | 678.9 ns against 568.2 ns parsed (13.8 ns per field) |
| wipe on drop | a 2 KiB `Secret` against a plain `String` | 50 to 100 ns for a 2 KiB form | 325.3 ns against 50.9 ns (274.4 ns for the wipe) |
| `encode_part_head` | a file part | under 100 ns | 71.0 ns |
| `MultipartWriter` | 1 MiB text, boundary scan off | scan 0.1 to 0.3 ns per byte | 617.98 µs (0.589 ns per byte) |
| `MultipartWriter` | 1 MiB text, boundary scan on | scan 0.1 to 0.3 ns per byte | 718.55 µs (0.685 ns per byte) |
| `Store::in_dir` | an existing private directory | 10 to 50 µs (`Store::temp`) | 24.99 µs |
| `Store::temp` | the per-user temporary directory | 10 to 50 µs | 226.74 µs |
| `StoredFile::persist` | upload, persist, delete (whole cycle) | persist under 50 µs | 675.08 µs |
| `UploadSession` + store | 100 MiB file in 64 KiB chunks, spilled | under 5% of one core at 1 GB/s | 139.01 ms (0.8 GB/s; 133% of one core at 1 GB/s) |
| `Driver` + `DiscardSink` | the same 100 MiB, CPU only | under 5% of one core at 1 GB/s | 6.60 ms (15.9 GB/s; 6.3% of one core at 1 GB/s) |
