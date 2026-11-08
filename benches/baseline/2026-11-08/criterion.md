| Group | Benchmark | Median | form-net vs best competitor |
|---|---|---|---|
| multipart-buffered/avatar-64KiB | form-net parse_multipart | 7.37 µs | 1.64x |
|  | multer one chunk | 12.1 µs |  |
|  | multipart 0.18 | 13.9 µs |  |
| multipart-buffered/batch-20x1MiB | form-net parse_multipart | 2.8 ms | 3.58x |
|  | multipart 0.18 | 10 ms |  |
|  | multer one chunk | 16.4 ms |  |
| multipart-buffered/login | form-net parse_multipart | 2.6 µs | 1.62x |
|  | multipart 0.18 | 4.2 µs |  |
|  | multer one chunk | 6.82 µs |  |
| multipart-disk/document-10MiB | form-net UploadSession + Store::temp | 14.8 ms | 2.20x |
|  | multipart 0.18 SaveBuilder::temp | 32.6 ms |  |
| multipart-stream/100-parts/1500B | form-net MultipartParser | 112 µs | 3.73x |
|  | multer | 418 µs |  |
| multipart-stream/100-parts/8192B | form-net MultipartParser | 62.9 µs | 3.75x |
|  | multer | 236 µs |  |
| multipart-stream/100-parts/whole | form-net MultipartParser | 59.8 µs | 3.20x |
|  | multer | 191 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/1500B | form-net MultipartParser | 171 µs | 4.65x |
|  | multer | 795 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/65536B | form-net MultipartParser | 73.4 µs | 25.51x |
|  | multer | 1.87 ms |  |
| multipart-stream/adversarial-all-CR-1MiB/8192B | form-net MultipartParser | 83.1 µs | 13.81x |
|  | multer | 1.15 ms |  |
| multipart-stream/adversarial-all-CR-1MiB/whole | form-net MultipartParser | 276 µs | 11.04x |
|  | multer | 3.04 ms |  |
| multipart-stream/adversarial-near-miss-1MiB/1500B | form-net MultipartParser | 454 µs | 1.78x |
|  | multer | 808 µs |  |
| multipart-stream/adversarial-near-miss-1MiB/65536B | form-net MultipartParser | 533 µs | 4.78x |
|  | multer | 2.55 ms |  |
| multipart-stream/adversarial-near-miss-1MiB/8192B | form-net MultipartParser | 534 µs | 2.29x |
|  | multer | 1.22 ms |  |
| multipart-stream/adversarial-near-miss-1MiB/whole | form-net MultipartParser | 369 µs | 1.97x |
|  | multer | 727 µs |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/1500B | form-net MultipartParser | 384 µs | 3.82x |
|  | multer | 1.47 ms |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/65536B | form-net MultipartParser | 375 µs | 5.34x |
|  | multer | 2 ms |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/8192B | form-net MultipartParser | 327 µs | 6.02x |
|  | multer | 1.97 ms |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/whole | form-net MultipartParser | 499 µs | 3.19x |
|  | multer | 1.59 ms |  |
| multipart-stream/avatar-64KiB/1500B | form-net MultipartParser | 12.8 µs | 1.74x |
|  | multer | 22.2 µs |  |
| multipart-stream/avatar-64KiB/1B | form-net MultipartParser | 3.05 ms |  |
| multipart-stream/avatar-64KiB/65536B | form-net MultipartParser | 6.48 µs | 5.16x |
|  | multer | 33.4 µs |  |
| multipart-stream/avatar-64KiB/8192B | form-net MultipartParser | 16.6 µs | 1.03x |
|  | multer | 17.2 µs |  |
| multipart-stream/avatar-64KiB/whole | form-net MultipartParser | 9.9 µs | 1.44x |
|  | multer | 14.2 µs |  |
| multipart-stream/document-10MiB/1500B | form-net MultipartParser | 4.02 ms | 3.88x |
|  | multer | 15.6 ms |  |
| multipart-stream/document-10MiB/65536B | form-net MultipartParser | 1.86 ms | 14.58x |
|  | multer | 27.1 ms |  |
| multipart-stream/document-10MiB/8192B | form-net MultipartParser | 2.8 ms | 6.45x |
|  | multer | 18 ms |  |
| multipart-stream/document-10MiB/whole | form-net MultipartParser | 8.1 ms | 3.51x |
|  | multer | 28.5 ms |  |
| multipart-stream/login/whole | form-net MultipartParser | 7.46 µs | 1.65x |
|  | multer | 12.3 µs |  |
| urlencoded-pairs/1000-fields | form-net pairs | 54.1 µs | 1.70x |
|  | form-net raw_pairs+decode_into | 65.1 µs |  |
|  | form_urlencoded parse | 92 µs |  |
|  | form-net parse_urlencoded | 136 µs |  |
| urlencoded-pairs/1MiB-escaped | form-net raw_pairs+decode_into | 1.47 ms | 4.21x |
|  | form-net pairs | 1.55 ms |  |
|  | form-net parse_urlencoded | 2.59 ms |  |
|  | form_urlencoded parse | 6.19 ms |  |
| urlencoded-pairs/1MiB-plain | form-net pairs | 321 µs | 8.13x |
|  | form-net raw_pairs+decode_into | 331 µs |  |
|  | form-net parse_urlencoded | 2.39 ms |  |
|  | form_urlencoded parse | 2.61 ms |  |
| urlencoded-pairs/login | form-net raw_pairs+decode_into | 244 ns | 2.45x |
|  | form-net pairs | 397 ns |  |
|  | form-net parse_urlencoded | 519 ns |  |
|  | form_urlencoded parse | 600 ns |  |
| urlencoded-pairs/search | form-net raw_pairs+decode_into | 139 ns | 1.69x |
|  | form_urlencoded parse | 235 ns |  |
|  | form-net pairs | 292 ns |  |
|  | form-net parse_urlencoded | 851 ns |  |
| urlencoded-pairs/signup | form-net raw_pairs+decode_into | 728 ns | 2.46x |
|  | form-net pairs | 1.18 µs |  |
|  | form_urlencoded parse | 1.79 µs |  |
|  | form-net parse_urlencoded | 2.26 µs |  |
| urlencoded-serialize/10-pairs | form-net serialize_urlencoded | 517 ns | 3.72x |
|  | form_urlencoded Serializer | 1.92 µs |  |
|  | serde_urlencoded to_string | 3.68 µs |  |
| urlencoded-struct/signup | form-net form! | 1.38 µs | 1.52x |
|  | serde_urlencoded | 2.1 µs |  |
|  | serde_qs | 3.34 µs |  |
