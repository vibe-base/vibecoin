# File Size Policy

## Overview

Vibecoin maintains a strict policy that no Rust source file should exceed 1000 lines of code. This policy is enforced through a pre-commit Git hook that prevents commits containing Rust files larger than this limit.

## Rationale

### Code Quality

Large files are often difficult to understand, maintain, and test. By limiting file size, we encourage:

- Better code organization
- Clearer separation of concerns
- More focused modules with single responsibilities

### Developer Experience

Smaller files provide several benefits for developers:

- Easier to navigate and understand
- Faster to review during code reviews
- Reduced cognitive load when making changes

### Performance

Smaller files can improve development performance:

- Faster compilation times
- Better IDE performance
- More efficient incremental builds

## Implementation

The file size limit is enforced through a pre-commit Git hook located at `.git/hooks/pre-commit`. This hook:

1. Scans all Rust files (*.rs) in the repository
2. Counts the number of lines in each file
3. Rejects the commit if any file exceeds 1000 lines
4. Provides a list of violating files

## Hook Implementation

```bash
#!/bin/bash
# Check for files > 1000 lines
echo "🔍 Checking file lengths..."
violations=$(find ./vibecoin -type f -name "*.rs" -exec wc -l {} + | awk '$1 > 1000')
if [ ! -z "$violations" ]; then
  echo "❌ Files exceeding 1000 lines:"
  echo "$violations"
  exit 1
fi
echo "✅ All files under 1000 lines."
```

## Exceptions

In rare cases, exceptions to this policy may be granted for:

- Generated code
- Test data files
- Third-party code that cannot be modified

To request an exception, developers should:

1. Open an issue explaining why the file cannot be split
2. Get approval from at least two maintainers
3. Document the exception in the file header

## Best Practices for File Organization

When approaching the line limit, consider these strategies:

1. **Extract modules**: Move related functionality into separate modules
2. **Create helper files**: Move helper functions to dedicated files
3. **Split by functionality**: Divide files based on logical functionality
4. **Use the type system**: Create new types to encapsulate related behavior

## Monitoring and Compliance

Regular audits are performed to ensure compliance with this policy. The CI/CD pipeline includes checks for file size and will flag any files approaching the limit.
