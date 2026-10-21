//! Mesh CI: GitHub Actions workflows run on the boxes of a mesh.
//!
//! LosOS Git tells `losos-ci` about pushes and pull requests. It reads the
//! commit's `.github/workflows/*.yml`, plans the jobs an event starts and
//! hands each one to the edge's queue, where any box of the mesh that
//! lends its runner may take it. Results come back as commit statuses, the
//! Checks box of a pull request. The service exists only on a box that
//! shares and has joined a mesh (`lososInternal.sharedSide`).
//!
//! * [`expr`]: the `${{ }}` expression language.
//! * [`glob`]: the branch and tag filters.
//! * [`workflow`]: parsing, triggers, the matrix and job planning.

pub mod expr;
pub mod glob;
pub mod workflow;
