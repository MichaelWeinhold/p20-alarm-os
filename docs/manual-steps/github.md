# GitHub

**State.** `MichaelWeinhold/p20-alarm-os` is public, MIT-licensed, with the wiki and
projects off and branches deleted on merge. All three merge methods are allowed at the
repository level; the rulesets narrow them per branch.

| Setting | Value |
|---|---|
| Secret scanning | on |
| Push protection | on |
| Dependabot alerts and security updates | on |
| Topics | `linux`, `embedded`, `rust`, `initramfs`, `android-device`, `postmarketos` |

Labels beyond GitHub's defaults: `feature`, `research`, `chore`, `workflow`, `future`,
`dependencies`, `hardware` (needs the phone to do or verify), `device` (on-device Rust code).

Commits in this checkout are authored as `MichaelWeinhold
<104927463+MichaelWeinhold@users.noreply.github.com>`, set in the repository's local git
config, so no private address reaches the public history.

**Reproduce.**

```bash
gh repo edit MichaelWeinhold/p20-alarm-os --enable-projects=false --delete-branch-on-merge \
  --enable-squash-merge --enable-rebase-merge --enable-merge-commit
gh api -X PATCH repos/MichaelWeinhold/p20-alarm-os \
  -f 'security_and_analysis[secret_scanning][status]=enabled' \
  -f 'security_and_analysis[secret_scanning_push_protection][status]=enabled'
gh api -X PUT repos/MichaelWeinhold/p20-alarm-os/vulnerability-alerts
gh api -X PUT repos/MichaelWeinhold/p20-alarm-os/automated-security-fixes
git config user.email 104927463+MichaelWeinhold@users.noreply.github.com
```

**Verify.**

```bash
gh api repos/MichaelWeinhold/p20-alarm-os --jq '.security_and_analysis'
git log --format='%ae' | sort -u
```

The second prints only the noreply address.

- [ ] Create the rulesets for `main` and `production` once the first pipeline has run, so
      the required check names exist, and record their ids and rules here.

→ `docs/device-history/github.md` for the record.
