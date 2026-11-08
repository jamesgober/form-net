| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 242 ns | form_urlencoded (collect) | 393 ns | 1.63x | pass |
| urlencoded pairs | login | pairs (no collect) | 197 ns | form_urlencoded (no collect) | 369 ns | 1.83x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 126 ns | form_urlencoded (no collect) | 349 ns | 2.79x | pass |
| urlencoded pairs | search | pairs (collect) | 164 ns | form_urlencoded (collect) | 157 ns | 0.94x | below |
| urlencoded pairs | search | pairs (no collect) | 113 ns | form_urlencoded (no collect) | 119 ns | 1.05x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 80 ns | form_urlencoded (no collect) | 121 ns | 1.55x | pass |
| urlencoded pairs | signup | pairs (collect) | 602 ns | form_urlencoded (collect) | 1.09 µs | 1.88x | pass |
| urlencoded pairs | signup | pairs (no collect) | 504 ns | form_urlencoded (no collect) | 1.02 µs | 2.04x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 346 ns | form_urlencoded (no collect) | 1.01 µs | 2.93x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 27.31 µs | form_urlencoded (collect) | 51.33 µs | 1.86x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 23.44 µs | form_urlencoded (no collect) | 47.49 µs | 2.06x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 32.76 µs | form_urlencoded (no collect) | 48.50 µs | 1.49x | pass |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 146.12 µs | form_urlencoded (collect) | 1.49 ms | 10.01x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 143.24 µs | form_urlencoded (no collect) | 1.40 ms | 9.81x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 145.04 µs | form_urlencoded (no collect) | 1.39 ms | 9.68x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 800.85 µs | form_urlencoded (collect) | 1.71 ms | 2.13x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 641.60 µs | form_urlencoded (no collect) | 1.56 ms | 2.41x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 702.65 µs | form_urlencoded (no collect) | 1.49 ms | 2.13x | pass |
| urlencoded struct | signup | form! | 641 ns | serde_urlencoded | 1.17 µs | 1.82x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 407 ns | form_urlencoded | 794 ns | 1.93x | pass |
| multipart stream | login, whole | MultipartParser | 1.11 µs | multer | 3.73 µs | 3.40x | pass |
| multipart stream | login, whole | MultipartParser | 1.12 µs | multer | 3.74 µs | 3.32x | pass |
| multipart stream | login, whole | MultipartParser | 1.09 µs | multer | 3.75 µs | 3.37x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 3.64 µs | multer | 6.87 µs | 1.90x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 3.73 µs | multer | 6.41 µs | 1.79x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 5.15 µs | multer | 10.69 µs | 1.92x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 25.61 µs | multer | 96.08 µs | 3.72x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 25.59 µs | multer | 101.38 µs | 3.85x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 26.05 µs | multer | 97.92 µs | 3.75x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 863.98 µs | multer | 3.27 ms | 3.79x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 909.70 µs | multer | 7.95 ms | 8.74x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 1.22 ms | multer | 6.36 ms | 5.35x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 31.08 µs | multer | 282.21 µs | 8.63x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 34.60 µs | multer | 561.75 µs | 16.50x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 75.70 µs | multer | 333.35 µs | 4.62x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 195.79 µs | multer | 266.65 µs | 1.39x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 202.56 µs | multer | 565.21 µs | 2.49x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 248.12 µs | multer | 353.07 µs | 1.46x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 174.19 µs | multer | 675.55 µs | 3.90x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 178.88 µs | multer | 927.20 µs | 4.94x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 218.82 µs | multer | 745.09 µs | 3.37x | pass |
| multipart buffered | login | parse_multipart | 1.36 µs | multipart 0.18 | 1.86 µs | 1.37x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 4.04 µs | multipart 0.18 | 6.70 µs | 1.72x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.61 ms | multipart 0.18 | 4.11 ms | 2.53x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 7.19 ms | multipart 0.18 SaveBuilder | 17.08 ms | 2.37x | pass |
