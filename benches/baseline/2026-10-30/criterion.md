| Group | Benchmark | Median | form-net vs best competitor |
|---|---|---|---|
| multipart-buffered/avatar-64KiB | form-net parse_multipart | 2.71 µs | 1.46x |
|  | multipart 0.18 | 3.97 µs |  |
|  | multer one chunk | 4.27 µs |  |
| multipart-buffered/batch-20x1MiB | form-net parse_multipart | 1.32 ms | 2.40x |
|  | multipart 0.18 | 3.16 ms |  |
|  | multer one chunk | 3.29 ms |  |
| multipart-buffered/login | form-net parse_multipart | 892 ns | 1.39x |
|  | multipart 0.18 | 1.24 µs |  |
|  | multer one chunk | 2.49 µs |  |
| multipart-disk/document-10MiB | form-net UploadSession + Store::temp | 4.1 ms | 2.28x |
|  | multipart 0.18 SaveBuilder::temp | 9.34 ms |  |
| multipart-stream/100-parts/1500B | form-net MultipartParser | 16 µs | 4.06x |
|  | multer | 64.8 µs |  |
| multipart-stream/100-parts/8192B | form-net MultipartParser | 15.9 µs | 4.06x |
|  | multer | 64.4 µs |  |
| multipart-stream/100-parts/whole | form-net MultipartParser | 15.8 µs | 4.07x |
|  | multer | 64.3 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/1500B | form-net MultipartParser | 46.5 µs | 5.38x |
|  | multer | 251 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/65536B | form-net MultipartParser | 26.4 µs | 14.12x |
|  | multer | 373 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/8192B | form-net MultipartParser | 30.8 µs | 12.25x |
|  | multer | 378 µs |  |
| multipart-stream/adversarial-all-CR-1MiB/whole | form-net MultipartParser | 25.6 µs | 7.56x |
|  | multer | 194 µs |  |
| multipart-stream/adversarial-near-miss-1MiB/1500B | form-net MultipartParser | 152 µs | 1.64x |
|  | multer | 249 µs |  |
| multipart-stream/adversarial-near-miss-1MiB/65536B | form-net MultipartParser | 124 µs | 3.10x |
|  | multer | 383 µs |  |
| multipart-stream/adversarial-near-miss-1MiB/8192B | form-net MultipartParser | 129 µs | 2.94x |
|  | multer | 379 µs |  |
| multipart-stream/adversarial-near-miss-1MiB/whole | form-net MultipartParser | 124 µs | 1.57x |
|  | multer | 194 µs |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/1500B | form-net MultipartParser | 176 µs | 2.55x |
|  | multer | 449 µs |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/65536B | form-net MultipartParser | 155 µs | 3.70x |
|  | multer | 571 µs |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/8192B | form-net MultipartParser | 157 µs | 3.64x |
|  | multer | 572 µs |  |
| multipart-stream/adversarial-near-miss-middle-1MiB/whole | form-net MultipartParser | 152 µs | 2.64x |
|  | multer | 402 µs |  |
| multipart-stream/avatar-64KiB/1500B | form-net MultipartParser | 3.25 µs | 1.92x |
|  | multer | 6.24 µs |  |
| multipart-stream/avatar-64KiB/1B | form-net MultipartParser | 909 µs |  |
| multipart-stream/avatar-64KiB/65536B | form-net MultipartParser | 2.52 µs | 3.22x |
|  | multer | 8.11 µs |  |
| multipart-stream/avatar-64KiB/8192B | form-net MultipartParser | 2.64 µs | 1.89x |
|  | multer | 4.98 µs |  |
| multipart-stream/avatar-64KiB/whole | form-net MultipartParser | 2.5 µs | 1.66x |
|  | multer | 4.14 µs |  |
| multipart-stream/document-10MiB/1500B | form-net MultipartParser | 973 µs | 3.77x |
|  | multer | 3.67 ms |  |
| multipart-stream/document-10MiB/65536B | form-net MultipartParser | 755 µs | 5.31x |
|  | multer | 4.01 ms |  |
| multipart-stream/document-10MiB/8192B | form-net MultipartParser | 791 µs | 5.03x |
|  | multer | 3.98 ms |  |
| multipart-stream/document-10MiB/whole | form-net MultipartParser | 745 µs | 2.14x |
|  | multer | 1.59 ms |  |
| multipart-stream/login/whole | form-net MultipartParser | 667 ns | 3.49x |
|  | multer | 2.33 µs |  |
| urlencoded-pairs/1000-fields | form-net pairs | 19.6 µs | 1.42x |
|  | form_urlencoded parse | 27.8 µs |  |
|  | form-net raw_pairs+decode_into | 32.8 µs |  |
|  | form-net parse_urlencoded | 36.6 µs |  |
| urlencoded-pairs/1MiB-escaped | form-net raw_pairs+decode_into | 329 µs | 2.71x |
|  | form-net pairs | 407 µs |  |
|  | form-net parse_urlencoded | 571 µs |  |
|  | form_urlencoded parse | 893 µs |  |
| urlencoded-pairs/1MiB-plain | form-net pairs | 98.5 µs | 8.09x |
|  | form-net raw_pairs+decode_into | 174 µs |  |
|  | form-net parse_urlencoded | 476 µs |  |
|  | form_urlencoded parse | 797 µs |  |
| urlencoded-pairs/login | form-net raw_pairs+decode_into | 108 ns | 2.29x |
|  | form-net pairs | 159 ns |  |
|  | form-net parse_urlencoded | 168 ns |  |
|  | form_urlencoded parse | 248 ns |  |
| urlencoded-pairs/search | form-net raw_pairs+decode_into | 68.5 ns | 1.36x |
|  | form_urlencoded parse | 93.4 ns |  |
|  | form-net pairs | 106 ns |  |
|  | form-net parse_urlencoded | 121 ns |  |
| urlencoded-pairs/signup | form-net raw_pairs+decode_into | 294 ns | 2.36x |
|  | form-net parse_urlencoded | 384 ns |  |
|  | form-net pairs | 399 ns |  |
|  | form_urlencoded parse | 694 ns |  |
| urlencoded-serialize/10-pairs | form-net serialize_urlencoded | 139 ns | 2.42x |
|  | form_urlencoded Serializer | 336 ns |  |
|  | serde_urlencoded to_string | 638 ns |  |
| urlencoded-struct/signup | form-net form! | 456 ns | 1.69x |
|  | serde_urlencoded | 772 ns |  |
|  | serde_qs | 796 ns |  |
