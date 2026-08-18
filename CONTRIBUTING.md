# Contributing to HyperType

Thanks for taking the time to contribute.

## Before you start

Please search existing issues and pull requests before opening a new one. For
larger changes, open an issue first so the intended behavior can be discussed
before implementation.

## Development setup

HyperType is a Windows desktop application built with Rust, Tauri, SolidJS,
Vite, and pnpm. Follow the prerequisites in the README, then run:

~~~powershell
pnpm install --frozen-lockfile
pnpm tauri dev
~~~

The optional marketing site has its own workspace:

~~~powershell
cd website
pnpm install --frozen-lockfile
pnpm dev
~~~

## Checks before opening a pull request

Run the checks relevant to your change:

~~~powershell
pnpm build
cd src-tauri
cargo fmt -- --check
cargo test
cd ..\website
pnpm build
~~~

Pull requests should:

- Explain the user-facing or maintenance change.
- Include focused tests when behavior changes.
- Keep generated output, local configuration, and user data out of commits.
- Update documentation when setup or behavior changes.
- Avoid unrelated formatting or dependency churn.

## Commit and review expectations

Keep commits small enough to review and describe the behavior they change.
Maintainers may ask for revisions to clarify scope, tests, or documentation.
By submitting a contribution, you agree that it may be distributed under the
project's MIT license.
