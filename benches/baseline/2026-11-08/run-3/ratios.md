| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 248 ns | form_urlencoded (collect) | 397 ns | 1.60x | pass |
| urlencoded pairs | login | pairs (no collect) | 233 ns | form_urlencoded (no collect) | 370 ns | 1.59x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 129 ns | form_urlencoded (no collect) | 347 ns | 2.69x | pass |
| urlencoded pairs | search | pairs (collect) | 173 ns | form_urlencoded (collect) | 163 ns | 0.94x | below |
| urlencoded pairs | search | pairs (no collect) | 116 ns | form_urlencoded (no collect) | 119 ns | 1.02x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 85 ns | form_urlencoded (no collect) | 112 ns | 1.32x | pass |
| urlencoded pairs | signup | pairs (collect) | 613 ns | form_urlencoded (collect) | 1.18 µs | 1.93x | pass |
| urlencoded pairs | signup | pairs (no collect) | 504 ns | form_urlencoded (no collect) | 1.05 µs | 2.08x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 333 ns | form_urlencoded (no collect) | 1.00 µs | 3.01x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 26.37 µs | form_urlencoded (collect) | 51.33 µs | 1.95x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 24.31 µs | form_urlencoded (no collect) | 49.64 µs | 2.04x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 32.76 µs | form_urlencoded (no collect) | 49.80 µs | 1.52x | pass |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 154.26 µs | form_urlencoded (collect) | 1.49 ms | 9.64x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 147.04 µs | form_urlencoded (no collect) | 1.39 ms | 9.43x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 145.04 µs | form_urlencoded (no collect) | 1.42 ms | 9.79x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 800.85 µs | form_urlencoded (collect) | 1.71 ms | 2.13x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 671.70 µs | form_urlencoded (no collect) | 1.56 ms | 2.32x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 702.65 µs | form_urlencoded (no collect) | 1.57 ms | 2.23x | pass |
| urlencoded struct | signup | form! | 641 ns | serde_urlencoded | 1.17 µs | 1.82x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 410 ns | form_urlencoded | 816 ns | 1.99x | pass |
| multipart stream | login, whole | MultipartParser | 1.09 µs | multer | 3.69 µs | 3.40x | pass |
| multipart stream | login, whole | MultipartParser | 1.12 µs | multer | 3.75 µs | 3.36x | pass |
| multipart stream | login, whole | MultipartParser | 1.15 µs | multer | 3.77 µs | 3.27x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 3.64 µs | multer | 7.68 µs | 2.11x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 3.73 µs | multer | 8.30 µs | 2.23x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 5.15 µs | multer | 11.35 µs | 2.20x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 26.57 µs | multer | 96.08 µs | 3.62x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 25.35 µs | multer | 97.52 µs | 3.85x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 25.92 µs | multer | 96.36 µs | 3.72x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 864.65 µs | multer | 3.33 ms | 3.85x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 909.70 µs | multer | 7.95 ms | 8.74x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 1.25 ms | multer | 7.61 ms | 6.07x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 32.68 µs | multer | 282.21 µs | 8.63x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 34.78 µs | multer | 604.96 µs | 17.39x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 71.75 µs | multer | 325.68 µs | 4.54x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 196.55 µs | multer | 266.65 µs | 1.36x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 202.56 µs | multer | 501.11 µs | 2.47x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 246.49 µs | multer | 271.49 µs | 1.10x | below |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 176.22 µs | multer | 648.61 µs | 3.68x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 178.88 µs | multer | 876.45 µs | 4.90x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 211.34 µs | multer | 637.95 µs | 3.02x | pass |
| multipart buffered | login | parse_multipart | 1.41 µs | multipart 0.18 | 1.96 µs | 1.39x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 4.04 µs | multipart 0.18 | 7.25 µs | 1.80x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.60 ms | multipart 0.18 | 4.12 ms | 2.57x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 6.80 ms | multipart 0.18 SaveBuilder | 16.14 ms | 2.37x | pass |
