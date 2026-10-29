| Group | Benchmark | Median | form-net vs best competitor |
|---|---|---|---|
| multipart-buffered/avatar-64KiB | form-net parse_multipart | 2.45 µs | 1.64x |
|  | multipart 0.18 | 4.02 µs |  |
|  | multer one chunk | 4.19 µs |  |
| multipart-buffered/batch-20x1MiB | form-net parse_multipart | 1.33 ms | 2.32x |
|  | multer one chunk | 3.09 ms |  |
|  | multipart 0.18 | 3.29 ms |  |
| multipart-buffered/login | form-net parse_multipart | 915 ns | 1.35x |
|  | multipart 0.18 | 1.24 µs |  |
|  | multer one chunk | 2.51 µs |  |
| multipart-disk/document-10MiB | form-net UploadSession + Store::temp | 4.24 ms | 2.25x |
|  | multipart 0.18 SaveBuilder::temp | 9.55 ms |  |
| multipart-stream/100-parts/1500B | form-net MultipartParser | 15.9 µs | 4.16x |
|  | multer | 66 µs |  |
| multipart-stream/100-parts/8192B | form-net MultipartParser | 15.6 µs | 4.24x |
|  | multer | 66.1 µs |  |
| multipart-stream/100-parts/whole | form-net MultipartParser | 15.6 µs | 4.25x |
|  | multer | 66 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/1500B | form-net MultipartParser | 87.8 µs | 2.70x |
|  | multer | 237 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/65536B | form-net MultipartParser | 61.9 µs | 5.51x |
|  | multer | 341 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/8192B | form-net MultipartParser | 66.1 µs | 5.06x |
|  | multer | 335 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/whole | form-net MultipartParser | 60.2 µs | 2.80x |
|  | multer | 168 µs |  |
| multipart-stream/adversarial-near-miss-1MiB/1500B | form-net MultipartParser | 162 µs | 1.47x |
|  | multer | 237 µs |  |
| multipart-stream/adversarial-near-miss-1MiB/65536B | form-net MultipartParser | 129 µs | 2.56x |
|  | multer | 330 µs |  |
| multipart-stream/adversarial-near-miss-1MiB/8192B | form-net MultipartParser | 134 µs | 2.48x |
|  | multer | 333 µs |  |
| multipart-stream/adversarial-near-miss-1MiB/whole | form-net MultipartParser | 128 µs | 1.33x |
|  | multer | 170 µs |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/1500B | form-net MultipartParser | 167 µs | 2.65x |
|  | multer | 442 µs |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/65536B | form-net MultipartParser | 142 µs | 3.73x |
|  | multer | 529 µs |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/8192B | form-net MultipartParser | 146 µs | 3.60x |
|  | multer | 526 µs |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/whole | form-net MultipartParser | 141 µs | 2.71x |
|  | multer | 382 µs |  |
| multipart-stream/avatar-64KiB/1500B | form-net MultipartParser | 3.01 µs | 2.04x |
|  | multer | 6.13 µs |  |
| multipart-stream/avatar-64KiB/1B | form-net MultipartParser | 944 µs |  |
| multipart-stream/avatar-64KiB/65536B | form-net MultipartParser | 2.27 µs | 3.34x |
|  | multer | 7.57 µs |  |
| multipart-stream/avatar-64KiB/8192B | form-net MultipartParser | 2.39 µs | 3.78x |
|  | multer | 9.03 µs |  |
| multipart-stream/avatar-64KiB/whole | form-net MultipartParser | 2.22 µs | 1.90x |
|  | multer | 4.22 µs |  |
| multipart-stream/document-10MiB/1500B | form-net MultipartParser | 953 µs | 3.49x |
|  | multer | 3.32 ms |  |
| multipart-stream/document-10MiB/65536B | form-net MultipartParser | 768 µs | 4.72x |
|  | multer | 3.62 ms |  |
| multipart-stream/document-10MiB/8192B | form-net MultipartParser | 797 µs | 4.84x |
|  | multer | 3.85 ms |  |
| multipart-stream/document-10MiB/whole | form-net MultipartParser | 769 µs | 1.94x |
|  | multer | 1.49 ms |  |
| multipart-stream/login/whole | form-net MultipartParser | 713 ns | 3.61x |
|  | multer | 2.57 µs |  |
| urlencoded-pairs/1000-fields | form-net pairs | 19.9 µs | 1.31x |
|  | form_urlencoded parse | 26 µs |  |
|  | form-net raw_pairs+decode_into | 32.9 µs |  |
|  | form-net parse_urlencoded | 36.9 µs |  |
| urlencoded-pairs/1MiB-escaped | form-net raw_pairs+decode_into | 346 µs | 2.71x |
|  | form-net pairs | 372 µs |  |
|  | form-net parse_urlencoded | 639 µs |  |
|  | form_urlencoded parse | 939 µs |  |
| urlencoded-pairs/1MiB-plain | form-net pairs | 101 µs | 8.03x |
|  | form-net raw_pairs+decode_into | 205 µs |  |
|  | form-net parse_urlencoded | 518 µs |  |
|  | form_urlencoded parse | 811 µs |  |
| urlencoded-pairs/login | form-net raw_pairs+decode_into | 109 ns | 2.23x |
|  | form-net pairs | 159 ns |  |
|  | form-net parse_urlencoded | 162 ns |  |
|  | form_urlencoded parse | 242 ns |  |
| urlencoded-pairs/search | form-net raw_pairs+decode_into | 70.9 ns | 1.32x |
|  | form_urlencoded parse | 93.9 ns |  |
|  | form-net pairs | 106 ns |  |
|  | form-net parse_urlencoded | 118 ns |  |
| urlencoded-pairs/signup | form-net raw_pairs+decode_into | 285 ns | 2.38x |
|  | form-net parse_urlencoded | 361 ns |  |
|  | form-net pairs | 386 ns |  |
|  | form_urlencoded parse | 680 ns |  |
| urlencoded-serialize/10-pairs | form-net serialize_urlencoded | 140 ns | 2.51x |
|  | form_urlencoded Serializer | 351 ns |  |
|  | serde_urlencoded to_string | 659 ns |  |
| urlencoded-struct/signup | form-net form! | 454 ns | 1.68x |
|  | serde_urlencoded | 764 ns |  |
|  | serde_qs | 793 ns |  |
