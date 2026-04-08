---
title: Labelled Types and Boundary Rules
date: 2026-04-07
author: Sigil Language Team
slug: labelled-types-and-boundary-rules
---

# Labelled Types and Boundary Rules

How can you enforce handling of Personally Identifiable Information (PII) and other sensitive data during compile time? What about API keys that should only go to a specific service and never be logged?

The approach Sigil takes is to let developers label types and explicitly enforce handling of labelled data at a named •topology boundary, such as interactions with the filesystem, a process, a log, or an external service. If the labelled type is not explicitly handled at the boundary, it will be blocked.

Quick code example:

```sigil invalid-module
⟦ labels are user defined ⟧
label Pii

⟦ The Ssn type is a string of length 11, and is labelled as Pii ⟧
t Ssn=String where #value=11 label Pii

⟦ Replace Ssn by *** ⟧
transform λredactSsn(ssn:µSsn)=>String="***"

⟦ When Pii goes through the audit log boundary, it will be redacted ⟧
rule [µ.Pii] for •topology.auditLog=Through(•policies.redactSsn)
```

## Combining Labels

Projects can also define label implication with `combines`, which allows boundaries to act on groups of labels. Here’s how to classify Portuguese and Spanish PII under Europe, allow it at an EU payroll boundary, and block it at a US analytics boundary:

In `projects/labelled-boundaries-eu/src/types.lib.sigil`:

```sigil module projects/labelled-boundaries-eu/src/types.lib.sigil
label Pii

label Portugal

label Spain

label Europe combines [Portugal,Spain]

t Niss=String label [Pii,Portugal]

t Nuss=String label [Pii,Spain]
```

In `projects/labelled-boundaries-eu/src/topology.lib.sigil`:

```sigil module projects/labelled-boundaries-eu/src/topology.lib.sigil
⟦ Now let's suppose we have two HTTP services, one for EU payroll and one for US analytics ⟧
c euPayrollApi=(§topology.httpService("euPayrollApi"):§topology.HttpServiceDependency)

c usAnalyticsApi=(§topology.httpService("usAnalyticsApi"):§topology.HttpServiceDependency)
```

In `projects/labelled-boundaries-eu/src/policies.lib.sigil`:

```sigil module projects/labelled-boundaries-eu/src/policies.lib.sigil
⟦ If it's PII and Europe, allow it to go to the EU payroll API ⟧
rule [µ.Europe,µ.Pii] for •topology.euPayrollApi=Allow()

⟦ If it's PII and Europe, block it from going to the US analytics API ⟧
rule [µ.Europe,µ.Pii] for •topology.usAnalyticsApi=Block()
```

## Boundary Rules Live in Policies

Classification is separate from explicit boundary handling. Projects put rules and transforms in `src/policies.lib.sigil`, where projects define the exact-boundary rules and trusted transforms. Here's how to redact USA PII going to the audit log:

```sigil module projects/labelled-boundaries/src/policies.lib.sigil
transform λredactSsn(ssn:µSsn)=>String="***"

rule [µ.Pii,µ.Usa] for •topology.auditLog=Through(•policies.redactSsn)
```

When labelled data reaches a named •topology boundary, the compiler resolves one of three outcomes:

- `Allow()`
- `Block()`
- `Through(transform)`

Unlabelled data is unaffected by this system. The point is not to make every value policy-heavy. The point is to make the rare values that really matter carry enforcement-bearing meaning. In most systems the majority of types will never be labelled.

## Topology Is Broader Now

This feature also widened the meaning of topology. Topology is no longer just HTTP and TCP dependency declarations. It is the language surface for named runtime boundaries:

- HTTP services
- TCP services
- filesystem roots
- log sinks
- process handles

That makes `label` and boundary rules useful outside network calls. For example, a project can now route labelled values through:

- `•topology.auditLog`
- `•topology.exportsDir`
- `•topology.govBrCli`

instead of treating filesystem, logging, and child-process execution as anonymous ambient sinks.

## A Small Real Project

The repo now includes a small runnable project under:

- `projects/labelled-boundaries/`

It shows three concrete cases:

- `Ssn` is labelled as `[Pii,Usa]`
- `Cpf` is labelled as `[Brazil,Pii]`
- `GovBrToken` is labelled as `[Brazil,Credential,GovAuth]`

and then handled through exact topology boundaries:

- USA PII heading to the audit log must go through a transform
- Brazilian PII may be written to a named export root
- the gov.br token may only reach the named gov.br process boundary
