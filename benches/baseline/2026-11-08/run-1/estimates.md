| Item | Input | Estimate | Measured |
|---|---|---|---|
| `Form::get` | the last of 10 fields | under 5 ns per field | 34.6 ns (3.5 ns per field) |
| `serialize_urlencoded` | one 1 MiB plain value | at least 1 GB/s | 479.00 µs (2.2 GB/s) |
| `serialize_urlencoded` | 1 MiB that all needs escaping | measure | 2.84 ms (0.4 GB/s) |
| `Boundary::new` | a 38-byte browser boundary | under 100 ns | 69.6 ns |
| `Boundary::generate` | OS randomness | under 1 µs | 148.7 ns |
| `FileName::sanitized` | a 24-byte name | about 100 ns | 160.4 ns |
| inspector calls | 100 text parts, no inspector | nothing | 29.64 µs |
| inspector calls | 100 text parts, three calls each | one monomorphized call per event | 33.56 µs (13.1 ns per call) |
| `SpillQuota::reserve` | reserve and release, one thread | 5 to 20 ns | 9.6 ns |
| typed extraction | sign-up, 8 fields | 10 to 30 ns per field over the parse | 656.9 ns against 533.3 ns parsed (15.5 ns per field) |
| wipe on drop | a 2 KiB `Secret` against a plain `String` | 50 to 100 ns for a 2 KiB form | 377.0 ns against 49.7 ns (327.3 ns for the wipe) |
| `encode_part_head` | a file part | under 100 ns | 76.2 ns |
| `MultipartWriter` | 1 MiB text, boundary scan off | scan 0.1 to 0.3 ns per byte | 629.48 µs (0.600 ns per byte) |
| `MultipartWriter` | 1 MiB text, boundary scan on | scan 0.1 to 0.3 ns per byte | 699.42 µs (0.667 ns per byte) |
| `Store::in_dir` | an existing private directory | 10 to 50 µs (`Store::temp`) | 25.95 µs |
| `Store::temp` | the per-user temporary directory | 10 to 50 µs | 359.50 µs |
| `StoredFile::persist` | upload, persist, delete (whole cycle) | persist under 50 µs | 696.10 µs |
| `UploadSession` + store | 100 MiB file in 64 KiB chunks, spilled | under 5% of one core at 1 GB/s | 135.91 ms (0.8 GB/s; 130% of one core at 1 GB/s) |
| `Driver` + `DiscardSink` | the same 100 MiB, CPU only | under 5% of one core at 1 GB/s | 5.79 ms (18.1 GB/s; 5.5% of one core at 1 GB/s) |
