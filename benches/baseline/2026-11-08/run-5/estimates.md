| Item | Input | Estimate | Measured |
|---|---|---|---|
| `Form::get` | the last of 10 fields | under 5 ns per field | 33.6 ns (3.4 ns per field) |
| `serialize_urlencoded` | one 1 MiB plain value | at least 1 GB/s | 473.84 µs (2.2 GB/s) |
| `serialize_urlencoded` | 1 MiB that all needs escaping | measure | 3.42 ms (0.3 GB/s) |
| `Boundary::new` | a 38-byte browser boundary | under 100 ns | 77.7 ns |
| `Boundary::generate` | OS randomness | under 1 µs | 100.0 ns |
| `FileName::sanitized` | a 24-byte name | about 100 ns | 165.8 ns |
| inspector calls | 100 text parts, no inspector | nothing | 36.67 µs |
| inspector calls | 100 text parts, three calls each | one monomorphized call per event | 32.55 µs (0.0 ns per call) |
| `SpillQuota::reserve` | reserve and release, one thread | 5 to 20 ns | 9.6 ns |
| typed extraction | sign-up, 8 fields | 10 to 30 ns per field over the parse | 644.8 ns against 541.9 ns parsed (12.9 ns per field) |
| wipe on drop | a 2 KiB `Secret` against a plain `String` | 50 to 100 ns for a 2 KiB form | 332.2 ns against 51.1 ns (281.0 ns for the wipe) |
| `encode_part_head` | a file part | under 100 ns | 72.7 ns |
| `MultipartWriter` | 1 MiB text, boundary scan off | scan 0.1 to 0.3 ns per byte | 708.98 µs (0.676 ns per byte) |
| `MultipartWriter` | 1 MiB text, boundary scan on | scan 0.1 to 0.3 ns per byte | 670.80 µs (0.640 ns per byte) |
| `Store::in_dir` | an existing private directory | 10 to 50 µs (`Store::temp`) | 26.91 µs |
| `Store::temp` | the per-user temporary directory | 10 to 50 µs | 223.90 µs |
| `StoredFile::persist` | upload, persist, delete (whole cycle) | persist under 50 µs | 723.71 µs |
| `UploadSession` + store | 100 MiB file in 64 KiB chunks, spilled | under 5% of one core at 1 GB/s | 130.60 ms (0.8 GB/s; 125% of one core at 1 GB/s) |
| `Driver` + `DiscardSink` | the same 100 MiB, CPU only | under 5% of one core at 1 GB/s | 7.72 ms (13.6 GB/s; 7.4% of one core at 1 GB/s) |
