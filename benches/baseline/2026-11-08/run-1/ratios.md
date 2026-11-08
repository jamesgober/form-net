| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 239 ns | form_urlencoded (collect) | 393 ns | 1.64x | pass |
| urlencoded pairs | login | pairs (no collect) | 188 ns | form_urlencoded (no collect) | 349 ns | 1.86x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 124 ns | form_urlencoded (no collect) | 345 ns | 2.79x | pass |
| urlencoded pairs | search | pairs (collect) | 161 ns | form_urlencoded (collect) | 157 ns | 0.97x | below |
| urlencoded pairs | search | pairs (no collect) | 113 ns | form_urlencoded (no collect) | 119 ns | 1.05x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 75 ns | form_urlencoded (no collect) | 127 ns | 1.69x | pass |
| urlencoded pairs | signup | pairs (collect) | 602 ns | form_urlencoded (collect) | 1.09 µs | 1.82x | pass |
| urlencoded pairs | signup | pairs (no collect) | 490 ns | form_urlencoded (no collect) | 1.02 µs | 2.09x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 334 ns | form_urlencoded (no collect) | 1.01 µs | 3.03x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 28.11 µs | form_urlencoded (collect) | 51.90 µs | 1.85x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 23.44 µs | form_urlencoded (no collect) | 47.17 µs | 2.01x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 33.76 µs | form_urlencoded (no collect) | 48.49 µs | 1.44x | pass |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 146.12 µs | form_urlencoded (collect) | 1.36 ms | 9.32x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 141.94 µs | form_urlencoded (no collect) | 1.36 ms | 9.59x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 151.80 µs | form_urlencoded (no collect) | 1.39 ms | 9.14x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 923.40 µs | form_urlencoded (collect) | 1.65 ms | 1.79x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 608.21 µs | form_urlencoded (no collect) | 1.59 ms | 2.61x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 727.23 µs | form_urlencoded (no collect) | 1.46 ms | 2.00x | pass |
| urlencoded struct | signup | form! | 674 ns | serde_urlencoded | 1.22 µs | 1.81x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 407 ns | form_urlencoded | 794 ns | 1.95x | pass |
| multipart stream | login, whole | MultipartParser | 1.11 µs | multer | 3.63 µs | 3.27x | pass |
| multipart stream | login, whole | MultipartParser | 1.15 µs | multer | 3.74 µs | 3.25x | pass |
| multipart stream | login, whole | MultipartParser | 1.08 µs | multer | 3.63 µs | 3.37x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 3.87 µs | multer | 6.21 µs | 1.61x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 4.15 µs | multer | 6.33 µs | 1.52x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 5.12 µs | multer | 8.50 µs | 1.66x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 25.32 µs | multer | 94.20 µs | 3.72x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 26.92 µs | multer | 101.38 µs | 3.77x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 26.05 µs | multer | 97.92 µs | 3.76x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 862.83 µs | multer | 1.92 ms | 2.22x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 930.94 µs | multer | 5.45 ms | 5.86x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 1.22 ms | multer | 5.36 ms | 4.41x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 30.96 µs | multer | 247.81 µs | 8.00x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 33.13 µs | multer | 561.67 µs | 16.95x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 75.70 µs | multer | 333.35 µs | 4.40x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 195.79 µs | multer | 264.04 µs | 1.35x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 200.22 µs | multer | 565.21 µs | 2.82x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 256.35 µs | multer | 318.40 µs | 1.24x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 173.20 µs | multer | 670.04 µs | 3.87x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 175.56 µs | multer | 839.27 µs | 4.78x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 220.80 µs | multer | 745.09 µs | 3.37x | pass |
| multipart buffered | login | parse_multipart | 1.36 µs | multipart 0.18 | 1.85 µs | 1.36x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 3.76 µs | multipart 0.18 | 6.47 µs | 1.72x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.60 ms | multipart 0.18 | 4.06 ms | 2.53x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 8.22 ms | multipart 0.18 SaveBuilder | 15.86 ms | 1.93x | pass |
