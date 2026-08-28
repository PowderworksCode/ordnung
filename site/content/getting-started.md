---
title: Getting started
description: Install Ordnung, create a small repository, and interpret its first structural check.
order: 1
---

In this tutorial, we will install Ordnung and use it to inspect a small Rust repository. By the
end, you will have seen the two views at the heart of Ordnung: what exists and whether it satisfies
policy.

You will need Git and a current Rust toolchain.

## Install Ordnung

Clone the source and install the command-line application:

```sh
git clone https://github.com/PowderworksCode/ordnung.git
cargo install --path ordnung/crates/ordnung-cli
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

Cargo creates a package manifest, source file, and Git repository. That is enough evidence for
Ordnung to identify a project.

## Inspect the inventory

Run the read-only inventory command:

```sh
ordnung inspect .
```

The output should name the repository, a Cargo package, and a Rust project. Notice that the report
describes observed files and relationships; it does not evaluate them yet.

## Check the repository

Now evaluate the inventory against the effective policy:

```sh
ordnung check .
```

The new repository is deliberately incomplete, so some checks will fail—for example, it does not
have a GitHub Actions workflow. This is the expected result. Each line gives you a status, policy
level, stable check identifier, subject, and explanation.

Run the same check as structured data:

```sh
ordnung check . --json
```

You have now completed the basic Ordnung loop: inventory the repository, evaluate it, and inspect
the findings. Neither command changed the repository.

Next, use [How to check an existing repository](/how-to-guides/check-a-repository) in your own
work, or read [The Ordnung model](/explanation/model) to understand the boundaries you just
encountered.
