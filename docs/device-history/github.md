# GitHub — the record

## 2026-11-11 — repository created

Created with `gh repo create MichaelWeinhold/p20-alarm-os --public --license mit
--add-readme`, so that the first commit on `main` was made on the server, not by a local
commit the commit guard would refuse.

The security read-back after enabling:

```
{"dependabot_security_updates":{"status":"disabled"},"secret_scanning":{"status":"enabled"},
 "secret_scanning_non_provider_patterns":{"status":"disabled"},
 "secret_scanning_push_protection":{"status":"enabled"},
 "secret_scanning_validity_checks":{"status":"disabled"}}
```

`dependabot_security_updates` read `disabled` in that response because it was switched on by
the `automated-security-fixes` call that followed, which returned success.
