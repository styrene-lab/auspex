+++
title = "Policy Followup Normalization and Exclusivity"
tags = ["styrene-policy","authorization","obligations","supererogations"]
+++

+++
id = "d5561c8d-4383-470b-89ab-82196e6428ce"
kind = "design_node"

[data]
title = "Policy Followup Normalization and Exclusivity"
status = "decided"
issue_type = "policy-design"
priority = 1
parent = "54a633b0-0853-445d-afd7-a60093d86b0a"
dependencies = []
open_questions = []
+++

## Overview

# Policy Followup Normalization and Exclusivity

---
title: Policy Followup Normalization and Exclusivity
status: decided
tags: [styrene-policy, authorization, obligations, supererogations]
---

# Policy Followup Normalization and Exclusivity

Parent: [[styrene-policy-crate-boundary]]

## Problem

`PolicyDecision` carries two followup sets:

```rust
obligations: Vec<PolicyFollowup>
supererogations: Vec<PolicyFollowup>
```

Supererogations are a superset-capable mirror of obligations, but the same followup must not be doubled-up in both sets. There are also mutually exclusive followup relationships that need normalization.

## Decision

`PolicyDecision` must normalize followups before being returned to callers.

Normalization rules:

1. **Obligation wins over supererogation.**
   - If `Signature` is mandatory, it must not also appear as recommended.
   - Remove any supererogation that is also present in obligations.

2. **Each set is internally deduplicated.**
   - `obligations` is a set.
   - `supererogations` is a set.
   - Ordering should be deterministic for stable audit output.

3. **XOR relationships are enforced by policy schema, not by caller convention.**
   - Mutually exclusive followups should be declared centrally.
   - Invalid combinations should normalize to a deterministic decision or produce a policy-construction error.

4. **Harder followup wins when a soft/hard pair conflicts.**
   - Example: `Approval` obligation removes `OperatorConfirmation` if confirmation is defined as weaker approval.
   - Example: `RuntimeCompatibility` obligation may remove `RuntimeReprobe` supererogation if compatibility proof already requires the reprobe.

5. **Supererogations may be dropped/coalesced by callers, but obligations may not.**

## Proposed API

```rust
impl PolicyDecision {
    pub fn normalized(mut self, rules: &FollowupRules) -> Result<Self, PolicyError>;
}

pub struct FollowupRules {
    pub ordering: Vec<PolicyFollowup>,
    pub aliases: Vec<(PolicyFollowup, PolicyFollowup)>,
    pub obligation_dominates: Vec<(PolicyFollowup, PolicyFollowup)>,
    pub xor_groups: Vec<Vec<PolicyFollowup>>,
}
```

## Example normalization

Input:

```text
obligations: [Audit, Signature]
supererogations: [Signature, DryRun, Audit]
```

Output:

```text
obligations: [Audit, Signature]
supererogations: [DryRun]
```

## Example XOR/dominance rule

If `Approval` is a stronger form of `OperatorConfirmation`:

```text
obligation_dominates: [(Approval, OperatorConfirmation)]
```

Input:

```text
obligations: [Approval]
supererogations: [OperatorConfirmation]
```

Output:

```text
obligations: [Approval]
supererogations: []
```

## Open questions

- Which followups are true aliases versus weaker/stronger related actions?
- Should XOR violations be normalized, denied, or treated as policy authoring errors?
- Should `DryRun` and `PostActionVerification` ever be mutually exclusive, or are they independent?
- Is `OperatorConfirmation` a weaker form of `Approval`, or a distinct human-factor signal?

## Initial recommendation

Start with simple normalization:

```text
deduplicate each set
remove supererogations already present in obligations
use deterministic ordering
```

Then add explicit dominance/XOR rules only after real policies reveal conflicts. Do not over-model before we have concrete cases.

## Open Questions
