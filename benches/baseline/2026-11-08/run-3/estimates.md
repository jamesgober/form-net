| Item | Input | Estimate | Measured |
|---|---|---|---|
| `Form::get` | the last of 10 fields | under 5 ns per field | 34.0 ns (3.4 ns per field) |
| `serialize_urlencoded` | one 1 MiB plain value | at least 1 GB/s | 572.54 µs (1.8 GB/s) |
| `serialize_urlencoded` | 1 MiB that all needs escaping | measure | 3.38 ms (0.3 GB/s) |
| `Boundary::new` | a 38-byte browser boundary | under 100 ns | 75.5 ns |
| `Boundary::generate` | OS randomness | under 1 µs | 155.5 ns |
| `FileName::sanitized` | a 24-byte name | about 100 ns | 168.1 ns |
| inspector calls | 100 text parts, no inspector | nothing | 31.09 µs |
| inspector calls | 100 text parts, three calls each | one monomorphized call per event | 31.93 µs (2.8 ns per call) |
| `SpillQuota::reserve` | reserve and release, one thread | 5 to 20 ns | 9.0 ns |
| typed extraction | sign-up, 8 fields | 10 to 30 ns per field over the parse | 652.9 ns against 564.1 ns parsed (11.1 ns per field) |
| wipe on drop | a 2 KiB `Secret` against a plain `String` | 50 to 100 ns for a 2 KiB form | 329.0 ns against 52.8 ns (276.2 ns for the wipe) |
| `encode_part_head` | a file part | under 100 ns | 89.7 ns |
| `MultipartWriter` | 1 MiB text, boundary scan off | scan 0.1 to 0.3 ns per byte | 672.20 µs (0.641 ns per byte) |
| `MultipartWriter` | 1 MiB text, boundary scan on | scan 0.1 to 0.3 ns per byte | 690.98 µs (0.659 ns per byte) |
| `Store::in_dir` | an existing private directory | 10 to 50 µs (`Store::temp`) | 29.54 µs |
| `Store::temp` | the per-user temporary directory | 10 to 50 µs | 244.40 µs |
| `StoredFile::persist` | upload, persist, delete (whole cycle) | persist under 50 µs | 645.88 µs |
| `UploadSession` + store | 100 MiB file in 64 KiB chunks, spilled | under 5% of one core at 1 GB/s | 146.80 ms (0.7 GB/s; 140% of one core at 1 GB/s) |
| `Driver` + `DiscardSink` | the same 100 MiB, CPU only | under 5% of one core at 1 GB/s | 7.40 ms (14.2 GB/s; 7.1% of one core at 1 GB/s) |
