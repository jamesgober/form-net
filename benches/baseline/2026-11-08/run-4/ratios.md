| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 234 ns | form_urlencoded (collect) | 391 ns | 1.67x | pass |
| urlencoded pairs | login | pairs (no collect) | 190 ns | form_urlencoded (no collect) | 369 ns | 1.94x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 126 ns | form_urlencoded (no collect) | 349 ns | 2.78x | pass |
| urlencoded pairs | search | pairs (collect) | 164 ns | form_urlencoded (collect) | 154 ns | 0.94x | below |
| urlencoded pairs | search | pairs (no collect) | 113 ns | form_urlencoded (no collect) | 121 ns | 1.07x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 80 ns | form_urlencoded (no collect) | 121 ns | 1.51x | pass |
| urlencoded pairs | signup | pairs (collect) | 562 ns | form_urlencoded (collect) | 1.09 µs | 1.94x | pass |
| urlencoded pairs | signup | pairs (no collect) | 527 ns | form_urlencoded (no collect) | 1.02 µs | 1.93x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 368 ns | form_urlencoded (no collect) | 1.02 µs | 2.77x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 27.31 µs | form_urlencoded (collect) | 50.67 µs | 1.86x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 23.10 µs | form_urlencoded (no collect) | 47.49 µs | 2.06x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 32.45 µs | form_urlencoded (no collect) | 48.50 µs | 1.49x | pass |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 139.65 µs | form_urlencoded (collect) | 1.40 ms | 10.01x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 140.55 µs | form_urlencoded (no collect) | 1.40 ms | 9.98x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 142.53 µs | form_urlencoded (no collect) | 1.38 ms | 9.68x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 748.30 µs | form_urlencoded (collect) | 1.59 ms | 2.13x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 641.60 µs | form_urlencoded (no collect) | 1.55 ms | 2.41x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 691.17 µs | form_urlencoded (no collect) | 1.49 ms | 2.15x | pass |
| urlencoded struct | signup | form! | 632 ns | serde_urlencoded | 1.16 µs | 1.84x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 394 ns | form_urlencoded | 761 ns | 1.93x | pass |
| multipart stream | login, whole | MultipartParser | 1.23 µs | multer | 3.82 µs | 3.10x | pass |
| multipart stream | login, whole | MultipartParser | 1.11 µs | multer | 3.67 µs | 3.32x | pass |
| multipart stream | login, whole | MultipartParser | 1.09 µs | multer | 3.67 µs | 3.36x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 3.61 µs | multer | 6.87 µs | 1.90x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 3.62 µs | multer | 6.41 µs | 1.77x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 5.57 µs | multer | 10.69 µs | 1.92x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 25.61 µs | multer | 94.24 µs | 3.68x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 25.23 µs | multer | 99.45 µs | 3.94x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 27.27 µs | multer | 108.14 µs | 3.97x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 863.98 µs | multer | 3.27 ms | 3.79x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 884.80 µs | multer | 8.12 ms | 9.18x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 1.19 ms | multer | 6.36 ms | 5.35x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 31.08 µs | multer | 289.35 µs | 9.31x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 34.60 µs | multer | 561.75 µs | 16.24x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 75.80 µs | multer | 355.68 µs | 4.69x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 194.48 µs | multer | 270.08 µs | 1.39x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 231.35 µs | multer | 640.35 µs | 2.77x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 248.12 µs | multer | 398.52 µs | 1.61x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 174.19 µs | multer | 714.10 µs | 4.10x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 174.84 µs | multer | 938.90 µs | 5.37x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 209.00 µs | multer | 776.90 µs | 3.72x | pass |
| multipart buffered | login | parse_multipart | 1.32 µs | multipart 0.18 | 1.85 µs | 1.40x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 4.01 µs | multipart 0.18 | 6.70 µs | 1.67x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.61 ms | multipart 0.18 | 3.76 ms | 2.34x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 7.04 ms | multipart 0.18 SaveBuilder | 18.97 ms | 2.70x | pass |
