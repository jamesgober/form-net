| Operation | Input | Target | Measured |
|---|---|---|---|
| `FormKind::detect` | browser `Content-Type` | under 100 ns | 185 ns |
| `raw_pairs` scan | 1 MiB, no escapes | at least 1.5 GB/s | 8.9 GB/s |
| `decode_into` | 1 MiB, every byte escaped | at least 700 MB/s | 1.6 GB/s |
| `parse_urlencoded` | 10 fields, 179 bytes | under 400 ns | 666 ns |
| `Signup::from_urlencoded` | 8 fields, 182 bytes | under 600 ns | 641 ns |
| `serialize_urlencoded` | 10 pairs | under 300 ns | 294 ns |
| `MultipartParser` | one 100 MiB file part, random bytes, 64 KiB chunks | at least 3 GB/s | 10.5 GB/s |
| `MultipartParser` | adversarial, 1 MiB all CR | at least 400 MB/s, linear | 31.3 GB/s |
| `MultipartParser` | adversarial, 1 MiB near-miss delimiters | at least 400 MB/s, linear | 5.0 GB/s |
| `MultipartParser` | 100 parts of 10 bytes | under 30 µs | 26.20 µs |
| `MultipartParser` | 360 B body, 3 text fields | under 1 µs | 1.06 µs |
| `parse_multipart` | 1 MiB, 2 files | within 10% of the raw parser | 35.42 µs against 35.30 µs raw (+0%) |
| split overhead | 65921 B body in 1-byte chunks | linear, under 50 ns per call | 24.0 ns per byte |
| idea 0.2: work factor | all CR against random data | add `max_work_factor` if 4x or more | 0.34x |
| idea 0.2: work factor | near-miss against random data | add `max_work_factor` if 4x or more | 2.23x |
| idea 0.2: work factor | near-miss in the middle against random data | add `max_work_factor` if 4x or more | 1.85x |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 0 | measure | 28.35 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 64 | measure | 22.36 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 512 | measure | 25.67 µs |
| idea 0.8: refusal digest | `record`, form-net's own code | about 1 ns | 5 ns |
| idea 0.8: refusal digest | `record`, the eighth named inspector code | about 1 ns | 10 ns |
| idea 0.8: refusal digest | `record`, an unnamed inspector code | about 1 ns | 6 ns |
| `TokenCheck` | login form, no inspector | measure | 396 ns |
| `TokenCheck` | login form, the check (verify excluded) | within 5% of none | 414 ns |
