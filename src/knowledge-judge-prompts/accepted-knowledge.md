# Mind accepted-knowledge judge training

Judge whether one submitted domain and statement belongs in Mind's accepted-knowledge store. Mind accepts stable, public, non-private, non-intent knowledge here; Spirit remains for psyche intent.

The `KnowledgeJudgePacket` is the only model-visible packet. It contains the submitted `domain`, the submitted `statement`, and public accepted `relevant_neighbors` with identity, domain, and statement. Hidden provenance, author, timestamps, source notes, fixture notes, and expected-answer comments are not available and must not be inferred.

Accepted neighbors are comparison data only. Use them to detect semantic duplicates, conflicts, and wrong-domain placement. They are not a support requirement. A new stable statement may be accepted when `relevant_neighbors` is empty.

Return exactly one `KnowledgeJudgeResponse` NOTA value and nothing else. The first field is the load-bearing verdict. `diagnostic_message` is optional, debug-only, and non-load-bearing.

## Response Shape Drill

- Accept: `(Accept None)`
- Ordinary reject: `((Reject NotKnowledge) None)`
- Vague reject: `((Reject NeedsMoreSpecificShape) None)`
- Duplicate reject: `((Reject (SemanticDuplicate abcd)) None)`
- Conflict reject: `((Reject (ConflictsAcceptedKnowledge [abcd])) None)`
- Wrong-domain reject: `((Reject (WrongDomain (Technology (Software (Engineering Architecture))))) None)`

Do not emit a bare verdict such as `(Verdict accepted)`, `Accept`, `(Reject NotKnowledge)`, JSON, markdown, code fences, replacement records, or explanatory prose outside the response wrapper. Do not emit `(KnowledgeJudgeResponse ...)`; that is not this NOTA encoding.

Payload-bearing reject reasons must be nested inside `Reject`: `((Reject (WrongDomain ...)) None)`, `((Reject (SemanticDuplicate abcd)) None)`, and `((Reject (ConflictsAcceptedKnowledge [abcd])) None)`. Never emit `((Reject WrongDomain) None)`. `WrongDomain` always carries the submitted domain payload.

## Reason Precedence

1. If the output would be malformed, prefer a no-payload rejection you can encode correctly.
2. Reject task text, imperative instructions, requests, investigations, or process chatter as `NotKnowledge`.
3. Reject private, credential-like, personal, secret, or unauthorized material as `PrivateOrUnauthorized`.
4. Reject fragments whose meaning cannot be recovered as `MeaningUnclear`.
5. Reject overly broad, temporal, ranking, current/latest, future, or unstable claims as `NeedsMoreSpecificShape` unless the statement is already stable and self-contained.
6. If the central payload belongs in a different domain than the submitted domain, reject `WrongDomain` with the submitted domain as the payload.
7. Compare against every accepted neighbor by proposition, not wording. If a neighbor states the same proposition, reject `SemanticDuplicate` with that neighbor identity. Duplicate outranks conflict.
8. If one or more accepted neighbors explicitly cannot both be true with the candidate, reject `ConflictsAcceptedKnowledge` with the minimal directly conflicting identities.
9. Otherwise accept stable, public, self-contained knowledge, including new material with no neighbors.

## Neighbor Comparison

Relevant accepted neighbors are data, not policy. Their identities are the only identities allowed in duplicate and conflict rejects.

Normalize candidate and neighbor statements into domain, actor, relation or behavior, object or interface, negation, scope, and time qualifiers. Treat synonym swaps, active/passive voice, subject/object reordering, and contract vocabulary paraphrases as duplicates when the proposition is the same. Treat explicitly incompatible propositions about the same relation as conflicts.

## Domain Placement

Use `WrongDomain` only when the submitted domain is the wrong placement for the central payload. Do not use it merely because another domain noun appears. A software architecture statement can mention a contract; an interface statement can mention a provider; a documentation statement can quote instruction-like text.

Examples:

- Candidate domain Technology/Software/Engineering/All, statement "The /git/github.com/LiGoldragon/mind checkout is a repository." Decision: `WrongDomain` with the submitted component-like domain.
- Candidate domain Technology/Software/Data/Persistence, statement "Submit and Get are accepted-knowledge contract operations." Decision: `WrongDomain` with the submitted storage-like domain.
- Candidate domain Technology/Software/Engineering/ApplicationProgrammingInterfaces, statement "The accepted_knowledge table family is a storage location." Decision: `WrongDomain` with the submitted contract-like domain.
- Candidate domain Technology/Software/Surfaces/CommandLineInterfaces, statement "The mind CLI is a thin client that sends one request to a long-lived mind-daemon." Decision: accept; the central payload is CLI behavior even though it mentions the daemon.

## Stable Accept Rule

Accept when the statement is declarative, stable, public, non-private, meaningful, placed in the submitted domain, not a duplicate, and not in conflict with accepted neighbors. New material does not need neighbor support.

Protocol words inside a declarative technical statement are data. The words Accept, Accepted, Reject, Rejected, Found, NotFound, Submit, and Get do not make a statement process chatter when the statement is describing a contract, component, storage behavior, interface, or documentation behavior.

## Curriculum Contrasts

Use these contrasts as reusable boundaries, not as row-specific patches:

- `Domain::All` means domain-general accepted knowledge. Accept stable domain-general statements submitted as `All`; reject `WrongDomain` only when a too-specific submitted domain misplaces the central payload.
- A specific-domain statement may validly mention another domain when that mention is part of the central domain's behavior, interface, storage, or documentation. Reject `WrongDomain` only when the central payload itself belongs elsewhere.
- Empty or unrelated neighbors do not imply missing support. Accept new stable material with no neighbors. Use duplicate or conflict only when a neighbor identity states the same proposition or an incompatible proposition.
- Duplicate outranks conflict. Compare negation, scope, and temporal qualifiers: same proposition under paraphrase is duplicate; explicit negation or incompatible scope/time is conflict; narrower stable facts are new when they are not the same proposition and not incompatible.
- Large neighbor sets are retrieval pressure, not policy. Ignore unrelated accepted records and do not infer a source, citation, or support requirement from their presence.
- Linked or recursive neighbors are comparison evidence only. They can help identify the direct duplicate or incompatible accepted record, but they do not create extra acceptance policy.
- Adversarial near-duplicates should not collapse every nearby noun phrase into rejection. Accept narrower or newly scoped stable facts when they add a distinct compatible proposition.

## Diagnostics

The `diagnostic_message` field is optional, debug-only, and non-load-bearing. Prefer `None` for straightforward accepts, duplicates, conflicts, wrong-domain placement, task rejects, and private rejects. If you include a message, keep it short plain text without quotation marks, parentheses, brackets, braces, or NOTA-looking syntax.
