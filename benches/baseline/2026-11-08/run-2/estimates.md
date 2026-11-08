| Item | Input | Estimate | Measured |
|---|---|---|---|
| `Form::get` | the last of 10 fields | under 5 ns per field | 33.3 ns (3.3 ns per field) |
| `serialize_urlencoded` | one 1 MiB plain value | at least 1 GB/s | 496.84 µs (2.1 GB/s) |
| `serialize_urlencoded` | 1 MiB that all needs escaping | measure | 3.19 ms (0.3 GB/s) |
| `Boundary::new` | a 38-byte browser boundary | under 100 ns | 79.7 ns |
| `Boundary::generate` | OS randomness | under 1 µs | 161.1 ns |
| `FileName::sanitized` | a 24-byte name | about 100 ns | 167.7 ns |
| inspector calls | 100 text parts, no inspector | nothing | 32.68 µs |
| inspector calls | 100 text parts, three calls each | one monomorphized call per event | 32.20 µs (0.0 ns per call) |
| `SpillQuota::reserve` | reserve and release, one thread | 5 to 20 ns | 9.6 ns |
| typed extraction | sign-up, 8 fields | 10 to 30 ns per field over the parse | 625.7 ns against 571.8 ns parsed (6.7 ns per field) |
| wipe on drop | a 2 KiB `Secret` against a plain `String` | 50 to 100 ns for a 2 KiB form | 325.3 ns against 48.3 ns (277.0 ns for the wipe) |
| `encode_part_head` | a file part | under 100 ns | 76.8 ns |
| `MultipartWriter` | 1 MiB text, boundary scan off | scan 0.1 to 0.3 ns per byte | 658.12 µs (0.628 ns per byte) |
| `MultipartWriter` | 1 MiB text, boundary scan on | scan 0.1 to 0.3 ns per byte | 703.85 µs (0.671 ns per byte) |
| `Store::in_dir` | an existing private directory | 10 to 50 µs (`Store::temp`) | 25.68 µs |
| `Store::temp` | the per-user temporary directory | 10 to 50 µs | 263.30 µs |
| `StoredFile::persist` | upload, persist, delete (whole cycle) | persist under 50 µs | 794.05 µs |
| `UploadSession` + store | 100 MiB file in 64 KiB chunks, spilled | under 5% of one core at 1 GB/s | 129.23 ms (0.8 GB/s; 123% of one core at 1 GB/s) |
| `Driver` + `DiscardSink` | the same 100 MiB, CPU only | under 5% of one core at 1 GB/s | 7.95 ms (13.2 GB/s; 7.6% of one core at 1 GB/s) |
