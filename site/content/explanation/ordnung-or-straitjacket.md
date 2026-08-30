---
title: Ordnung or Straitjacket?
description: Two Powderworks CI scanners, and why they do not overlap.
order: 2
---

Powderworks builds two CI scanners, and they do not overlap.

**Ordnung checks the repository around the code** — that CI exists and gates
the right things, that lockfiles and Dependabot cover every package, that
branch protection is on. It never opens a source file.

**[Straitjacket](https://straitjacket.dev) checks the code itself** for smells
and forbidden patterns.

"Is this project set up correctly?" is Ordnung. "Is this code written well?"
is Straitjacket. Running both is normal: they read different evidence, fail
for different reasons, and one being green says nothing about the other.
