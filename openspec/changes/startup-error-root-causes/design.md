# Design

## Agreed Contract

One plugin startup/prepare pass is the classification boundary. A Runtime preparation/construction failure is a root. Every MissingResource in the same pass is a consequence regardless of reporting order; all other failures remain roots. With no preparation failure, MissingResource errors are roots. Do not introduce a resource dependency contract or infer relationships from names.

## Ownership

SDK operations/startup.rs owns classification and readable Display. SDK registration/serving transports the complete StartupReport in the existing Failure detail harmos.startup. Each issue retains its original Failure, resource and operation fields. Existing refusal codes, details and diagnostic chains remain unchanged. Runtime reexports the report for consumers.

## Locked Inventory

SDK startup report/classification, SDK registration and serving carrier mapping, runtime reexports, inline classification/carrier/rendering tests, product guide; README reviewed and unaffected. No new dependency graph, author contracts or hilti-hcl edits.

## Paired Consumer

Dokime ../dokime.wt/rootcause (fix/plugin-start-root-cause) maps roots, collapsed consequences and dokime.secret identity. Integration uses an uncommitted Cargo patch. Both repositories commit locally only; published Harmos revision adoption remains with the orchestrator.
