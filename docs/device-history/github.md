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
the `automated-security-fixes` call that followed, which returned success; a read-back
afterwards printed `{"status":"enabled"}`.

## 2026-11-11 — `production` and the rulesets

After PR 2 merged with all five checks green, `production` was created from `main` at
`323e42f`, and both rulesets were created with a POST each. They returned ids `27054316`
(`main`) and `27054317` (`production`). The read-back:

```
main: enforcement=active bypass=0 rules=deletion,non_fast_forward,pull_request,required_status_checks,required_linear_history merge=squash/rebase checks=checks / check, checks / secrets, checks / image, closing-references, dependencies
production: enforcement=active bypass=0 rules=deletion,non_fast_forward,pull_request,required_status_checks merge=merge checks=checks / check, checks / secrets, checks / image, closing-references, dependencies
```

A probe commit pushed straight to `main` was refused:

```
remote: error: GH013: Repository rule violations found for refs/heads/main.
 ! [remote rejected] main -> main (push declined due to repository rule violations)
```
