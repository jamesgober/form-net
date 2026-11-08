| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 242 ns | form_urlencoded (collect) | 393 ns | 1.63x | pass |
| urlencoded pairs | login | pairs (no collect) | 197 ns | form_urlencoded (no collect) | 350 ns | 1.78x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 126 ns | form_urlencoded (no collect) | 376 ns | 2.97x | pass |
| urlencoded pairs | search | pairs (collect) | 152 ns | form_urlencoded (collect) | 156 ns | 1.02x | below |
| urlencoded pairs | search | pairs (no collect) | 116 ns | form_urlencoded (no collect) | 119 ns | 1.03x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 76 ns | form_urlencoded (no collect) | 119 ns | 1.56x | pass |
| urlencoded pairs | signup | pairs (collect) | 577 ns | form_urlencoded (collect) | 1.09 µs | 1.88x | pass |
| urlencoded pairs | signup | pairs (no collect) | 508 ns | form_urlencoded (no collect) | 1.04 µs | 2.04x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 346 ns | form_urlencoded (no collect) | 1.01 µs | 2.93x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 25.45 µs | form_urlencoded (collect) | 50.41 µs | 1.98x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 22.78 µs | form_urlencoded (no collect) | 47.11 µs | 2.07x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 32.74 µs | form_urlencoded (no collect) | 50.04 µs | 1.53x | pass |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 145.19 µs | form_urlencoded (collect) | 1.50 ms | 10.30x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 143.24 µs | form_urlencoded (no collect) | 1.41 ms | 9.81x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 141.73 µs | form_urlencoded (no collect) | 1.35 ms | 9.52x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 757.60 µs | form_urlencoded (collect) | 1.74 ms | 2.30x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 632.70 µs | form_urlencoded (no collect) | 1.52 ms | 2.40x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 679.50 µs | form_urlencoded (no collect) | 1.43 ms | 2.11x | pass |
| urlencoded struct | signup | form! | 627 ns | serde_urlencoded | 1.16 µs | 1.84x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 397 ns | form_urlencoded | 766 ns | 1.93x | pass |
| multipart stream | login, whole | MultipartParser | 1.05 µs | multer | 3.73 µs | 3.55x | pass |
| multipart stream | login, whole | MultipartParser | 1.07 µs | multer | 3.69 µs | 3.46x | pass |
| multipart stream | login, whole | MultipartParser | 1.09 µs | multer | 3.75 µs | 3.43x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 3.52 µs | multer | 6.23 µs | 1.77x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 3.56 µs | multer | 6.38 µs | 1.79x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 5.15 µs | multer | 8.35 µs | 1.62x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 24.23 µs | multer | 96.56 µs | 3.99x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 25.59 µs | multer | 103.60 µs | 4.05x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 25.73 µs | multer | 96.51 µs | 3.75x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 846.75 µs | multer | 2.13 ms | 2.52x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 867.90 µs | multer | 5.79 ms | 6.67x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 1.21 ms | multer | 5.71 ms | 4.73x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 29.60 µs | multer | 245.92 µs | 8.31x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 31.91 µs | multer | 511.60 µs | 16.03x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 70.46 µs | multer | 325.74 µs | 4.62x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 189.19 µs | multer | 262.55 µs | 1.39x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 199.73 µs | multer | 479.25 µs | 2.40x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 241.71 µs | multer | 353.07 µs | 1.46x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 173.35 µs | multer | 675.55 µs | 3.90x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 187.88 µs | multer | 927.20 µs | 4.94x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 218.82 µs | multer | 714.60 µs | 3.27x | pass |
| multipart buffered | login | parse_multipart | 1.36 µs | multipart 0.18 | 1.86 µs | 1.36x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 4.07 µs | multer | 6.35 µs | 1.56x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.63 ms | multipart 0.18 | 4.11 ms | 2.52x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 7.19 ms | multipart 0.18 SaveBuilder | 17.08 ms | 2.38x | pass |
