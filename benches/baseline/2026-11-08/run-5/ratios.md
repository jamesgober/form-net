| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 266 ns | form_urlencoded (collect) | 433 ns | 1.63x | pass |
| urlencoded pairs | login | pairs (no collect) | 203 ns | form_urlencoded (no collect) | 371 ns | 1.83x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 135 ns | form_urlencoded (no collect) | 392 ns | 2.91x | pass |
| urlencoded pairs | search | pairs (collect) | 181 ns | form_urlencoded (collect) | 157 ns | 0.86x | below |
| urlencoded pairs | search | pairs (no collect) | 112 ns | form_urlencoded (no collect) | 122 ns | 1.09x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 84 ns | form_urlencoded (no collect) | 130 ns | 1.55x | pass |
| urlencoded pairs | signup | pairs (collect) | 669 ns | form_urlencoded (collect) | 1.22 µs | 1.83x | pass |
| urlencoded pairs | signup | pairs (no collect) | 486 ns | form_urlencoded (no collect) | 977 ns | 2.01x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 374 ns | form_urlencoded (no collect) | 1.09 µs | 2.90x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 30.06 µs | form_urlencoded (collect) | 55.43 µs | 1.84x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 24.95 µs | form_urlencoded (no collect) | 51.32 µs | 2.06x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 35.36 µs | form_urlencoded (no collect) | 47.71 µs | 1.35x | pass |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 156.21 µs | form_urlencoded (collect) | 1.57 ms | 10.03x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 145.49 µs | form_urlencoded (no collect) | 1.61 ms | 11.04x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 146.06 µs | form_urlencoded (no collect) | 1.53 ms | 10.51x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 811.65 µs | form_urlencoded (collect) | 1.76 ms | 2.16x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 681.60 µs | form_urlencoded (no collect) | 1.67 ms | 2.45x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 746.83 µs | form_urlencoded (no collect) | 1.59 ms | 2.13x | pass |
| urlencoded struct | signup | form! | 670 ns | serde_urlencoded | 1.18 µs | 1.76x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 436 ns | form_urlencoded | 843 ns | 1.93x | pass |
| multipart stream | login, whole | MultipartParser | 1.22 µs | multer | 4.15 µs | 3.40x | pass |
| multipart stream | login, whole | MultipartParser | 1.29 µs | multer | 4.03 µs | 3.13x | pass |
| multipart stream | login, whole | MultipartParser | 1.22 µs | multer | 4.27 µs | 3.51x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 4.05 µs | multer | 7.94 µs | 1.96x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 4.26 µs | multer | 8.89 µs | 2.09x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 5.90 µs | multer | 13.53 µs | 2.29x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 27.52 µs | multer | 107.12 µs | 3.89x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 28.40 µs | multer | 103.19 µs | 3.63x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 29.54 µs | multer | 106.59 µs | 3.61x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 890.51 µs | multer | 3.58 ms | 4.02x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 956.12 µs | multer | 9.88 ms | 10.34x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 1.32 ms | multer | 8.31 ms | 6.30x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 34.60 µs | multer | 346.49 µs | 10.01x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 39.48 µs | multer | 651.56 µs | 16.50x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 80.73 µs | multer | 442.77 µs | 5.48x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 225.28 µs | multer | 416.10 µs | 1.85x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 290.18 µs | multer | 721.94 µs | 2.49x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 282.32 µs | multer | 449.20 µs | 1.59x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 190.07 µs | multer | 815.20 µs | 4.29x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 182.82 µs | multer | 985.92 µs | 5.39x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 226.93 µs | multer | 835.67 µs | 3.68x | pass |
| multipart buffered | login | parse_multipart | 1.58 µs | multipart 0.18 | 2.16 µs | 1.37x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 4.11 µs | multipart 0.18 | 7.07 µs | 1.72x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.70 ms | multipart 0.18 | 5.63 ms | 3.31x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 8.02 ms | multipart 0.18 SaveBuilder | 17.69 ms | 2.21x | pass |
