# Tasks

## Implementation

- [x] 1.1 Add broadcast progress to SDK endpoints and pass observers through native instances into runtime bodies.
- [x] 1.2 Rearm streaming execution on movement; retain unary total budgets and retry fences.
- [x] 1.3 Restrict ingress and encoded completion total deadlines to unary operations (submission.rs, wire.rs); keep listener streaming lifetime consistent.
- [x] 1.4 Add paused-time progress, stall, final-wait and unary regression tests.
- [x] 1.5 Update operation-policy documentation and review README impact.

## Validation And Delivery

- [x] 2.1 Complete focused Rust proof and forbidden-term diff review.
- [x] 2.2 Review the complete candidate and confirm existing Harmos-first delivery authorization.

## Workflow Follow-Up

- After archive and push, run the default all-platform local CI once, start PR checks, squash immediately into dev, and report actual proof. Dokime adoption and cleanup remain obligations of the native goal.
