# Contributing

Contributions are welcome. A few things worth knowing before you spend time.

## How changes land

Every change reaches `main` through a pull request that the maintainer reviews
and merges. That includes changes from forks, which is the normal path: you
cannot push to this repository directly, and nothing merges without a review.
Fork the repository, branch from the latest `main`, and open the PR against
`main`. Keep one topic per PR. If a PR sits without a response, a nudge is
fine.

Commit messages follow the short conventional style already in the log:
`fix: ...`, `docs: ...`, `chore: ...`, with the PR number added on merge.

## Before opening a PR

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
```

CI runs exactly these four on `ubuntu-latest` with the stable toolchain.
Warnings are denied, and that includes `missing_docs`: a new public item
without a doc comment fails the build.

## AI-assisted contributions

Using AI tools is fine; please just be open about it.

- Say in the PR description which AI tools you used, if any: none, some or most, plus the tool or agent and the model (for example "Claude Code, Opus 5.5" or "Copilot, GPT-5").
- Write the PR description and any issues yourself, in your own words. Short, plain descriptions are welcome; please don't paste generated text.
- Read, run and test every line you submit, and be ready to explain it in review.
- PRs without the disclosure may be closed.

## What a good change looks like

A test built on a worked example with a stated source. Every check here
implements an SNI clause, and the test for it should reproduce a published
example, from the standard's own commentary or from a well-known textbook for
the equivalent ACI, AISC or ASCE provision, with a comment naming the
document, edition and page. A test that only checks the code against itself
does not show the formula is right.

For a bug fix, the test fails without the fix.

## Things to be careful with

**Cite the clause.** Every new check says in its doc comment which SNI
standard, edition and clause it implements. If you cannot confirm a
coefficient, do not guess one: leave it as an input and say so in the PR.

**Do not reproduce the standard.** SNI documents are copyrighted by BSN. The
crate implements equations and procedures and cites clause numbers. It does
not copy text and does not embed tables: a value an engineer reads from a
table (site coefficients, `R`, `Ie`, `kv` and similar) is an input.

**Refuse rather than guess.** A case outside the implemented clauses returns
`Err(SniError)`, for example `UnsupportedSection`, rather than an answer from
a clause that does not apply.

**No bare booleans.** A check returns the demand, the capacity, the ratio,
the governing limit state, and the intermediate values needed to repeat the
calculation by hand.

**Units.** Every quantity is a newtype and conversion happens only in named
constructors and accessors. Do not add a function that takes a bare `f64`
for a physical quantity.

**No panics.** `unwrap` and `expect` belong in tests and examples only.

## Scope

Member and load checks from SNI 1726, 1727, 1729 and 2847. Irrigation
hydraulics belongs in
[imaji-kp-hydraulics](https://github.com/saimskywalker/irigasi-imaji-kp-hydraulics).
The crate has no dependencies beyond the standard library, and a PR that adds
one needs a strong reason.
