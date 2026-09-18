# logq

Local index for json logs. The first run walks the files. Later queries use the index.

```
logq index logs/
logq query -o logs/.logq --level error --since 2026-08-01T00:00:00Z
logq stats -o logs/.logq --by service
```

Time is `ts`, `time`, `timestamp` or `@timestamp` (RFC3339, or unix seconds/millis). Level is `level`, `lvl` or `severity`. Service is `service`, `svc` or `logger`.

`logq index logs/ --field status`, then `logq query -o logs/.logq --field status=500`. Fine for status codes. Useless for request ids.

`--until` does not include that instant. `--contains` reads the original line. Indexing again skips a file when the path, size and mtime match.

`logq app.log --contains refused --stats` scans the file and does not touch the index. A directory walk takes `.log`, `.jsonl` and gzip of those.
