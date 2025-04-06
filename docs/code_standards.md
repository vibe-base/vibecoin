# Vibecoin Code Standards

## General Principles

- **Readability**: Code should be easy to read and understand
- **Maintainability**: Code should be easy to maintain and extend
- **Performance**: Code should be efficient and performant
- **Security**: Code should be secure and resistant to attacks

## Rust Coding Standards

### File Organization

- **File Size Limit**: No Rust file should exceed 1000 lines (enforced by pre-commit hook)
- **Module Structure**: Each module should have a clear responsibility
- **File Naming**: Use snake_case for file names

### Code Style

- Follow the [Rust Style Guide](https://github.com/rust-lang/style-team/blob/master/guide/guide.md)
- Use `rustfmt` to format code
- Use `clippy` to catch common mistakes

### Documentation

- Document all public items with doc comments (`///`)
- Include examples in documentation where appropriate
- Keep documentation up-to-date with code changes

### Error Handling

- Use `Result` and `Option` types for error handling
- Avoid panics in production code
- Provide meaningful error messages

### Testing

- Write unit tests for all functionality
- Aim for high test coverage
- Use property-based testing where appropriate

## Git Workflow

### Branches

- `main`: Production-ready code
- `develop`: Integration branch for features
- Feature branches: Named as `feature/description`
- Bugfix branches: Named as `bugfix/description`

### Commits

- Write clear, concise commit messages
- Follow conventional commits format
- Keep commits focused on a single change

### Pull Requests

- Keep PRs small and focused
- Include tests for new functionality
- Update documentation as needed
- Ensure CI passes before merging

## Code Review Guidelines

### What to Look For

- Correctness: Does the code work as intended?
- Performance: Is the code efficient?
- Security: Are there any security concerns?
- Maintainability: Is the code easy to maintain?
- Test coverage: Are there sufficient tests?

### Review Process

1. Automated checks must pass
2. At least one maintainer must approve
3. All comments must be addressed
4. Changes must be rebased before merging

## Security Practices

- Follow secure coding practices
- Avoid unsafe code unless absolutely necessary
- Review dependencies for security vulnerabilities
- Conduct regular security audits
