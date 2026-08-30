---
title: Getting started
description: Install Ordnung, check a small repository, and repair its first findings.
order: 1
---

In this tutorial, we will install Ordnung, point it at a small Rust repository, and repair two of
its findings — one applied by Ordnung, one by hand. By the end, you will have seen the whole loop
at the heart of Ordnung: what exists, what policy wants, and what changing something looks like.

You will need Git and a current Rust toolchain — for the example repository as much as for the
installation.

## Install Ordnung

Install the command-line application straight from the repository:

```sh
cargo install --git https://github.com/PowderworksCode/ordnung ordnung-cli --locked
```

Check that the command is available:

```sh
ordnung --version
```

You should see an Ordnung version number. If your shell cannot find `ordnung`, make sure Cargo's
binary directory is on your `PATH`.

## Create a repository to inspect

Create a small Rust application and enter its directory:

```sh
cargo new ordnung-first-check
cd ordnung-first-check
```

Cargo creates a package manifest, a source file, and a Git repository. That is enough evidence for
Ordnung to identify a project.

## Inspect the inventory

Run the read-only inventory command:

```sh
ordnung inspect .
```

```text
repository: /home/you/ordnung-first-check
github actions: absent
package .: ecosystem cargo, manifest Cargo.toml, workspace standalone, lockfile owner ., lockfile missing
project .: languages [rust], capabilities [], ecosystems [cargo] [Cargo.toml, src/main.rs]
```

The report describes observed files and relationships — a Cargo package, a Rust project, no
workflows yet. It does not evaluate anything.

## Check the repository

Now evaluate that inventory against the effective policy:

```sh
ordnung check .
```

The new repository is deliberately incomplete, so checks fail. This is the expected result:

```text
fail  recommended changelog              CHANGELOG.md: no root changelog found
fail  required    ci-exists              .github/workflows: no GitHub Actions workflows found
fail  required    dependabot             .github/dependabot.yml: no .github/dependabot.yml or .github/dependabot.yaml found
pass  required    gitignore              target: Cargo ignores target/ at .
fail  required    lockfiles              .: Cargo package has no Cargo.lock at its lockfile owner .
fail  required    readme                 README.md: no root README found
…
27 results (10 hidden, see --all): 6 pass, 11 fail, 10 skip — 4 required failures (exit 1)
note: 12 GitHub-backed checks did not run, 4 of them required. Run `ordnung repo-check . --repo owner/name` for the full audit.
```

Each line carries a status, the policy severity, the stable check identifier, the subject, and an
explanation. Only `required` failures gate the exit code; `recommended` findings are reported and
cost nothing. The closing note is Ordnung being honest about its blind spot: some checks read
GitHub settings, and a plain `check` cannot see them — the
[full audit](/how-to-guides/check-a-repository) can.

## Let Ordnung fix what it can

`fix` plans changes Ordnung can make exactly, without guessing:

```sh
ordnung fix .
```

```text
create check:changelog                CHANGELOG.md
planned 1 file change(s) and 0 GitHub setting change(s)
```

One file — most findings are deliberately yours to fix. Nothing has been written; the plan is the
whole output. Carry it out:

```sh
ordnung fix . --apply
```

## Fix one yourself

The `lockfiles` finding wants a committed `Cargo.lock`. Create one:

```sh
cargo generate-lockfile
```

Run the check again:

```sh
ordnung check .
```

```text
pass  recommended changelog              CHANGELOG.md: root changelog present at CHANGELOG.md
pass  required    lockfiles              .: Cargo package is covered by Cargo.lock
…
27 results (10 hidden, see --all): 8 pass, 9 fail, 10 skip — 3 required failures (exit 1)
```

Two findings flipped to `pass`, and neither run touched anything you did not ask for.

For automation, the same report is available as structured data:

```sh
ordnung check . --json
```

You have now completed the basic Ordnung loop: inventory the repository, evaluate it, apply an
exact fix, and verify. Next, run it against
[your own repository](/how-to-guides/check-a-repository), or read
[The Ordnung model](/explanation/model) to understand the boundaries you just crossed.
