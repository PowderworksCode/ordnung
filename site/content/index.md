<p class="cover"><img src="/cover.png" alt="A tidy house, in order" width="220"></p>

Ordnung checks that a GitHub repository is structurally in order. It never
reads your code — it reads what surrounds it: manifests, lockfiles, workflows,
branch protection, CODEOWNERS, Dependabot configuration, and the layout those
live in.

```console
$ ordnung check .
fail  required    ci-exists              .github/workflows: no GitHub Actions workflows found
fail  required    dependabot             .github/dependabot.yml: no .github/dependabot.yml or .github/dependabot.yaml found
pass  required    gitignore              target: Cargo ignores target/ at .
fail  recommended license                LICENSE: no root license file found
fail  required    lockfiles              .: Cargo package has no Cargo.lock at its lockfile owner .
pass  required    project-inventory      .: detected 1 project boundary/boundaries
…
27 results (10 hidden, see --all): 6 pass, 11 fail, 10 skip — 4 required failures (exit 1)
```

None of these findings breaks anything today, and none is what a code reviewer
looks at — each is invisible until it matters. Ordnung makes the state
checkable: [51 checks](/reference/checks), each with a stable identifier, gate
CI on one repository, and across many, a
[fleet](/how-to-guides/set-up-a-fleet) keeps shared configuration synchronized
from one place, as reviewable pull requests.

Checking is read-only. Every write shows its complete plan first and requires
an explicit `--apply`.

Install the prebuilt binary — Linux x86_64/aarch64, macOS arm64/x86_64:

```sh
curl -fsSL https://ordnung.dev/install | sh
```

[Get started](/getting-started) · [Browse the checks](/reference/checks)
