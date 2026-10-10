# Development Checklist

Use this checklist when submitting pull requests to ensure code quality and consistency.

## Pre-Submission

- [ ] Code compiles without errors
- [ ] All tests pass locally: `cargo test --workspace`
- [ ] No clippy warnings: `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Code is formatted: `cargo fmt --all`
- [ ] Scripts pass: `shellcheck scripts/*.sh && scripts/test.sh` (if changed)

## Git & Commits

- [ ] Branch is up to date with base branch
- [ ] Commits have clear, descriptive messages
- [ ] Branch follows naming convention:
  - `feature/description` for features
  - `bugfix/issue-number` for bug fixes
  - `hotfix/version` for hotfixes

## Code Quality

- [ ] No hardcoded values (use constants or the YAML configuration)
- [ ] Code is well-commented for complex logic
- [ ] Error messages are user-friendly
- [ ] No unnecessary dependencies added
- [ ] No `unwrap()` without justification; proper error handling with `Result<T, E>`
- [ ] No deprecated functions used

## Documentation

- [ ] README.md updated if applicable
- [ ] CHANGELOG.md updated under `## [Unreleased]`
- [ ] Configuration changes documented (YAML format, defaults)

## Testing

- [ ] Unit tests added/updated
- [ ] Manual testing completed, with a light gun if the UI changed
- [ ] Tested on supported platforms (if applicable)

## Checklist Template for PR Description

```markdown
## Description
Brief description of changes

## Related Issues
Closes #123

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change (e.g. configuration format)
- [ ] Documentation update

## Testing
- [ ] Unit tests added/updated
- [ ] Manual testing completed
- [ ] Tested on: Linux / Windows / macOS

## Checklist
- [ ] Code follows project style
- [ ] All tests pass
- [ ] No new warnings (clippy)
- [ ] CHANGELOG.md updated
- [ ] Documentation updated
```
