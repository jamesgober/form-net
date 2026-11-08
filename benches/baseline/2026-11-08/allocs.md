| Family | Input | Crate | Allocations | Bytes allocated |
|---|---|---|---|---|
| urlencoded pairs | login | form-net parse_urlencoded | 2 | 186 |
| urlencoded pairs | login | form-net raw_pairs+decode_into | 0 | 0 |
| urlencoded pairs | login | form_urlencoded | 5 | 240 |
| urlencoded pairs | signup | form-net parse_urlencoded | 2 | 502 |
| urlencoded pairs | signup | form-net raw_pairs+decode_into | 0 | 0 |
| urlencoded pairs | signup | form_urlencoded | 14 | 773 |
| urlencoded pairs | 1000-fields | form-net parse_urlencoded | 2 | 57779 |
| urlencoded pairs | 1000-fields | form-net raw_pairs+decode_into | 0 | 0 |
| urlencoded pairs | 1000-fields | form_urlencoded | 9 | 98112 |
| urlencoded struct | signup | form-net form! | 6 | 95 |
| urlencoded struct | signup | serde_urlencoded | 14 | 201 |
| urlencoded struct | signup | serde_qs | 8 | 741 |
| urlencoded serialize | 10 pairs | form-net | 1 | 256 |
| urlencoded serialize | 10 pairs | form_urlencoded | 6 | 504 |
| urlencoded serialize | 10 pairs | serde_urlencoded | 16 | 566 |
| multipart stream | login, whole chunks | form-net MultipartParser | 1 | 8480 |
| multipart stream | login, whole chunks | multer | 41 | 1657 |
| multipart stream | login, 65536 B chunks | form-net MultipartParser | 1 | 8480 |
| multipart stream | login, 65536 B chunks | multer | 39 | 1601 |
| multipart stream | login, 1500 B chunks | form-net MultipartParser | 1 | 8480 |
| multipart stream | login, 1500 B chunks | multer | 39 | 1601 |
| multipart buffered | login | form-net parse_multipart | 2 | 9024 |
| multipart buffered | login | multipart 0.18 | 11 | 8495 |
| multipart stream | avatar-64KiB, whole chunks | form-net MultipartParser | 1 | 8480 |
| multipart stream | avatar-64KiB, whole chunks | multer | 43 | 67359 |
| multipart stream | avatar-64KiB, 65536 B chunks | form-net MultipartParser | 1 | 8480 |
| multipart stream | avatar-64KiB, 65536 B chunks | multer | 43 | 198022 |
| multipart stream | avatar-64KiB, 1500 B chunks | form-net MultipartParser | 1 | 8480 |
| multipart stream | avatar-64KiB, 1500 B chunks | multer | 48 | 191914 |
| multipart buffered | avatar-64KiB | form-net parse_multipart | 2 | 9024 |
| multipart buffered | avatar-64KiB | multipart 0.18 | 13 | 8525 |
| multipart stream | document-10MiB, whole chunks | form-net MultipartParser | 1 | 8480 |
| multipart stream | document-10MiB, whole chunks | multer | 33 | 10487266 |
| multipart stream | document-10MiB, 65536 B chunks | form-net MultipartParser | 1 | 8480 |
| multipart stream | document-10MiB, 65536 B chunks | multer | 40 | 33490076 |
| multipart stream | document-10MiB, 1500 B chunks | form-net MultipartParser | 1 | 8480 |
| multipart stream | document-10MiB, 1500 B chunks | multer | 45 | 24575680 |
| multipart buffered | document-10MiB | form-net parse_multipart | 2 | 9024 |
| multipart buffered | document-10MiB | multipart 0.18 | 11 | 8499 |
