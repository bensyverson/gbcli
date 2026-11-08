//! The SameBoy bindings' integration tests, one binary (`smoke` keeps its own).
//! One binary links once and starts once; a test that needs a process of its
//! own is a file beside this directory instead, saying why.

mod layer_tags;
mod patch_set;
