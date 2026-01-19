# Review Requirements Command

要件定義書を100点満点で評価し、95点未満の場合は改善案を適用します。

## Usage

```
/review-requirements {spec_name}
```

## Examples

```bash
# Review sap-ase-lexer requirements
/review-requirements sap-ase-lexer

# Review another spec
/review-requirements mysql-emitter
```

## What It Does

1. Reads the requirements document at `.kiro/specs/{spec_name}/requirements.md`
2. Evaluates against EARS format standards using 5 criteria:
   - EARS format accuracy (25 points)
   - Requirements completeness (25 points)
   - Acceptance criteria specificity (20 points)
   - Absence of ambiguity (15 points)
   - Document structure (15 points)
3. Provides detailed scoring for each category
4. If score < 95: Identifies issues and applies fixes

## Evaluation Criteria

### EARS Format Accuracy (25 points)
- Correct use of When/If/While/Where/The system shall triggers
- Consistency across all requirements
- English keywords for EARS triggers

### Requirements Completeness (25 points)
- All functional requirements covered
- Non-functional requirements (performance, security, reliability)
- Constraints documented

### Acceptance Criteria Specificity (20 points)
- Testable criteria
- Quantitative metrics

### Absence of Ambiguity (15 points)
- No vague terms like "appropriately", "as much as possible"
- Unambiguous descriptions

### Document Structure (15 points)
- Introduction with purpose, scope, stakeholders, success criteria
- Logical organization
- Traceability (requirement IDs)

## Output

- Overall score (X/100)
- Category-by-category breakdown
- List of identified issues with locations
- Applied fixes (if score < 95)
