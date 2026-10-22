//! LosOS Reviewer: the box's code model reviewing pull requests, for the
//! repositories that turn it on.
//!
//! It works the way a reviewer on a forge does. When a pull request is
//! opened, reopened or gets new commits, or someone asks the reviewer
//! again, a review goes into a queue; one worker takes them one at a time
//! and posts a single review as the bot account `losos-reviewer`: an
//! overview of the pull request, a table of the files, and comments on
//! lines, with a suggested line where the fix is one line. People answer
//! and resolve its comments like anyone's. It never pushes, approves or
//! merges.
//!
//! The model is the one Build with AI runs (`losos.builder.model`, on
//! Ollama on loopback), so the code never leaves the box and no second
//! model takes memory. A 4 GiB box runs Qwen2.5-Coder 1.5B, which reads a
//! diff well enough to describe it but cannot tell a bug from a good line
//! on its own, so the review is driven by the box: [`checks`] points out
//! the bug shapes it can be sure of, the model is asked short questions
//! with a fixed answer shape ([`review`]), and whatever the model says
//! about a line that the diff does not show is dropped. A larger model
//! (7B on 8 GiB) finds bugs of its own through the same questions.
//!
//! The mesh's shared compute is not used: it would send the code to other
//! people's boxes.

pub mod api;
pub mod checks;
pub mod jobs;
pub mod model;
pub mod patch;
pub mod review;
pub mod worker;
