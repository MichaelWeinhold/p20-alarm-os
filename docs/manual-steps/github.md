# GitHub

**State.** `MichaelWeinhold/p20-bedside-clock` is public, MIT-licensed, with the wiki and
projects off and branches deleted on merge. All three merge methods are allowed at the
repository level; the rulesets narrow them per branch.

| Setting | Value |
|---|---|
| Secret scanning | on |
| Push protection | on |
| Dependabot alerts and security updates | on |
| Description | A bedside clock made from an old Huawei P20 Lite: the stock Linux kernel, and one static Rust program as `/init` in place of Android. Alarms, a timer, and deep sleep between uses. |
| Topics | `rust`, `embedded-linux`, `aarch64`, `musl`, `framebuffer`, `alarm-clock`, `huawei`, `pid1` |

Labels beyond GitHub's defaults: `feature`, `research`, `chore`, `workflow`, `future`,
`dependencies`, `hardware` (needs the phone to do or verify), `device` (on-device Rust code).

Commits in this checkout are authored as `MichaelWeinhold
<104927463+MichaelWeinhold@users.noreply.github.com>`, set in the repository's local git
config, so no private address reaches the public history.

**Reproduce.**

```bash
gh repo rename p20-bedside-clock --repo MichaelWeinhold/p20-alarm-os --yes
git remote set-url origin git@github.com:MichaelWeinhold/p20-bedside-clock.git
gh repo edit MichaelWeinhold/p20-bedside-clock --enable-projects=false --delete-branch-on-merge \
  --enable-squash-merge --enable-rebase-merge --enable-merge-commit
gh repo edit MichaelWeinhold/p20-bedside-clock \
  --description 'A bedside clock made from an old Huawei P20 Lite: the stock Linux kernel, and one static Rust program as /init in place of Android. Alarms, a timer, and deep sleep between uses.' \
  --add-topic rust,embedded-linux,aarch64,musl,framebuffer,alarm-clock,huawei,pid1 \
  --remove-topic linux,embedded,initramfs,android-device,postmarketos
gh api -X PATCH repos/MichaelWeinhold/p20-bedside-clock \
  -f 'security_and_analysis[secret_scanning][status]=enabled' \
  -f 'security_and_analysis[secret_scanning_push_protection][status]=enabled'
gh api -X PUT repos/MichaelWeinhold/p20-bedside-clock/vulnerability-alerts
gh api -X PUT repos/MichaelWeinhold/p20-bedside-clock/automated-security-fixes
git config user.email 104927463+MichaelWeinhold@users.noreply.github.com
```

**Verify.**

```bash
gh api repos/MichaelWeinhold/p20-bedside-clock --jq '.security_and_analysis'
gh repo view MichaelWeinhold/p20-bedside-clock --json name,description,repositoryTopics
git log --format='%ae' | sort -u
```

The second prints only the noreply address.

## Rulesets

**State.** `main` and `production` are protected by repository rulesets, ids `27054316` and
`27054317`. Nobody bypasses either, the owner included.

| Rule | `main` | `production` |
|---|---|---|
| Direct push blocked (PR required) | yes | yes |
| Required checks: `checks / check`, `checks / secrets`, `checks / image`, `closing-references`, `dependencies` | yes | yes |
| Branch must be up to date before merge | yes | yes |
| Force-push blocked | yes | yes |
| Branch deletion blocked | yes | yes |
| Conversations must be resolved | yes | yes |
| Stale approvals dismissed on new push | yes | yes |
| Linear history required | yes | **no** |
| Merge methods offered | squash, rebase | merge commit only |
| Required approvals | 0 | 0 |

All five checks are reported by the GitHub Actions app (`integration_id` 15368).

⚠️ Approvals `0` holds only while one person has push access. → ADR 0001, D2.

**Reproduce.** The bodies are built as JSON with `bypass_actors: []` and the rules above,
then:

```bash
gh api repos/:owner/:repo/rulesets --method POST --input ruleset-main.json
```

⚠️ A ruleset PUT replaces the whole ruleset, so read it first, change one rule in the body,
and diff the readback against the state before:

```bash
gh api repos/:owner/:repo/rulesets/<id> > ruleset.json
gh api repos/:owner/:repo/rulesets/<id> --method PUT --input ruleset.json
```

**Verify.**

```bash
gh api repos/:owner/:repo/rulesets/<id> \
  --jq '.rules[] | select(.type=="required_status_checks")
        | .parameters.required_status_checks[].context'
```

This prints the five checks, on both ids. `git push origin main` from a local commit is
refused with `GH013: Repository rule violations`.

⚠️ A ruleset edit is the only thing that makes a check **block** a merge; a workflow on its own
is a visible red.

→ `docs/device-history/github.md` for the record.
