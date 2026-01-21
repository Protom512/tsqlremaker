# Review Design Command

技術設計書（design.md）を100点満点で評価し、95点未満の場合は改善案を適用します。

## Usage

```
/review-design {spec_name}
```

## Examples

```bash
# Review sap-ase-lexer design
/review-design sap-ase-lexer

# Review another spec
/review-design mysql-emitter
```

## What It Does

1. Reads the design document at `.kiro/specs/{spec_name}/design.md`
2. Reads corresponding requirements document
3. Evaluates against 7 criteria (100 points total):
   - Requirements traceability (20 points)
   - Architecture design (20 points)
   - Data structure design (15 points)
   - Interface design (15 points)
   - Error handling design (10 points)
   - Implementation plan (10 points)
   - Document structure (10 points)
4. If score < 95: Applies fixes to design.md

## Evaluation Criteria

### Requirements Traceability (20 points)
- Mapping between design elements and requirements
- Traceability matrix
- All requirements covered

### Architecture Design (20 points)
- Component responsibilities
- Dependency relationships
- Bounded context compliance

### Data Structure Design (15 points)
- Struct definitions
- Enum usage
- Type safety (Option/Result)

### Interface Design (15 points)
- Public APIs
- Trait definitions
- Function signatures

### Error Handling Design (10 points)
- Error type definitions
- Error propagation
- Error messages with location

### Implementation Plan (10 points)
- Implementation order
- Milestones
- Risk assessment

### Document Structure (10 points)
- Logical organization
- Diagrams
- Code examples

## Output

- Overall score (X/100)
- Category-by-category breakdown
- List of identified issues
- Applied fixes (if score < 95)
