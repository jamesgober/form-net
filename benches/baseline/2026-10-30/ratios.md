| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 156 ns | form_urlencoded (collect) | 226 ns | 1.45x | pass |
| urlencoded pairs | login | pairs (no collect) | 126 ns | form_urlencoded (no collect) | 218 ns | 1.74x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 105 ns | form_urlencoded (no collect) | 219 ns | 2.08x | pass |
| urlencoded pairs | search | pairs (collect) | 104 ns | form_urlencoded (collect) | 91 ns | 0.88x | below |
| urlencoded pairs | search | pairs (no collect) | 75 ns | form_urlencoded (no collect) | 70 ns | 0.94x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 77 ns | form_urlencoded (no collect) | 70 ns | 0.91x | below |
| urlencoded pairs | signup | pairs (collect) | 380 ns | form_urlencoded (collect) | 668 ns | 1.76x | pass |
| urlencoded pairs | signup | pairs (no collect) | 316 ns | form_urlencoded (no collect) | 624 ns | 1.97x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 273 ns | form_urlencoded (no collect) | 634 ns | 2.32x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 19.02 µs | form_urlencoded (collect) | 25.51 µs | 1.34x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 16.16 µs | form_urlencoded (no collect) | 23.35 µs | 1.44x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 32.10 µs | form_urlencoded (no collect) | 22.99 µs | 0.72x | below |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 96.33 µs | form_urlencoded (collect) | 767.89 µs | 7.97x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 96.77 µs | form_urlencoded (no collect) | 774.39 µs | 8.00x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 170.02 µs | form_urlencoded (no collect) | 769.99 µs | 4.53x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 565.90 µs | form_urlencoded (collect) | 982.40 µs | 1.74x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 451.69 µs | form_urlencoded (no collect) | 903.10 µs | 2.00x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 313.15 µs | form_urlencoded (no collect) | 902.54 µs | 2.88x | pass |
| urlencoded struct | signup | form! | 434 ns | serde_urlencoded | 758 ns | 1.75x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 147 ns | form_urlencoded | 322 ns | 2.20x | pass |
| multipart stream | login, whole | MultipartParser | 663 ns | multer | 2.24 µs | 3.38x | pass |
| multipart stream | login, whole | MultipartParser | 666 ns | multer | 2.24 µs | 3.37x | pass |
| multipart stream | login, whole | MultipartParser | 657 ns | multer | 2.19 µs | 3.33x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 2.51 µs | multer | 4.16 µs | 1.66x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 2.52 µs | multer | 4.94 µs | 1.96x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 3.31 µs | multer | 5.92 µs | 1.79x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 15.74 µs | multer | 61.91 µs | 3.93x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 15.76 µs | multer | 61.26 µs | 3.89x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 15.97 µs | multer | 61.72 µs | 3.87x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 739.92 µs | multer | 1.44 ms | 1.95x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 738.98 µs | multer | 3.60 ms | 4.87x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 959.24 µs | multer | 3.31 ms | 3.45x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 24.94 µs | multer | 147.50 µs | 5.91x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 25.50 µs | multer | 314.25 µs | 12.32x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 44.72 µs | multer | 199.44 µs | 4.46x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 119.94 µs | multer | 147.69 µs | 1.23x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 120.66 µs | multer | 318.14 µs | 2.64x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 147.01 µs | multer | 198.62 µs | 1.35x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 150.21 µs | multer | 367.84 µs | 2.45x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 151.49 µs | multer | 506.46 µs | 3.34x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 174.09 µs | multer | 395.53 µs | 2.27x | pass |
| multipart buffered | login | parse_multipart | 917 ns | multipart 0.18 | 1.20 µs | 1.31x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 2.70 µs | multipart 0.18 | 3.90 µs | 1.44x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.31 ms | multer | 2.98 ms | 2.27x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 4.17 ms | multipart 0.18 SaveBuilder | 8.59 ms | 2.06x | pass |
