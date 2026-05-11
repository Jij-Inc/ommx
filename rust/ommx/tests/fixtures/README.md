# Auth e2e test fixtures

`htpasswd` is the bcrypt-hashed credential file consumed by the
`registry:2` test container in `tests/auth_e2e.rs`. It encodes a single
user `alice` with password `secret`, generated via:

```
htpasswd -nbB alice secret > htpasswd
```

The hash is committed (rather than regenerated at runtime) so the test
suite does not need a bcrypt dependency and so the file is byte-stable
across CI runs.

The credentials are intentionally weak — they only ever authenticate
against an ephemeral testcontainer that is torn down at the end of each
test, so there is no security relevance.
