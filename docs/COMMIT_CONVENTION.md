# Commit convention

FastZap uses [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

## Format

```text
<type>(<scope>): <description>
```

The scope is optional. Write the description in English, in the imperative mood,
short and specific, without a trailing period.

## Types

- `feat`: new feature;
- `fix`: bug fix;
- `refactor`: internal change with no intentional functional change;
- `perf`: performance or efficiency;
- `test`: tests;
- `docs`: documentation;
- `build`: dependencies, tooling, and build infrastructure;
- `ci`: workflows and continuous integration;
- `style`: formatting with no behavior change;
- `chore`: maintenance that does not fit the other types;
- `revert`: revert.

## Suggested scopes

`app`, `accounts`, `webview`, `evolution`, `settings`, `migration`, `ui`,
`chat`, `notifications`, `tray`, `dictionaries`, `downloads`, `permissions`,
`proxy`, `customizations`, `reporting`, `i18n`, `packaging`, `flatpak`,
`appimage`, `snap`, `windows`, `macos`, `release`, and `docs`.

Use a scope only when it clearly identifies the main area of the change. Do not
combine several names into a generic scope; when independent changes can be
split apart, propose separate commits.

## Examples

```text
feat(evolution): pair accounts by phone code
fix(proxy): preserve the active proxy after validation failure
refactor(ui): move shared dialogs to components
test(migration): cover inverted ZapZap close preference
docs(architecture): describe the child webview layout
ci(windows): validate native runner architecture
```

## Non-trivial commits

Use the body to record the context and the tests actually run:

```text
fix(webview): preserve disabled account state

Keep disabled accounts registered without creating a child webview and
create it only after explicit activation.

Tests:
- cargo test --manifest-path src-tauri/Cargo.toml accounts
- pnpm check
```

Do not record a test as passing if it failed or was not run. When a
validation cannot be completed, describe the limitation outside the suggested
message, in the task delivery.

## Breaking changes

Use `type!:` or `type(scope)!:` and add a `BREAKING CHANGE:` footer with the
impact and the required migration. Also record the change in the numbered
section marked `In development` in `CHANGELOG.md`.

## Codex and other agents

When finishing a logical unit, the agent must review `git status` and `git diff`,
run the applicable checks, update `CHANGELOG.md`, and produce a suitable
Conventional Commit message. Every final response after modifying repository
files must include at least one suggested message, even for small changes.

Independent changes must get separate messages. Non-trivial changes must also
include an optional body with context and the tests actually run. The agent must
not rewrite published history without explicit instruction.

## Relation to the changelog

The commit message describes one logical unit of work; the changelog keeps the
curated history of the release cycle. One does not replace the other. Every
change or addition is still required to update `CHANGELOG.md`, as per the
[maintenance](maintenance.md) contract.

## Validation

The convention is currently verified during review. A local or CI validator must
be introduced in its own change, with tests for accepted and rejected examples,
before being treated as an automated project requirement.
