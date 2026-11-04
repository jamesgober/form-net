| Operation | Input | Target | Measured |
|---|---|---|---|
| `FormKind::detect` | browser `Content-Type` | under 100 ns | 114 ns |
| `raw_pairs` scan | 1 MiB, no escapes | at least 1.5 GB/s | 12.6 GB/s |
| `decode_into` | 1 MiB, every byte escaped | at least 700 MB/s | 3.9 GB/s |
| `parse_urlencoded` | 10 fields, 179 bytes | under 400 ns | 405 ns |
| `Signup::from_urlencoded` | 8 fields, 182 bytes | under 600 ns | 429 ns |
| `serialize_urlencoded` | 10 pairs | under 300 ns | 132 ns |
| `MultipartParser` | one 100 MiB file part, random bytes, 64 KiB chunks | at least 3 GB/s | 13.7 GB/s |
| `MultipartParser` | adversarial, 1 MiB all CR | at least 400 MB/s, linear | 52.9 GB/s |
| `MultipartParser` | adversarial, 1 MiB near-miss delimiters | at least 400 MB/s, linear | 8.7 GB/s |
| `MultipartParser` | 100 parts of 10 bytes | under 30 µs | 15.88 µs |
| `MultipartParser` | 360 B body, 3 text fields | under 1 µs | 669 ns |
| `parse_multipart` | 1 MiB, 2 files | within 10% of the raw parser | 21.66 µs against 21.46 µs raw (+1%) |
| split overhead | 65921 B body in 1-byte chunks | linear, under 50 ns per call | 13.3 ns per byte |
| idea 0.2: work factor | all CR against random data | add `max_work_factor` if 4x or more | 0.27x |
| idea 0.2: work factor | near-miss against random data | add `max_work_factor` if 4x or more | 1.64x |
| idea 0.2: work factor | near-miss in the middle against random data | add `max_work_factor` if 4x or more | 2.04x |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 0 | measure | 17.12 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 64 | measure | 16.33 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 512 | measure | 16.40 µs |
