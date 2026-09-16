# logq

Index JSON logs on disk, then query the index.

```
logq index logs/
logq query -o logs/.logq --level error --since 2026-08-07T12:00:00Z --until 2026-08-07T13:00:00Z
logq stats -o logs/.logq
```

A line is one JSON object. Time is read from `ts`, `time`, `timestamp`, or `@timestamp`: RFC3339, or a unix timestamp in seconds or milliseconds. Level is `level`, `lvl`, or `severity`. Service is `service`, `svc`, or `logger`.

`logq index logs/ --field status` stores `status` in its own dictionary. Worth it when the field does not have a different value on every line.

`--until` leaves out the instant you pass. `--contains` opens the original file and checks the line text. Running `logq index` again skips a file when the path, size, and mtime are the same.

`logq logs/app.log --contains timeout` scans without an index. A directory walk picks up `.log`, `.jsonl`, and those names ending in `.gz`.
