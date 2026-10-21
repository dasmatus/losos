//! lososd's own routes, called as a given caller. Only for a manifest with
//! `dispatch: true` (the terminal, which replays a call this way).

use crate::{Request, Response};

/// Route `request` as if it had arrived, its caller included.
pub fn call(request: Request) -> crate::Result<Response> {
    crate::wit::dispatch::call(&request.into_wit())
        .map(Response::from_wit)
        .map_err(Into::into)
}
