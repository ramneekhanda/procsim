# 1.6 Multilevel Routing & Reverse Paths

In distributed architectures, clients often communicate through an **API Gateway** or **Reverse Proxy** rather than connecting directly to internal backend services.

```
[ Client ]  <--->  [ API Gateway ]  <--->  [ Auth Service ]
                                    <--->  [ Data Service ]
```

---

## 1. Downstream Request Dispatching

When a client sends a request to the gateway, it specifies the target `path` and its own address in `reply_to`:

```rhai
send("gateway", #{
  type: "REQ",
  path: "/auth",
  req_id: state.req_id,
  reply_to: "client",
  display: "/auth #" + state.req_id
});
```

The gateway inspects `msg.path` and forwards the payload to the corresponding backend service:

```rhai
fn on_msg(msg) {
  if msg.type == "REQ" {
    if msg.path == "/auth" {
      send("auth_svc", msg);
    } else {
      send("data_svc", msg);
    }
  }
}
```

The gateway is a pure router here - it never inspects or enforces authentication
itself. That's each backend service's own job, same as a real API gateway: routing
and authorization are separate concerns.

---

## 2. Reverse Response Routing with `msg.from` and `msg.reply_to`

Backend services don't need direct outgoing links back to the original client. They simply reply to whoever forwarded the request (`msg.from`):

```rhai
// Inside auth_svc:
fn on_msg(msg) {
  let token = "tok_" + msg.req_id;
  send(msg.from, #{
    type: "RESP",
    path: msg.path,
    req_id: msg.req_id,
    reply_to: msg.reply_to,
    status: 200,
    token: token
  });
}
```

When the gateway receives the response (`msg.type == "RESP"`), it forwards it back along the existing connection to `msg.reply_to`:

```rhai
// Inside gateway:
if msg.type == "RESP" {
  send(msg.reply_to, msg);
}
```

---

## 3. Access Control Actually Belongs to the Service, Not the Path Name

A path literally named `/auth` doesn't grant anything by itself - nothing stops a
client from skipping straight to `/data`. The example wires up real enforcement:
the client only requests `/data` after `/auth` has replied with a token, and
attaches that token to the request; `data_svc` checks for it and rejects with
`401` if it's missing, instead of trusting the path name (or worse, `msg.display`)
to imply the caller was ever checked:

```rhai
// Inside data_svc:
fn on_msg(msg) {
  if msg.type == "REQ" {
    if !msg.contains("token") {
      send(msg.from, #{ type: "RESP", path: msg.path, req_id: msg.req_id,
                         reply_to: msg.reply_to, status: 401 });
    } else {
      send(msg.from, #{ type: "RESP", path: msg.path, req_id: msg.req_id,
                         reply_to: msg.reply_to, status: 200 });
    }
  }
}
```

The example graph includes a second client, `rogue_client`, that always requests
`/data` directly with no token - watch its requests come back `401` in the Logs
panel while the well-behaved `client` gets `200`s after authenticating once.

---

## Key Takeaways

1. **`links` defines downstream visibility**: A node's `links` array establishes bidirectional connector paths.
2. **`msg.from` identifies the immediate hop**: Useful for replying to the immediate caller.
3. **`msg.reply_to` tracks the origin client**: Allows multi-hop gateways and proxies to route responses back across complex topologies.
4. **Routing isn't authorization**: A gateway forwarding by path name doesn't enforce
   anything - the destination service has to actually check the caller's credentials.
