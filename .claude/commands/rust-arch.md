# Rust Architect Command

Consult the Rust architect for design guidance and architecture decisions.

## Usage

```
/rust-arch [design-question]
```

## Description

Get architectural guidance for:
- AST design and structure
- Module organization
- Crate dependencies
- Error handling strategies
- Performance optimization
- Parsing approach selection

## Examples

```bash
# Design AST hierarchy
/rust-arch How should I structure the AST for SELECT statements?

# Evaluate parsing approaches
/rust-arch Should I use recursive descent or parser combinators?

# Performance guidance
/rust-arch How can I optimize keyword lookup for the lexer?
```

## Topics Covered

1. **AST Design**
   - Node hierarchies
   - Enum vs struct decisions
   - Visitor pattern usage

2. **Error Handling**
   - Error type design
   - Error recovery strategies
   - Helpful error messages

3. **Performance**
   - Allocation minimization
   - Iterator usage
   - Static data initialization

4. **Module Structure**
   - Crate boundaries
   - Dependency management
   - Public API design

## The Architect Will

1. Ask clarifying questions about requirements
2. Evaluate trade-offs between approaches
3. Provide concrete code examples
4. Consider long-term maintainability
5. Reference Rust best practices
