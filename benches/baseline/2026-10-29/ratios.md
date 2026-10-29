| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 160 ns | form_urlencoded (collect) | 235 ns | 1.47x | pass |
| urlencoded pairs | login | pairs (no collect) | 125 ns | form_urlencoded (no collect) | 210 ns | 1.68x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 107 ns | form_urlencoded (no collect) | 209 ns | 1.96x | pass |
| urlencoded pairs | search | pairs (collect) | 105 ns | form_urlencoded (collect) | 92 ns | 0.88x | below |
| urlencoded pairs | search | pairs (no collect) | 72 ns | form_urlencoded (no collect) | 71 ns | 0.99x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 69 ns | form_urlencoded (no collect) | 72 ns | 1.04x | below |
| urlencoded pairs | signup | pairs (collect) | 384 ns | form_urlencoded (collect) | 672 ns | 1.75x | pass |
| urlencoded pairs | signup | pairs (no collect) | 317 ns | form_urlencoded (no collect) | 628 ns | 1.98x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 290 ns | form_urlencoded (no collect) | 629 ns | 2.17x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 19.36 µs | form_urlencoded (collect) | 26.88 µs | 1.39x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 16.33 µs | form_urlencoded (no collect) | 24.55 µs | 1.50x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 32.42 µs | form_urlencoded (no collect) | 24.99 µs | 0.77x | below |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 98.24 µs | form_urlencoded (collect) | 888.40 µs | 9.04x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 97.40 µs | form_urlencoded (no collect) | 929.88 µs | 9.55x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 202.28 µs | form_urlencoded (no collect) | 944.96 µs | 4.67x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 370.36 µs | form_urlencoded (collect) | 900.80 µs | 2.43x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 331.71 µs | form_urlencoded (no collect) | 887.44 µs | 2.68x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 320.23 µs | form_urlencoded (no collect) | 871.65 µs | 2.72x | pass |
| urlencoded struct | signup | form! | 465 ns | serde_urlencoded | 740 ns | 1.59x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 152 ns | form_urlencoded | 344 ns | 2.26x | pass |
| multipart stream | login, whole | MultipartParser | 670 ns | multer | 2.32 µs | 3.46x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 2.54 µs | multer | 5.28 µs | 2.08x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 2.56 µs | multer | 5.22 µs | 2.03x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 3.20 µs | multer | 7.85 µs | 2.45x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 15.08 µs | multer | 63.52 µs | 4.21x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 15.37 µs | multer | 63.77 µs | 4.15x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 751.69 µs | multer | 1.48 ms | 1.97x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 752.11 µs | multer | 3.53 ms | 4.69x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 933.73 µs | multer | 3.37 ms | 3.60x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 66.66 µs | multer | 165.22 µs | 2.48x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 67.56 µs | multer | 335.97 µs | 4.97x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 92.42 µs | multer | 227.02 µs | 2.46x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 123.48 µs | multer | 165.11 µs | 1.34x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 115.10 µs | multer | 333.74 µs | 2.90x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 160.64 µs | multer | 212.24 µs | 1.32x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 139.55 µs | multer | 375.32 µs | 2.69x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 140.07 µs | multer | 513.95 µs | 3.67x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 161.69 µs | multer | 418.48 µs | 2.59x | pass |
| multipart buffered | login | parse_multipart | 932 ns | multipart 0.18 | 1.27 µs | 1.36x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 2.80 µs | multipart 0.18 | 4.05 µs | 1.44x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.32 ms | multer | 3.04 ms | 2.31x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 3.95 ms | multipart 0.18 SaveBuilder | 8.23 ms | 2.09x | pass |
