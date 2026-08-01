# paranoid

Every check Ordnung knows how to run, required. For repositories that would
rather argue with a check than miss something.

It extends [`recommended`](../recommended), so `policy.toml` is exactly the
difference between the two tiers rather than a second copy of the whole list.

```toml
[[extends]]
git = "https://github.com/PowderworksCode/ordnung"
rev = "<full 40-character commit revision>"
path = "confs/paranoid"
```

## What this tier adds over `recommended`

- **Specific linters**: Vale and Stylelint. Adopting this tier means adopting
  those tools, which is why no other shipped tier mandates them.
- **Ordnung's own conventions**: `field-guide` and `stray-files` are Ordnung
  ideas, not industry practice.
- **Presentation and metadata**: `website` and `action-badge`, which most
  repositories legitimately have no use for.
- **`test-layout`**: tests outside source files, mirroring the source tree. Rust's
  inline `#[cfg(test)]` convention keeps tests in the file they cover; this tier
  takes the opposite position deliberately, so that a test file is findable from
  its source path and source files stay free of test scaffolding.

Every entry is `allow_override = true` except `stray-files`, so a member
repository can request a documented exception under `[overrides]`. A fleet that
extends this tier can redefine anything outright.
