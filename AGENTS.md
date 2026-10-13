<!-- Generated from .agents/GUIDANCE.md; edit .agents/GUIDANCE.md and run `stkr project update` -->

# Project Guidance

- Project-local guidance and skills are canonical under `.agents/`; regenerate provider copies with `stkr project update`. Do not author competing provider copies.
- Complete cohesive implementation before broad validation. Every project Python environment is the repository-root `.venv/`. Private task inputs follow the Hub task lifecycle.

- Development exception for Harmos: dev integration requires complete Linux checks, release build, tests and exact-revision artifact proof. Windows runs independently with truthful failures and artifacts, without continue-on-error, and remains advisory for dev until the user explicitly restores its requirement. Keep the development gate/catalog independent of Windows completion and distinct from full release validation. Windows remains required for main and releases. Do not hardcode or automate restoration. This exception supersedes all-platform dev requirements only for Harmos dev.
