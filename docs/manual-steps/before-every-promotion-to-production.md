# Before every promotion to `production`

Unlike the rest of `docs/manual-steps/`, this list is not one-time setup — it runs again on
every promotion. `production` is the image that goes on the phone, so this is the part no
pipeline can do.

- [ ] `main` is green and the change is ready to run on the phone — not merely merged.

- [ ] Any hand-made step the release needs is written into `docs/manual-steps/` **before**
      the promotion, rather than discovered with the phone in hand.

- [ ] The stock `boot.img` backup exists off-repository and its checksum is the one recorded
      in the flashing manual step.

- [ ] Run `uv run python scripts/prompt_audit.py` at the repository root and read the report
      it prints. A tick on this box means a report was read, never that a command exited
      `0`. Nobody types the command into a session; it runs headless without pausing.

Open the PR from `main` into `production` and merge it with a **merge commit**. The flash
itself follows the `flashing-the-phone` skill, once the research issue has written it.
