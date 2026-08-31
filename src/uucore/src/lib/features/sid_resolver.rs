// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Asking the authority for the names only it can know.
//!
//! [`sid_render`](crate::sid_render) names well-known SIDs from a static table
//! and leaves everything else as a raw `S-1-…`. This fills that gap: it
//! installs a resolver into [`sid_render::set_resolver`] which asks authd over
//! `/run/ident.sock`, so `ls -l` shows `jack` rather than
//! `S-1-5-21-…-1000`.
//!
//! It is a separate feature from the renderer on purpose. The renderer must
//! keep working with no authority at all — during boot, in a rescue shell, in
//! a build root — where naming what is statically known and printing the rest
//! as SIDs is the honest answer rather than a degraded one.
//!
//! # A connection per lookup, and a cache per process
//!
//! `nss-peios` holds no connection across calls because a shared object cannot
//! see its process fork, and an inherited connection would let a child
//! interleave requests with its parent. That reasoning is about shared objects
//! and does not apply here — but connecting to a Unix socket is cheap, authd
//! closes a connection idle for 120 seconds, and a walk of a large tree can
//! easily outlast that. So: connect, ask, close.
//!
//! What makes that affordable is the cache, and the cache is **per process**.
//! Deliberately:
//!
//! - Within one `ls -lR`, a tree owned by a handful of principals costs a
//!   handful of lookups rather than one per file.
//! - Between two runs of `ls -l`, nothing is remembered, so an account renamed
//!   in `lpsd` shows its new name on the very next command. There is no
//!   invalidation problem because there is nothing to invalidate.
//!
//! This is what libc already does for `getpwuid`, so it matches what people
//! expect from `ls`.
//!
//! # Misses are cached too
//!
//! The map holds `Option<String>`, not `String`. A SID the authority cannot
//! name — a deleted account, a service identity, a principal from a source
//! that is not running — is exactly the case where an uncached lookup would
//! cost a round trip *per file*, and exactly the case most likely to occur on
//! a real system. Caching the miss is what makes the cache do its job when it
//! matters most.
//!
//! The cost is that an account created while a long-running process is already
//! looking at it will not resolve in that process. At per-process scope that is
//! a fair trade; `revstrm` is the only consumer that runs indefinitely, and a
//! stale name in a debug stream is a cosmetic wrong answer.
//!
//! # Failure is always a SID
//!
//! Every failure — no socket, no authority, a timeout, a refusal, a malformed
//! reply — resolves to `None`, and the renderer prints the raw SID. A tool that
//! lists files must not fail because a name server did not answer.

use std::collections::HashMap;
use std::io;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::time::Duration;

use libauthd::ident::{self, Fields, Key, Kind, Outcome};
use libauthd::transport::{recv_message, send_message};
use peios::security::SidRef;

use crate::sid_render;

/// Bounds one lookup, not the whole command.
///
/// Short on purpose: this sits between a user and their `ls` output. If the
/// authority is wedged, printing SIDs promptly is a better answer than a
/// terminal that appears to hang.
const TIMEOUT: Duration = Duration::from_secs(2);

/// SID string -> the name, or the absence of one.
///
/// Keyed on the rendered SID rather than the bytes because that is what the
/// renderer already has in hand, and it makes the map trivially `Ord`-free.
static CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

/// Install the resolver. Idempotent; returns whether this call installed it.
///
/// Call once, early. Nothing here connects: the first lookup does, so a command
/// that never renders a SID never touches the socket.
pub fn install() -> bool {
    sid_render::set_resolver(Box::new(resolve))
}

fn resolve(sid: &SidRef) -> Option<String> {
    let key = sid.to_string();

    let mut guard = CACHE.lock().ok()?;
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(hit) = cache.get(&key) {
        return hit.clone();
    }

    // Held across the lookup on purpose. Two threads asking for the same SID
    // at once is not worth two round trips, and no consumer of this renders
    // SIDs from more than one thread today.
    let answer = ask(sid).unwrap_or(None);
    cache.insert(key, answer.clone());
    answer
}

/// One round trip. `Ok(None)` is an authoritative "no such principal";
/// `Err` is everything else, and the caller treats them the same.
fn ask(sid: &SidRef) -> io::Result<Option<String>> {
    let stream = UnixStream::connect(libauthd::IDENT_SOCKET_PATH)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;

    // NAME only: a renderer wants the name and nothing else, and asking for
    // fields we would discard invites the authority to withhold on a field we
    // never needed.
    let request = ident::encode_lookup(&ident::Lookup {
        tag: 1,
        key: Key::Sid(sid.as_bytes().to_vec()),
        kind: Kind::Any,
        fields: Fields::empty(),
    })
    .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;

    send_message(&stream, &request)?;
    let received = recv_message(&libauthd::wire::FRAMING, &stream)?;
    let reply = ident::decode_lookup_reply(received.expose())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;

    match reply.outcome {
        Outcome::Found => Ok(reply
            .record
            .map(|record| record.qualified_name)
            .filter(|name| !name.is_empty())),
        // NotFound is authoritative and worth caching. Unavailable is not an
        // absence -- but at per-process scope the distinction buys nothing: the
        // command is over in milliseconds and would ask again next time either
        // way, and re-asking a down authority once per file is the cost this
        // cache exists to avoid.
        _ => Ok(None),
    }
}
