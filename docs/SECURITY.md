# form-net security

The threat model, every deliberate difference from the standards, and the differential log: what form-net refuses that other readers accept, and why. The design behind it is `dev/PLAN.md` sections 10 and 12; the decisions are its log (D-numbers below).

---

## The rule

form-net refuses any input that two reasonable parsers could read differently, and never repairs input. A body is either read the one way every careful reader reads it, or refused with a reason and a location. Every leniency is a named option with a secure default, documented with the disagreement it risks. Nothing a client sends is trusted: every limit is checked before the bytes it governs are buffered, stored, or passed on.

---

## Threats

Every threat has an attack test named after it. Costs are per request unless stated, measured at 0.6 where marked in `docs/PERFORMANCE.md`.

| Id | Threat | Defence | Attack test | Cost |
|---|---|---|---|---|
| T-1 | many tiny parts (Werkzeug CVE-2023-25577) | `max_parts` at each delimiter, before headers | `test_attack_t1_many_tiny_parts_refused` (`tests/attacks.rs`) | one compare per part |
| T-2 | many fields, hash flooding | `max_fields`; no hash map in the crate | `test_attack_t2_many_fields_refused` (`tests/attacks.rs`) | one compare per pair |
| T-3 | per-byte work on preamble or epilogue (python-multipart CVE-2024-53981) | SWAR-scanned preamble; epilogue counted O(1) per chunk; both budgeted | `test_attack_t3_huge_preamble_and_epilogue_refused` (`tests/attacks.rs`) | none on normal bodies |
| T-4 | quadratic boundary search (Werkzeug CVE-2023-46136) | streaming KMP, state across chunks, no growing buffer | `test_attack_t4_cve_2023_46136_shape_is_linear_at_every_chunk_size` (`tests/attacks.rs`) | none |
| T-5 | header bomb inside a part | `max_part_header_bytes`, `max_part_headers` before copying | `test_attack_t5_header_bomb_refused` (`tests/attacks.rs`) | one compare per line |
| T-6 | one huge text value | `max_value_bytes`, `max_text_bytes` before buffering | `test_attack_t6_huge_text_value_refused_at_the_limit` (`tests/attacks.rs`) | one compare per chunk |
| T-7 | disk filling | `max_file_bytes`, `max_files`, `max_disk_bytes`, `SpillBudget`, reserved before writes | `test_attack_t7_parallel_uploads_never_exceed_the_shared_budget` (`tests/budget.rs`) | one CAS per write, 5 to 20 ns |
| T-8 | aborted uploads leave files or handles (Node multer CVE-2026-5038, CVE-2026-77037) | abort closes then deletes every file; anonymous files have no name; `Truncated` at eof | `test_attack_t8_abort_at_every_byte_leaves_nothing` (`tests/abort.rs`) | none on success |
| T-9 | temporary file races: predictable names, planted files, symlinks | fresh private directory with 128 random bits, 0700; random file names; `create_new`; 0600 | `test_attack_t9_planted_file_is_never_opened` (`src/store/temp.rs`)<br>`test_attack_t9_planted_link_is_never_followed` (`src/store/temp.rs`) | one random read per file |
| T-10 | disclosure to other local users | 0600 in 0700 (Unix); per-user temp ACL (Windows) | `test_attack_t10_permissions_are_owner_only` (`tests/store.rs`)<br>`test_attack_t10_temp_directory_is_under_the_user_profile` (`tests/store.rs`) | none |
| T-11 | a crash leaves uploads on disk | anonymous mode; named mode plus `sweep` | `test_attack_t11_named_files_survive_a_crash_until_sweep` (`tests/crash.rs`) | none |
| T-12 | delimiter prefix inside data | a full delimiter followed by anything but `--`, padding, or CRLF is refused | `test_attack_t12_delimiter_prefix_inside_data_refused` (`tests/attacks.rs`) | none |
| T-13 | two boundaries, or token and quoted forms that read differently | exactly one `boundary`; parse-net grammar | `test_attack_t13_two_boundaries_refused` (`tests/attacks.rs`) | none |
| T-14 | differential with a WAF or proxy in front (WAFFLED) | refuse every construct readers disagree on: duplicate disposition, duplicate `name`, both file names, transfer encodings, bare LF, folding, whitespace before colon, `\"`, missing close delimiter, data past the epilogue budget | `test_attack_t14_waf_differential_shapes_refused` (`tests/attacks.rs`)<br>`test_attack_t14_waffled_mutations_are_refused_or_read_the_same` (`tests/waffled.rs`) | none on clean bodies |
| T-15 | differential with browsers (refusing real forms) | browser capture corpus (NFR-14) | `test_attack_t15_browser_captures_parse_to_their_fields` (`tests/multipart_browsers.rs`) | none |
| T-16 | `;` as a separator (Python CVE-2021-23336) | `&` only | `test_attack_t16_semicolon_is_not_a_separator` (`tests/attacks.rs`) | none |
| T-17 | parameter pollution | order kept; `get_one`, `field_one`, typed extraction refuse duplicates; `get` documented as first-wins | `test_attack_t17_parameter_pollution_refused_by_strict_accessors` (`tests/attacks.rs`) | none |
| T-18 | nested bracket names (Node multer CVE-2026-5079, CVE-2026-82333, CVE-2026-77078) | names opaque; `NamePath` (after 1.0) checks depth and index while scanning | `test_attack_t18_bracket_names_are_opaque` (`tests/attacks.rs`) | none |
| T-19 | invalid encodings smuggling bytes | strict percent and UTF-8; never replace | `test_attack_t19_invalid_encodings_refused` (`tests/attacks.rs`) | none |
| T-20 | file name traversal, absolute, UNC, streams, device names | names never paths; random spill names; `sanitized`; `persist` never overwrites | `test_attack_t20_t21_file_names_are_never_paths` (`tests/attacks_upload.rs`) | about 100 ns when `sanitized` is called |
| T-21 | file name Unicode tricks (RLO, NFD, invisibles) | `sanitized` replaces every default-ignorable character (D-084), path-lib the rest; NFC with `unicode` (0.5) | `test_attack_t20_t21_file_names_are_never_paths` (`tests/attacks_upload.rs`)<br>`test_attack_t21_nfd_names_sanitize_like_nfc` (`tests/attacks_upload.rs`) | only when called |
| T-22 | content type spoofing | declared type is a hint; sniff window for customs-sec, on by default (512 bytes, D-122) | `test_attack_t22_t38_sniff_sees_real_bytes_before_any_write` (`tests/attacks_upload.rs`)<br>`test_alpha_upload_route_at_every_chunk_size` (`tests/alpha_routes.rs`) | one copy of up to `sniff_bytes` per file |
| T-23 | uploads stored before the token is checked | `TokenField` with `BeforeFirstFile`; `TokenCheck` verifies the token before the next part starts (D-120) | `test_attack_t23_late_token_refused_before_the_sink_opens` (`tests/attacks_upload.rs`)<br>`test_attack_t23_session_token_is_available_before_the_files` (`tests/attacks_upload.rs`)<br>`test_attack_t23_t24_token_rules` (`tests/attacks.rs`)<br>`test_attack_a_forged_token_stops_the_upload_before_any_file_byte` (`tests/token_check.rs`)<br>`test_attack_a_file_before_the_token_is_refused_before_it_opens` (`tests/token_check.rs`) | about 10 ns per text field name |
| T-24 | two token fields | `DuplicateToken`; `TokenCheck` refuses a second token in either encoding | `test_attack_t23_t24_token_rules` (`tests/attacks.rs`)<br>`test_attack_two_tokens_are_refused_whichever_is_valid` (`tests/token_check.rs`) | none |
| T-25 | charset confusion (UTF-7 and friends) | UTF-8 only | `test_attack_t25_t26_t27_hidden_content_refused` (`tests/attacks_upload.rs`) | none |
| T-26 | base64 or quoted-printable parts hide content | transfer encodings refused | `test_attack_t25_t26_t27_hidden_content_refused` (`tests/attacks_upload.rs`) | none |
| T-27 | nested `multipart/mixed` hides files | refused | `test_attack_t25_t26_t27_hidden_content_refused` (`tests/attacks_upload.rs`) | none |
| T-28 | counter overflow | `u64`, checked arithmetic | `test_attack_t28_counters_near_u64_max_never_wrap` (`src/multipart/parser.rs`) | none |
| T-29 | panic on crafted input (release builds abort) | no panicking constructs; fuzzing | the fuzz targets, 24 hours each (below) | none |
| T-30 | secrets in logs | errors hold no values; `Debug` redacts | `test_attack_t30_errors_and_debug_never_show_values` (`tests/attacks.rs`) | none |
| T-31 | secret residue in freed memory | volatile wipe on drop (D-083) | `test_attack_t31_owned_values_are_wiped_on_drop` (`src/wipe.rs`)<br>`test_attack_t31_stored_files_and_uploads_are_wiped_on_drop` (`src/wipe.rs`) | 50 to 100 ns per 2 KiB form |
| T-32 | split-dependent results | split invariance as a property | `test_attack_t32_split_invariant` (`tests/properties.rs`) | none |
| T-33 | memory exhaustion through many concurrent uploads | `memory_bound` for admission; bounded buffers | `test_attack_t33_concurrent_uploads_stay_within_the_bound` (`tests/concurrent_memory.rs`) | none |
| T-34 | header injection in built requests | writer escapes LF, CR, `"`; refuses other controls; content types validated | `test_attack_t34_header_injection_refused` (`tests/writer.rs`) | about 10 ns per part |
| T-35 | boundary collision in client bodies | 128-bit generated boundaries; caller boundaries scanned | `test_attack_t35_data_containing_the_delimiter_is_refused_across_splits` (`tests/writer.rs`) | 0.1 to 0.3 ns per byte |
| T-36 | quoted-pair ambiguity in `filename` | backslash literal, `\"` refused | `test_attack_t36_quoted_pair_ambiguity_refused` (`tests/attacks.rs`) | none |
| T-37 | empty or missing names | `EmptyName`, `MissingName` | `test_attack_t37_empty_and_missing_names_refused` (`tests/attacks.rs`) | none |
| T-38 | inspector bypass through ordering | drivers call `part_start`, `sniff`, and the first `data` before any sink call (D-089) | `test_attack_t22_t38_sniff_sees_real_bytes_before_any_write` (`tests/attacks_upload.rs`) | none |

Out of scope, named so nobody assumes otherwise: decompression bombs (the HTTP layer and customs-sec), slow uploads (serve-net and net-io), malware in files and image parser exploits (the application, in a sandbox), cross-site scripting through a reflected file name (the render layer's escaping).

---

## Where form-net is stricter than the standards

### urlencoded (WHATWG URL, section 5.1)

| Input | WHATWG | form-net |
|---|---|---|
| `%` not followed by two hex digits | kept as a literal byte | refused under the default `PercentPolicy::Refuse`; kept under `PercentPolicy::Literal` |
| invalid UTF-8 after decoding | replaced with U+FFFD | refused, always: replacement merges distinct inputs |
| `;` | an ordinary byte | the same; never a separator (Python CVE-2021-23336) |
| sizes and counts | unbounded | `max_fields`, `max_name_bytes`, `max_value_bytes`, `max_text_bytes`, `max_body_bytes`, all finite by default |

### multipart (RFC 7578, RFC 2046, the HTML form encoding)

| Construct | Other readers | form-net | Decision |
|---|---|---|---|
| a bare LF where CR LF is required | often accepted | refused (`BareLineFeed`) | D-019 |
| `--boundary` after a lone CR or LF | some readers (Werkzeug's, for one) take it as a delimiter | refused (`BoundaryInContent`) | D-113 |
| `--boundary` as the first bytes of a part's data | an empty part to RFC 2046 readers, data to others | refused (`BoundaryInContent`) | D-113 |
| a full delimiter followed by other bytes | read as data by some | refused (`BoundaryInContent`) | FR-18 |
| `\"` inside a quoted value | a quoted pair (RFC 9110) or a backslash and the end of the value (HTML) | refused (`AmbiguousQuote`) | D-015 |
| a backslash in `name` | dropped (RFC 9110) or kept (browsers) | refused (`MalformedParameter`); kept literally in a file name | D-082 |
| an unquoted `name` or `filename` starting or ending with `'` | the token itself, or the quoted text | refused (`MalformedParameter`) | D-114 |
| `name`, `filename`, or `filename*` not in lower case (`NAME=`) | the same parameter (RFC 7578), or an unknown one (readers that match exactly, multer 3.1 among them) | refused (`MalformedParameter`) | D-116 |
| `name*`, `filename*0` and other starred parameters | joined or preferred by RFC 2231 readers | refused (`MalformedParameter`) | D-081 |
| `boundary*` beside `boundary` in the request `Content-Type`, or any starred request parameter | Go's `mime.ParseMediaType` frames the body with the starred value | refused (`InvalidBoundary`, `InvalidContentType`) | D-136 |
| an empty parameter (`;;`) in the request `Content-Type` or a part header | allowed by RFC 9110; Go refuses the whole list, so it sees no boundary, or a part with no name, which `net/http` skips | refused (`InvalidContentType`, `MalformedParameter`) | D-136 |
| a parameter value that holds `name=` (`x="y;name=evil;"`, `filename="a; name=role;"`) | the value (RFC 6266), or the parameter itself (readers that search for `name=`, multer 3.1 among them) | refused (`MalformedParameter`) | D-136 |
| a boundary starting or ending with `'` | the token itself, or the quoted text | refused (`InvalidBoundary`) | D-136 |
| `filename` and `filename*` together | either one wins, by reader | refused (`BothFilenames`); `filename*` alone only by policy | D-014, FR-27 |
| `filename*` alone under `Ignore` | a file to RFC 2231 readers | refused (`ExtendedFilename`), as under `Refuse` | D-147 |
| a repeated `Content-Disposition` or parameter | first or last wins, by reader | refused | FR-26 |
| header folding, whitespace before a colon, control bytes | accepted by some | refused | FR-25 |
| `Content-Transfer-Encoding` other than identity | ignored by most | refused | T-26 |
| a `multipart/*` part | passed on as bytes | refused (`NestedMultipart`) | T-27 |
| a charset other than UTF-8 (`_charset_`, part `charset`) | decoded or replaced | refused | T-25 |
| data past the close delimiter | ignored | at most `max_epilogue_bytes`, then refused | T-3 |

Browsers send none of these. The capture corpus below proves that every body current Chrome, Edge, Firefox, and Safari send is accepted; the shapes they can send that form-net refuses by design are tested as refusals.

---

## The capture corpus

`tests/corpus` holds the bodies real browsers and clients sent for one fixed page (`tools/capture/capture.py`): text with ASCII, non-ASCII, CR LF, and quotes; names with `"`, `%`, and non-ASCII; an empty file input; file names with non-ASCII, `"`, `%`, `\`, and a line break; two files under one name; `_charset_`. `tests/multipart_browsers.rs` checks each whole and at every split, and fails if any of Chrome, Edge, Firefox, or Safari is missing (D-115, D-119).

| Source | Captured | Result |
|---|---|---|
| Chrome 155 (headless) | 2026-10-29 | every field read as sent; `"` and LF in names and file names arrive as `%22` and `%0A`, which `name()` decodes and `FileName::html_unescaped` decodes |
| Edge 155 (headless) | 2026-10-29 | the same bytes as Chrome, but for the boundary |
| curl 8.5 | 2026-10-29 | every field read as sent; `"` in a file name arrives as `%22` |
| Python requests 2.31 | 2026-10-29 | the same |
| Go 1.27 `mime/multipart` | 2026-10-29 | every field read as sent; a `"` in a file name arrives as `\"`, which form-net refuses (`AmbiguousQuote`): a Go client should not send one |
| Firefox 159 (headless) | 2026-11-04 | the same bytes as Chrome, but for the boundary |
| Safari 26 (macOS 26.6, through safaridriver on a GitHub runner) | 2026-11-04 | the same bytes as Chrome, but for the boundary |

A part named with a backslash (`name="back\slash"`), which every browser sends as is, is refused (D-082); the corpus holds it as `refused.body` for each browser.

---

## Differential log

### The WAFFLED-style generator

`tests/common/waffled.rs` mutates every captured body in the categories the WAFFLED study found readers disagree on: duplicated, reordered, re-cased, re-spaced, and re-quoted headers and parameters, line endings, folding, transport padding, preambles, epilogues, and truncation. `tests/waffled.rs` requires every mutant to be refused or to read exactly as the original, whole and in chunks of 1, 7, and 64 bytes, and the shapes real clients produce (header case and order, spacing after `;`, padding, a short preamble or epilogue) to be accepted.

The generator found two disagreements, and the multer differential a third, all fixed before 0.7:

| Found | form-net read | Others read | Fix |
|---|---|---|---|
| `\n--boundary` inside a part | data, one part | a delimiter, two parts | refused (D-113) |
| `name='x'` | the name `'x'` | the name `x` | refused (D-114) |
| `NAME="x"` | the name `x` | no name (multer 3.1) | refused (D-116) |

With all three fixed, the 1,319 mutants of the corpus split 519 read the same and 800 refused, with none read differently.

### multer

`benches/src/bin/differential.rs` runs every mutant through form-net and multer 3.1.0 and counts, per category, how each reads it against its own reading of the original:

| Mutation | Mutants | form-net same / refused / different | multer same / refused / different | Both accepted, read differently |
|---|---|---|---|---|
| Content-Transfer-Encoding | 89 | 0 / 89 / 0 | 89 / 0 / 0 | 0 |
| NUL in header | 89 | 0 / 89 / 0 | 0 / 89 / 0 | 0 |
| bare LF before a delimiter | 7 | 0 / 7 / 0 | 0 / 0 / 7 | 0 |
| bare LF header endings | 7 | 0 / 7 / 0 | 0 / 0 / 7 | 0 |
| close delimiter alone | 7 | 7 / 0 / 0 | 7 / 0 / 0 | 0 |
| close delimiter cut short | 7 | 0 / 7 / 0 | 0 / 7 / 0 | 0 |
| close delimiter then CR LF | 7 | 7 / 0 / 0 | 7 / 0 / 0 | 0 |
| disposition type case | 89 | 89 / 0 / 0 | 89 / 0 / 0 | 0 |
| duplicate Content-Disposition | 89 | 0 / 89 / 0 | 89 / 0 / 0 | 0 |
| duplicate Content-Disposition, other name | 89 | 0 / 89 / 0 | 0 / 0 / 89 | 0 |
| epilogue past the budget | 7 | 0 / 7 / 0 | 7 / 0 / 0 | 0 |
| escaped quote in name | 89 | 0 / 89 / 0 | 0 / 0 / 89 | 0 |
| extra spaces after semicolon | 89 | 89 / 0 / 0 | 89 / 0 / 0 | 0 |
| filename before name | 48 | 48 / 0 / 0 | 48 / 0 / 0 | 0 |
| filename* beside filename | 48 | 0 / 48 / 0 | 48 / 0 / 0 | 0 |
| folded disposition | 89 | 0 / 89 / 0 | 0 / 89 / 0 | 0 |
| header names lower case | 89 | 89 / 0 / 0 | 89 / 0 / 0 | 0 |
| header names upper case | 89 | 89 / 0 / 0 | 89 / 0 / 0 | 0 |
| headers reordered | 48 | 48 / 0 / 0 | 48 / 0 / 0 | 0 |
| name single-quoted | 89 | 0 / 89 / 0 | 0 / 0 / 89 | 0 |
| name unquoted | 77 | 77 / 0 / 0 | 77 / 0 / 0 | 0 |
| name* beside name | 89 | 0 / 89 / 0 | 89 / 0 / 0 | 0 |
| nested multipart type | 89 | 0 / 89 / 0 | 89 / 0 / 0 | 0 |
| no space after semicolon | 89 | 89 / 0 / 0 | 89 / 0 / 0 | 0 |
| padding past the budget | 7 | 0 / 7 / 0 | 7 / 0 / 0 | 0 |
| parameter name case | 89 | 0 / 89 / 0 | 0 / 0 / 89 | 0 |
| second filename parameter | 48 | 0 / 48 / 0 | 48 / 0 / 0 | 0 |
| second name parameter | 89 | 0 / 89 / 0 | 89 / 0 / 0 | 0 |
| short epilogue | 7 | 7 / 0 / 0 | 7 / 0 / 0 | 0 |
| short preamble | 7 | 7 / 0 / 0 | 7 / 0 / 0 | 0 |
| space around equals | 89 | 0 / 89 / 0 | 89 / 0 / 0 | 0 |
| tab after semicolon | 89 | 89 / 0 / 0 | 89 / 0 / 0 | 0 |
| transport padding | 7 | 7 / 0 / 0 | 7 / 0 / 0 | 0 |
| unknown parameter | 89 | 89 / 0 / 0 | 89 / 0 / 0 | 0 |
| whitespace before colon | 89 | 0 / 89 / 0 | 0 / 89 / 0 | 0 |

Run again on 2026-11-07 for 0.9, after D-117 and on the corpus with Firefox and Safari: 2,119 mutants, and still none accepted by both and read differently. (The first run, on 2026-10-29 after D-113, D-114, and D-116, had 1,319 mutants and the same result.) Triage of what remains:

- **multer accepts, form-net refuses:** transfer encodings, nested `multipart/*`, a duplicated `Content-Disposition` or parameter, `name*` and `filename*` beside the plain form, `name = "x"`, padding and epilogues past form-net's budgets. multer reads each by its own rule (first or last wins, encodings ignored); a reader in front of it could read them another way, which is why form-net refuses them. No action for form-net.
- **multer reads differently from its own original, form-net refuses:** a second `Content-Disposition` with another name, `\"` in a name, single quotes, bare LF in headers and before a delimiter. multer reads each, differently from the unmutated body; a reader in front of multer could disagree with it. No action for form-net.
- **Both refuse:** NUL in a header, folding, whitespace before a colon, a cut close delimiter.

The mutant set does not cover everything the two readers could disagree on. The 0.9.x adversarial review (below) built bodies by hand that multer 3.1.0, Go 1.27's `mime/multipart`, and form-net read differently, and checked each one by running those readers. Four were accepted by form-net: a parameter value holding `name=` or `filename=`, `boundary*` beside `boundary`, an empty parameter, and a single-quoted boundary. All four are refused since D-136, and each has a test in `tests/review_multipart_core.rs` or `tests/review_urlencoded_extract.rs`.

### The 0.9.x adversarial review

Three independent reviews (DIRECTIVES, definition of done item 10), each of one area: the multipart core, the driver and storage, and the urlencoded, extraction, and writer code. Each finding came with a failing test, and each was fixed so that its test passes (D-136 to D-146):

| Severity | Finding | Fix |
|---|---|---|
| high | `boundary*` beside `boundary` accepted and ignored; Go frames with the starred value | refused, D-136 |
| medium | an empty parameter accepted in `Content-Disposition` and the request `Content-Type`; Go skips the part or the body | refused, D-136 |
| medium | a single-quoted boundary accepted, and sent by the writer | refused in `Boundary::new`, D-136 |
| medium | a `part_end` refusal closed the sink instead of aborting it: a sink that commits on close kept a refused file | `part_end` runs before `close`, D-139 |
| low | a parameter value holding `name=` read differently by multer | refused, D-136 |
| low | a file field given twice, once as an empty input, accepted | `Duplicate`, D-137 |
| low | `form!` accepted two fields with one wire name | a compile error, D-138 |
| low | the driver's head copy could pass `memory_bound` by one header budget | the block copied once, D-140 |
| low | a second `_charset_` field read differently by the buffered parser and `UploadSession`; `PartReader::text` ignored a skipped `_charset_` | one record, read before the part starts, D-141 |
| low | `MultipartReader` never fed the epilogue, so its budget did not hold | the rest of the body is read, D-142 |
| low | an aborted named spill gave its bytes back to the quota when the delete failed | kept reserved, as on drop, D-143 |
| low | `Upload::parse_with` did not call `failed` on a setup error | called, D-144 |
| low | which error a body with two errors reported depended on the split (the body limit; a framing error against a budget) | parsed to the limit, data charged first, D-145 |
| low | `Pairs` and `Serializer` `Debug` printed the body and the values written | redacted, D-146 |

Decided after the review, in 0.9.2: under the opt-in `ExtendedFilenamePolicy::Ignore` a part with only `filename*` was a text field, where an RFC 2231 reader sees a file. It is refused now, as under `Refuse` (D-147); `Ignore` is deprecated, since a part with both file names was already refused under every policy. `tests/review_multipart_core.rs` holds it.

### The streaming parser against a reference

`fuzz_multipart_differential` compares the streaming parser, at fuzzer-chosen splits, with a small whole-body reference parser on every input the reference understands. One finding, in the harness: its reference had no name limit and reported form-net's correct refusal of a 263-byte name as a disagreement; the reference now stays inside the default limits (0.6.0).

---

## Hardening runs

### Soak (NFR-15)

`tests/soak.rs`, 14,400 seconds, release build, Linux (WSL2, kernel 6.6), Rust 1.99, run on 2026-10-30 on the code that ships (after D-117). 64 sessions upload in a loop through one `Store` (memory threshold 16 KiB), with bodies of 0 bytes to 3 MiB and 20% of uploads dropped part way.

| | result |
|---|---|
| uploads finished | 55,517,561 |
| uploads dropped part way | 13,875,071 |
| worst one-minute heap peak, first half / second half | 509.3 MB / 485.7 MB (the 64 sessions' work in flight, the test's own request bodies included) |
| most files in the store directory at a sample, first half / second half | 40 / 38 |
| most open handles at a sample, first half / second half | 69 / 73 |
| mean open handles at a sample, first half / second half | 50.5 / 51.0 |
| heap left at the end | 10,240 B (the test's own bookkeeping; the bound is 64 KiB) |
| files left at the end | 0 |
| handles left at the end | the same as at the start |

No growth: the second half's worst sample is within the first half's on every measure, by the test's bound of a quarter plus room for which sizes coincide. Handles are the one measure whose worst sample rose, by 4 with 64 sessions in flight; their mean moved by 0.5, and the highest quarter of the run was the third, not the last. The process ends where it started.

### Fuzzing (T-29)

Every target ran for 24 hours on the code that ships (after D-117), all 12 at once at the lowest CPU priority, with a 2 GiB memory limit (nightly `cargo fuzz`, optimised build, Linux on WSL2), from 2026-11-02 15:43 UTC to 2026-11-03 15:51 UTC. No crash, no memory-limit stop, no real timeout: every target's artifact directory is empty at the end.

Eight targets were stopped once each by a false timeout. libFuzzer times a unit on the wall clock, and the WSL2 clock stepped backwards during the run (the kernel logged it 54 times), so a unit read as running for -1 seconds, which libFuzzer's unsigned compare takes as past any limit. Each of those eight inputs replays in milliseconds. The target was resumed from its corpus with the time it had already run counted (D-118), so each ran its full 24 hours with the timeout check on.

| Target | Runs | Stopped by a false timeout at |
|---|---|---|
| `fuzz_boundary` | 3,854,120,074 | 59,823 s |
| `fuzz_decode` | 3,175,598,276 | 10,315 s |
| `fuzz_detect` | 2,606,521,725 | |
| `fuzz_extract` | 974,887,339 | 10,315 s |
| `fuzz_filename` | 594,846,027 | 59,824 s |
| `fuzz_multipart` | 388,030,052 | 59,823 s |
| `fuzz_multipart_differential` | 466,090,813 | |
| `fuzz_part_headers` | 550,069,485 | |
| `fuzz_raw_pairs` | 952,742,629 | 10,315 s |
| `fuzz_upload_session` | 541,323,210 | 10,315 s |
| `fuzz_urlencoded_stream` | 949,518,857 | |
| `fuzz_writer_roundtrip` | 269,026,383 | 59,823 s |
| **Total** | **15,322,774,870** | |

The run counts are about a quarter of the earlier 24-hour run's 56.3 billion (on the code before D-117, also clean): this machine was shared all day with other work at normal priority, while the fuzzers ran at the lowest. Each target's corpus carried over from the earlier runs.
